#!/usr/bin/env python3
"""aurora_storage — Atomic Rollback, Hash-Chained WAL & WiGLE/ESP32/AlexAI State Engine.

Implements the two-fsync atomic checkpoint + hash-chained Write-Ahead Log (WAL)
for `.aurora/state/`:
  - `MANIFEST`: active `<epoch>` pointer
  - `checkpoint.<epoch>.snap`: immutable base state snapshot
  - `checkpoint.<next>.tmp`: staging file synced before atomic `os.replace`
  - `wal.<epoch>.log`: append-only binary frame stream:
      `Frame_k = CRC32 (4B) || LSN (8B) || OpCode (1B) || Length (4B) || H_{k-1} (32B) || Payload_k`
      where `H_k = BLAKE2b-256(LSN || OpCode || H_{k-1} || Payload_k)`

Integrates:
  - WiGLE DataBomb v3.0 Custodian Principle (`Record != Admit != Commit`, `29/30` @ `0x62`)
  - ESP32-S3 MEDUSA (`ADA:MEDUSA`, `VAK:MEDUSA`, `OMN:MEDUSA`) `64B@0x62` `LitanyWireFrame`
  - AlexAI (`GAIA-MLX-QUANT` BitNet b1.58 `{-1, 0, +1}`) state folding
"""

from __future__ import annotations

import hashlib
import json
import os
import struct
import zlib
from pathlib import Path
from typing import Any, Callable, Dict, Tuple


def fsync_dir(dir_path: Path) -> None:
    """Flush directory entry metadata to disk (POSIX O_DIRECTORY fsync)."""
    fd = os.open(str(dir_path), os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def atomic_write_file(target_path: Path, data: bytes) -> None:
    """Guarantee atomic file replacement with zero partial-write exposure (2x fsync)."""
    parent_dir = target_path.parent
    parent_dir.mkdir(parents=True, exist_ok=True)
    tmp_path = parent_dir / f".{target_path.name}.tmp.{os.getpid()}"

    with open(tmp_path, "wb") as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())

    os.replace(tmp_path, target_path)
    fsync_dir(parent_dir)


class AuroraStateEngine:
    # Frame Header: CRC32 (4B) | LSN (8B) | OpCode (1B) | Length (4B) | PrevHash (32B) = 49 bytes
    HEADER_FMT = "<QBI32s"
    HEADER_SIZE = 4 + struct.calcsize(HEADER_FMT)

    OP_MUTATE = 1
    OP_WIGLE_RECORD = 2
    OP_WIGLE_ADMIT = 3
    OP_WIGLE_COMMIT = 4
    OP_ESP32_SENSE = 5
    OP_ALEXAI_FOLD = 6

    def __init__(self, state_dir: str | Path) -> None:
        self.dir = Path(state_dir)
        self.dir.mkdir(parents=True, exist_ok=True)
        self.manifest_path = self.dir / "MANIFEST"
        self.epoch = self._load_epoch()
        self.lsn = 0
        self.last_hash = b"\x00" * 32
        self.state: Dict[str, Any] = {}
        self._recover()

    def _load_epoch(self) -> int:
        if not self.manifest_path.exists():
            atomic_write_file(self.manifest_path, b"0\n")
            return 0
        return int(self.manifest_path.read_bytes().strip())

    def _snap_path(self, epoch: int) -> Path:
        return self.dir / f"checkpoint.{epoch:06d}.snap"

    def _wal_path(self, epoch: int) -> Path:
        return self.dir / f"wal.{epoch:06d}.log"

    def _recover(self) -> None:
        """Load base snapshot for active epoch and replay verified WAL frames."""
        snap = self._snap_path(self.epoch)
        if snap.exists():
            raw = snap.read_bytes()
            doc = json.loads(raw.decode("utf-8"))
            self.lsn = int(doc["lsn"])
            self.last_hash = bytes.fromhex(doc["last_hash"])
            self.state = dict(doc["state"])
        else:
            self.lsn = 0
            self.last_hash = hashlib.blake2b(b"AURORA_GENESIS_0X62", digest_size=32).digest()
            self.state = {
                "gate": "0x62",
                "qt": "0x55",
                "wigle_custodian": {"record": 0, "admit": 0, "commit": 0, "lossless_pq": "29/30"},
                "esp32_frames": [],
                "alexai_folds": [],
            }

        wal = self._wal_path(self.epoch)
        if not wal.exists():
            return

        with open(wal, "r+b") as f:
            valid_offset = 0
            while True:
                header_buf = f.read(self.HEADER_SIZE)
                if not header_buf:
                    break
                if len(header_buf) < self.HEADER_SIZE:
                    f.truncate(valid_offset)
                    os.fsync(f.fileno())
                    break

                crc_stored = struct.unpack("<I", header_buf[:4])[0]
                lsn, opcode, length, prev_hash = struct.unpack(self.HEADER_FMT, header_buf[4:])
                payload = f.read(length)

                if len(payload) < length:
                    f.truncate(valid_offset)
                    os.fsync(f.fileno())
                    break

                crc_calc = zlib.crc32(header_buf[4:] + payload) & 0xFFFFFFFF
                if crc_calc != crc_stored or prev_hash != self.last_hash or lsn != self.lsn + 1:
                    f.truncate(valid_offset)
                    os.fsync(f.fileno())
                    break

                self._apply_Transition(opcode, json.loads(payload.decode("utf-8")))
                self.lsn = lsn
                self.last_hash = hashlib.blake2b(
                    header_buf[4:] + payload, digest_size=32
                ).digest()
                valid_offset = f.tell()

    def _apply_Transition(self, opcode: int, delta: Dict[str, Any]) -> None:
        if opcode == self.OP_MUTATE:
            self.state.update(delta)
        elif opcode in (self.OP_WIGLE_RECORD, self.OP_WIGLE_ADMIT, self.OP_WIGLE_COMMIT):
            cust = self.state.setdefault(
                "wigle_custodian", {"record": 0, "admit": 0, "commit": 0, "lossless_pq": "29/30"}
            )
            if opcode == self.OP_WIGLE_RECORD:
                cust["record"] = int(cust.get("record", 0)) + 1
            elif opcode == self.OP_WIGLE_ADMIT:
                cust["admit"] = int(cust.get("admit", 0)) + 1
            else:
                cust["commit"] = int(cust.get("commit", 0)) + 1
            cust["last_observation"] = delta
        elif opcode == self.OP_ESP32_SENSE:
            frames = self.state.setdefault("esp32_frames", [])
            frames.append(delta)
            if len(frames) > 32:
                del frames[:-32]
        elif opcode == self.OP_ALEXAI_FOLD:
            folds = self.state.setdefault("alexai_folds", [])
            folds.append(delta)
            if len(folds) > 16:
                del folds[:-16]

    def append_wal(self, opcode: int, delta: Dict[str, Any]) -> Tuple[int, str]:
        """Append a single state transition to the active WAL and fsync."""
        payload = json.dumps(delta, sort_keys=True, separators=(",", ":")).encode("utf-8")
        next_lsn = self.lsn + 1
        body_header = struct.pack(self.HEADER_FMT, next_lsn, opcode, len(payload), self.last_hash)
        crc = zlib.crc32(body_header + payload) & 0xFFFFFFFF
        frame = struct.pack("<I", crc) + body_header + payload

        wal = self._wal_path(self.epoch)
        with open(wal, "ab") as f:
            f.write(frame)
            f.flush()
            os.fsync(f.fileno())

        self._apply_Transition(opcode, delta)
        self.lsn = next_lsn
        self.last_hash = hashlib.blake2b(body_header + payload, digest_size=32).digest()
        return self.lsn, self.last_hash.hex()

    def create_checkpoint(self) -> int:
        """Compact current state into `checkpoint.<next_epoch>.snap` and rotate MANIFEST."""
        next_epoch = self.epoch + 1
        snap_doc = {
            "epoch": next_epoch,
            "lsn": self.lsn,
            "last_hash": self.last_hash.hex(),
            "state": self.state,
        }
        snap_bytes = json.dumps(snap_doc, sort_keys=True, indent=2).encode("utf-8")
        atomic_write_file(self._snap_path(next_epoch), snap_bytes)
        atomic_write_file(self.manifest_path, f"{next_epoch}\n".encode("ascii"))

        old_wal = self._wal_path(self.epoch)
        if old_wal.exists():
            old_wal.unlink()
            fsync_dir(self.dir)

        self.epoch = next_epoch
        return self.epoch

    def execute_atomic_step(
        self,
        fn: Callable[[Dict[str, Any]], Dict[str, Any]],
        opcode: int = OP_MUTATE,
    ) -> Tuple[int, str]:
        """Run `fn` against a copy of state; on exception, reload from disk without corruption."""
        try:
            delta = fn(dict(self.state))
            return self.append_wal(opcode, delta)
        except Exception:
            self._recover()
            raise


if __name__ == "__main__":
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        eng = AuroraStateEngine(tmp)
        lsn1, h1 = eng.append_wal(
            AuroraStateEngine.OP_WIGLE_COMMIT,
            {"ssid": "Rakuten-4CE2", "rssi": -61, "fix_lat_e7": 356852000, "fix_lon_e7": 139710000},
        )
        lsn2, h2 = eng.append_wal(
            AuroraStateEngine.OP_ESP32_SENSE,
            {"node": "ADA:MEDUSA", "phase": 3, "trit": 1, "value": "836/793", "seal16": "0x0214"},
        )
        lsn3, h3 = eng.append_wal(
            AuroraStateEngine.OP_ALEXAI_FOLD,
            {"engine": "GAIA-MLX-QUANT", "coherence_pq": "407/389", "lossless_pq": "29/30"},
        )
        # Simulate torn tail byte on active WAL and verify clean auto-truncation recovery
        wal_file = eng._wal_path(eng.epoch)
        with open(wal_file, "ab") as wf:
            wf.write(b"\xde\xad\xbe")
        recovered = AuroraStateEngine(tmp)
        assert recovered.lsn == 3
        assert recovered.last_hash.hex() == h3
        assert recovered.state["wigle_custodian"]["commit"] == 1
        ep = recovered.create_checkpoint()
        assert ep == 1
        print(
            f"[aurora_storage] OK epoch={ep} lsn={recovered.lsn} hash={h3[:16]}... "
            f"custodian={recovered.state['wigle_custodian']}"
        )
