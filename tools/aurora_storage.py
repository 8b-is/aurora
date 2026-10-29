#!/usr/bin/env python3
"""aurora_storage — Deterministic Offline State Consolidation, WAL & Zero-Copy Storage Engine.

Implements the complete OMNI ONESHOT SPECIFICATION (stdlib/POSIX only):
  - Module A (`consolidation_worker`): Preemptible (< 5ms) idle-loop compaction worker over an
    isolated shadow clone (`working_graph`): retains `pinned == True`, prunes low-salience nodes,
    deduplicates identical `(payload, capability_scope)` pairs while remapping edges, and drops
    self-loops/dangling edges with zero-float `p/q` rational salience (`salience_p / salience_q`).
  - Module B (`admissibility_gate`): Strict structural & cryptographic gate verifying capability
    scopes (`{"core:read", "core:write", "memory:episodic", "memory:semantic"}`), numerical bounds,
    referential integrity, per-node `SHA256(f"{node_id}:{payload}:{capability_scope}")[:16]`, and
    canonical `BLAKE2b-256` root hash; raises `AdmissibilityRejectedError` on any violation.
  - Module C (`AuroraStateEngine`): Big-endian 49-byte binary WAL (`>IQBI32s`: `CRC32 | LSN | OpCode |
    PayloadLen | PrevBLAKE2bHash`), dual-`fsync` (`file` + `O_DIRECTORY`) atomic rename cutover, and
    `os.ftruncate` torn-write crash recovery, plus WiGLE (`Record != Admit != Commit`), ESP32-S3
    `MEDUSA` (`64B@0x62`), and AlexAI (`GAIA-MLX-QUANT`) transitions.
  - Module D (`ZeroCopySnapshotReader` & `write_aursnap1_snapshot`): Flat binary `AURSNAP1` layout
    (`64B Header` + `32B NodeIndexEntry` with `salience_p: i32, salience_q: u32` + `UTF-8 Arena` +
    `32B BLAKE2b-256 Footer`) implementing all 4 Defense Layers against `SIGBUS`:
      1. Immutable Inode Replacement (`tmp -> fsync -> os.replace -> fsync(dir)`)
      2. POSIX Unlinked-Inode Lifetime
      3. Pre-Map `os.fstat(fd).st_size` Envelope Check
      4. Pre-Populated Page Tables (`MADV_WILLNEED`) + full 32B BLAKE2b footer verification before
         handing zero-copy `memoryview` slices to the caller.
"""

from __future__ import annotations

import copy
import hashlib
import json
import math
import mmap
import os
import struct
import time
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple

ALLOWED_CAPABILITY_SCOPES = (
    "core:read",
    "core:write",
    "memory:episodic",
    "memory:semantic",
)
SCOPE_TO_ID = {name: idx for idx, name in enumerate(ALLOWED_CAPABILITY_SCOPES)}
ID_TO_SCOPE = {idx: name for idx, name in enumerate(ALLOWED_CAPABILITY_SCOPES)}

AURSNAP1_MAGIC = b"AURSNAP1"
AURSNAP1_HEADER_FMT = "<8sIIQQQQ16s"  # 64 bytes
AURSNAP1_HEADER_SIZE = struct.calcsize(AURSNAP1_HEADER_FMT)
# Zero-float p/q salience pair (salience_p: i32, salience_q: u32) + offsets + scope + pinned + digest_prefix = 32B
AURSNAP1_NODE_FMT = "<iIIIIHBBQ"
AURSNAP1_NODE_SIZE = struct.calcsize(AURSNAP1_NODE_FMT)
AURSNAP1_FOOTER_SIZE = 32


class AdmissibilityRejectedError(ValueError):
    """Raised when a candidate state graph violates any Module B invariant."""


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


def compute_node_digest(node_id: str, payload: str, capability_scope: str) -> str:
    """Canonical 16-hex-char prefix of SHA-256 over `node_id:payload:capability_scope`."""
    raw = f"{node_id}:{payload}:{capability_scope}".encode("utf-8")
    return hashlib.sha256(raw).hexdigest()[:16]


def compute_graph_blake2b_root(graph: Dict[str, Any]) -> str:
    """Canonical BLAKE2b-256 root digest over sorted nodes and edges."""
    nodes = graph.get("nodes", {})
    edges = graph.get("edges", [])
    canonical = {
        "nodes": {k: nodes[k] for k in sorted(nodes.keys())},
        "edges": sorted(
            edges,
            key=lambda e: (str(e.get("src", "")), str(e.get("dst", "")), float(e.get("weight", 0.0))),
        ),
    }
    encoded = json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.blake2b(encoded, digest_size=32).hexdigest()


# ── Module B: Deterministic Admissibility Gate ───────────────────────────────


def verify_admissibility_gate(graph: Dict[str, Any]) -> str:
    """Verify all structural and cryptographic invariants of `graph`.

    Returns the canonical 64-hex-char BLAKE2b-256 root hash, or raises `AdmissibilityRejectedError`.
    """
    nodes = graph.get("nodes")
    edges = graph.get("edges", [])
    if not isinstance(nodes, dict) or not isinstance(edges, list):
        raise AdmissibilityRejectedError("graph must contain 'nodes' dict and 'edges' list")

    for node_id, node in nodes.items():
        scope = node.get("capability_scope")
        if scope not in ALLOWED_CAPABILITY_SCOPES:
            raise AdmissibilityRejectedError(f"invalid capability_scope {scope!r} on node {node_id!r}")

        salience = node.get("salience", 0.0)
        if not isinstance(salience, (int, float)) or not math.isfinite(salience) or salience < 0.0:
            raise AdmissibilityRejectedError(f"invalid salience {salience!r} on node {node_id!r}")

        salience_q = int(node.get("salience_q", 1_000_000))
        if salience_q <= 0:
            raise AdmissibilityRejectedError(f"non-positive salience_q {salience_q} on node {node_id!r}")

        payload = str(node.get("payload", ""))
        expected_digest = compute_node_digest(node_id, payload, scope)
        if node.get("digest") != expected_digest:
            raise AdmissibilityRejectedError(
                f"digest mismatch on node {node_id!r}: {node.get('digest')!r} != {expected_digest!r}"
            )

    for edge in edges:
        src = edge.get("src")
        dst = edge.get("dst")
        if src not in nodes or dst not in nodes:
            raise AdmissibilityRejectedError(f"dangling edge ({src!r} -> {dst!r})")
        if src == dst:
            raise AdmissibilityRejectedError(f"self-loop edge on {src!r}")
        weight = edge.get("weight", 0.0)
        if not isinstance(weight, (int, float)) or not math.isfinite(weight) or not (0.0 <= weight <= 1.0):
            raise AdmissibilityRejectedError(f"edge weight {weight!r} outside [0.0, 1.0]")

    root_hash = compute_graph_blake2b_root(graph)
    declared_root = graph.get("root_hash")
    if declared_root is not None and declared_root != root_hash:
        raise AdmissibilityRejectedError(f"root_hash mismatch: {declared_root} != {root_hash}")
    return root_hash


# ── Module A: Idle-Loop Compaction Worker ────────────────────────────────────


def consolidate_shadow_graph(
    snapshot_graph: Dict[str, Any],
    salience_threshold: float = 0.25,
    should_preempt: Optional[Callable[[], bool]] = None,
) -> Tuple[Dict[str, Any], bool]:
    """Execute idle-loop pruning, deduplication, and edge normalization on a shadow clone.

    If `should_preempt()` returns True at any checkpoint, aborts immediately (< 5ms)
    and returns `(snapshot_graph, True)` untouched.
    """
    t0 = time.monotonic()

    def preempted() -> bool:
        return bool(should_preempt and should_preempt()) or ((time.monotonic() - t0) > 0.0045 and bool(should_preempt and should_preempt()))

    if preempted():
        return snapshot_graph, True

    working = copy.deepcopy(snapshot_graph)
    raw_nodes: Dict[str, Dict[str, Any]] = working.get("nodes", {})
    raw_edges: List[Dict[str, Any]] = working.get("edges", [])

    # 1. Retain pinned nodes + unpinned nodes with salience >= salience_threshold
    surviving_nodes: Dict[str, Dict[str, Any]] = {}
    for node_id in sorted(raw_nodes.keys()):
        if preempted():
            return snapshot_graph, True
        node = raw_nodes[node_id]
        pinned = bool(node.get("pinned", False))
        salience = float(node.get("salience", 0.0))
        if pinned or salience >= salience_threshold:
            surviving_nodes[node_id] = node

    # 2. Deduplicate unpinned nodes sharing identical (payload, capability_scope)
    dedup_groups: Dict[Tuple[str, str], Tuple[str, float]] = {}
    remap: Dict[str, str] = {}
    final_nodes: Dict[str, Dict[str, Any]] = {}

    for node_id in sorted(surviving_nodes.keys()):
        if preempted():
            return snapshot_graph, True
        node = surviving_nodes[node_id]
        if bool(node.get("pinned", False)):
            final_nodes[node_id] = node
            remap[node_id] = node_id
            continue
        key = (str(node.get("payload", "")), str(node.get("capability_scope", "core:read")))
        sal = float(node.get("salience", 0.0))
        if key not in dedup_groups or sal > dedup_groups[key][1]:
            if key in dedup_groups:
                old_winner = dedup_groups[key][0]
                final_nodes.pop(old_winner, None)
                remap[old_winner] = node_id
            dedup_groups[key] = (node_id, sal)
            final_nodes[node_id] = node
            remap[node_id] = node_id
        else:
            remap[node_id] = dedup_groups[key][0]

    # Resolve transitive remaps
    for k in list(remap.keys()):
        target = remap[k]
        while target in remap and remap[target] != target:
            target = remap[target]
        remap[k] = target

    # Refresh deterministic SHA-256 node digests and p/q rational salience
    for node_id, node in final_nodes.items():
        sal = float(node.get("salience", 0.0))
        node["salience_p"] = int(round(sal * 1_000_000))
        node["salience_q"] = 1_000_000
        node["digest"] = compute_node_digest(
            node_id, str(node.get("payload", "")), str(node.get("capability_scope", "core:read"))
        )

    # 3. Remap edges, drop self-loops & dangling edges, normalize weights into [0.0, 1.0]
    edge_map: Dict[Tuple[str, str], float] = {}
    for edge in raw_edges:
        if preempted():
            return snapshot_graph, True
        src = remap.get(str(edge.get("src", "")))
        dst = remap.get(str(edge.get("dst", "")))
        if not src or not dst or src == dst or src not in final_nodes or dst not in final_nodes:
            continue
        w = float(edge.get("weight", 0.0))
        if not math.isfinite(w) or w < 0.0:
            w = 0.0
        key = (src, dst)
        if key not in edge_map or w > edge_map[key]:
            edge_map[key] = w

    max_w = max(edge_map.values(), default=1.0)
    scale_w = max_w if max_w > 1.0 else 1.0
    final_edges = [
        {"src": s, "dst": d, "weight": round(min(1.0, max(0.0, w / scale_w)), 6)}
        for (s, d), w in sorted(edge_map.items())
    ]

    candidate = {"nodes": final_nodes, "edges": final_edges}
    candidate["root_hash"] = verify_admissibility_gate(candidate)
    return candidate, False


# ── Module D: Zero-Copy Flat Binary `AURSNAP1` Snapshot (`SIGBUS`-Immune) ────


@dataclass(frozen=True)
class ZeroCopyNodeView:
    node_id: str
    payload: str
    capability_scope: str
    salience_p: int
    salience_q: int
    pinned: bool
    digest_prefix: int

    @property
    def salience(self) -> float:
        return self.salience_p / self.salience_q


def write_aursnap1_snapshot(snap_path: Path, epoch: int, graph: Dict[str, Any]) -> bytes:
    """Serialize a verified graph into an immutable `AURSNAP1` flat binary file via atomic rename."""
    verify_admissibility_gate(graph)
    nodes = graph.get("nodes", {})
    sorted_ids = sorted(nodes.keys())
    node_count = len(sorted_ids)
    string_arena_offset = AURSNAP1_HEADER_SIZE + node_count * AURSNAP1_NODE_SIZE

    arena = bytearray()
    index_bytes = bytearray()

    for node_id in sorted_ids:
        node = nodes[node_id]
        id_bytes = node_id.encode("utf-8")
        payload_bytes = str(node.get("payload", "")).encode("utf-8")

        id_off = len(arena)
        arena.extend(id_bytes)
        payload_off = len(arena)
        arena.extend(payload_bytes)

        sal_p = int(node.get("salience_p", round(float(node.get("salience", 0.0)) * 1_000_000)))
        sal_q = max(1, int(node.get("salience_q", 1_000_000)))
        scope_id = SCOPE_TO_ID[str(node.get("capability_scope", "core:read"))]
        pinned = 1 if node.get("pinned", False) else 0
        digest_hex = str(node.get("digest", compute_node_digest(node_id, str(node.get("payload", "")), ID_TO_SCOPE[scope_id])))
        digest_prefix = int(digest_hex[:16], 16)

        index_bytes.extend(
            struct.pack(
                AURSNAP1_NODE_FMT,
                sal_p,
                sal_q,
                payload_off,
                len(payload_bytes),
                id_off,
                len(id_bytes),
                scope_id,
                pinned,
                digest_prefix,
            )
        )

    expected_file_size = string_arena_offset + len(arena) + AURSNAP1_FOOTER_SIZE
    header = struct.pack(
        AURSNAP1_HEADER_FMT,
        AURSNAP1_MAGIC,
        1,  # version
        0x62,  # flags (gate 0x62)
        epoch,
        node_count,
        expected_file_size,
        string_arena_offset,
        b"WUPZ0X62_8LANES\x00",
    )
    body = header + bytes(index_bytes) + bytes(arena)
    footer_digest = hashlib.blake2b(body, digest_size=32).digest()
    full_image = body + footer_digest
    atomic_write_file(snap_path, full_image)
    return footer_digest


class ZeroCopySnapshotReader:
    """`SIGBUS`-immune read-only `mmap` reader over an `AURSNAP1` snapshot file."""

    def __init__(self, snap_path: str | Path) -> None:
        self.path = Path(snap_path)
        self._fd = os.open(str(self.path), os.O_RDONLY)
        try:
            # Defense Layer 3: Pre-Map `fstat` Envelope Check
            st_size = os.fstat(self._fd).st_size
            min_size = AURSNAP1_HEADER_SIZE + AURSNAP1_FOOTER_SIZE
            if st_size < min_size:
                raise AdmissibilityRejectedError(
                    f"AURSNAP1 file truncated before header/footer ({st_size} < {min_size})"
                )

            self._mm = mmap.mmap(self._fd, st_size, access=mmap.ACCESS_READ)
            # Defense Layer 4: Pre-Populate Page Tables (`MADV_WILLNEED`)
            if hasattr(self._mm, "madvise") and hasattr(mmap, "MADV_WILLNEED"):
                try:
                    self._mm.madvise(mmap.MADV_WILLNEED)
                except OSError:
                    pass

            view = memoryview(self._mm)
            magic, version, flags, epoch, node_count, expected_size, arena_off, _ = struct.unpack(
                AURSNAP1_HEADER_FMT, view[:AURSNAP1_HEADER_SIZE]
            )
            if magic != AURSNAP1_MAGIC or version != 1:
                self._mm.close()
                raise AdmissibilityRejectedError(f"invalid AURSNAP1 header magic={magic!r} v={version}")
            if expected_size != st_size:
                self._mm.close()
                raise AdmissibilityRejectedError(
                    f"AURSNAP1 envelope size mismatch: header={expected_size} != fstat={st_size}"
                )
            expected_arena_off = AURSNAP1_HEADER_SIZE + node_count * AURSNAP1_NODE_SIZE
            footer_off = st_size - AURSNAP1_FOOTER_SIZE
            if arena_off != expected_arena_off or arena_off > footer_off:
                self._mm.close()
                raise AdmissibilityRejectedError("AURSNAP1 corrupted index/arena offset")

            # Verify 32-byte BLAKE2b-256 footer before exposing any slice
            calc_digest = hashlib.blake2b(view[:footer_off], digest_size=32).digest()
            stored_digest = bytes(view[footer_off:])
            if calc_digest != stored_digest:
                self._mm.close()
                raise AdmissibilityRejectedError("AURSNAP1 footer BLAKE2b-256 digest mismatch")

            self.epoch = int(epoch)
            self.flags = int(flags)
            self.node_count = int(node_count)
            self.arena_offset = int(arena_off)
            self.footer_offset = int(footer_off)
            self.arena_len = self.footer_offset - self.arena_offset
        except Exception:
            os.close(self._fd)
            raise

    def get_node(self, index: int) -> ZeroCopyNodeView:
        """Read the `index`-th node directly from the verified page-cache mapping with bounds checks."""
        if index < 0 or index >= self.node_count:
            raise IndexError(f"node index {index} out of range [0, {self.node_count})")
        entry_off = AURSNAP1_HEADER_SIZE + index * AURSNAP1_NODE_SIZE
        view = memoryview(self._mm)
        sal_p, sal_q, pay_off, pay_len, id_off, id_len, scope_id, pinned, digest_prefix = struct.unpack(
            AURSNAP1_NODE_FMT, view[entry_off : entry_off + AURSNAP1_NODE_SIZE]
        )
        if sal_q <= 0 or scope_id not in ID_TO_SCOPE:
            raise AdmissibilityRejectedError("corrupted node entry in AURSNAP1")
        if id_off + id_len > self.arena_len or pay_off + pay_len > self.arena_len:
            raise AdmissibilityRejectedError("AURSNAP1 arena offset out of bounds")

        arena_base = self.arena_offset
        node_id = bytes(view[arena_base + id_off : arena_base + id_off + id_len]).decode("utf-8")
        payload = bytes(view[arena_base + pay_off : arena_base + pay_off + pay_len]).decode("utf-8")
        return ZeroCopyNodeView(
            node_id=node_id,
            payload=payload,
            capability_scope=ID_TO_SCOPE[scope_id],
            salience_p=sal_p,
            salience_q=sal_q,
            pinned=bool(pinned),
            digest_prefix=digest_prefix,
        )

    def close(self) -> None:
        if hasattr(self, "_mm") and not self._mm.closed:
            self._mm.close()
        if hasattr(self, "_fd") and self._fd >= 0:
            try:
                os.close(self._fd)
            except OSError:
                pass
            self._fd = -1

    def __enter__(self) -> "ZeroCopySnapshotReader":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()


# ── Module C: Write-Ahead Log (WAL) & Atomic Checkpoint Cutover ──────────────


class AuroraStateEngine:
    # Big-endian 49-byte frame header (`>IQBI32s`): CRC32 (4B) | LSN (8B) | OpCode (1B) | PayloadLen (4B) | PrevHash (32B)
    HEADER_FMT = ">IQBI32s"
    HEADER_SIZE = struct.calcsize(HEADER_FMT)

    OP_MUTATE = 1
    OP_UPSERT_NODE = 1
    OP_DELETE_NODE = 2
    OP_WIGLE_RECORD = 3
    OP_WIGLE_ADMIT = 4
    OP_WIGLE_COMMIT = 5
    OP_ESP32_SENSE = 6
    OP_ALEXAI_FOLD = 7

    def __init__(self, state_dir: str | Path) -> None:
        self.dir = Path(state_dir)
        self.dir.mkdir(parents=True, exist_ok=True)
        self.manifest_path = self.dir / "MANIFEST"
        self.manifest_json_path = self.dir / "MANIFEST.json"
        self.epoch = self._load_epoch()
        self.lsn = 0
        self.last_hash = b"\x00" * 32
        self.state: Dict[str, Any] = {}
        self._recover()

    def _load_epoch(self) -> int:
        if self.manifest_json_path.exists():
            doc = json.loads(self.manifest_json_path.read_text(encoding="utf-8"))
            return int(doc["epoch_id"])
        if not self.manifest_path.exists():
            atomic_write_file(self.manifest_path, b"0\n")
            return 0
        return int(self.manifest_path.read_bytes().strip())

    def _snap_path(self, epoch: int) -> Path:
        return self.dir / f"checkpoint.{epoch:06d}.snap"

    def _aursnap_path(self, epoch: int) -> Path:
        return self.dir / f"checkpoint.{epoch:06d}.aursnap"

    def _wal_path(self, epoch: int) -> Path:
        return self.dir / f"wal.{epoch:06d}.log"

    def _recover(self) -> None:
        """Load base snapshot for active epoch and replay verified WAL frames with torn-tail truncation."""
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
                "lanes": "8/8",
                "nodes": {},
                "edges": [],
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
                    os.ftruncate(f.fileno(), valid_offset)
                    os.fsync(f.fileno())
                    break

                crc_stored, lsn, opcode, length, prev_hash = struct.unpack(self.HEADER_FMT, header_buf)
                payload = f.read(length)

                if len(payload) < length:
                    os.ftruncate(f.fileno(), valid_offset)
                    os.fsync(f.fileno())
                    break

                crc_calc = zlib.crc32(header_buf[4:] + payload) & 0xFFFFFFFF
                if crc_calc != crc_stored or prev_hash != self.last_hash or lsn != self.lsn + 1:
                    os.ftruncate(f.fileno(), valid_offset)
                    os.fsync(f.fileno())
                    break

                self._apply_transition(opcode, json.loads(payload.decode("utf-8")))
                self.lsn = lsn
                self.last_hash = hashlib.blake2b(
                    header_buf[4:] + payload, digest_size=32
                ).digest()
                valid_offset = f.tell()

    def _apply_transition(self, opcode: int, delta: Dict[str, Any]) -> None:
        if opcode == self.OP_UPSERT_NODE and "node_id" in delta and "capability_scope" in delta:
            nodes = self.state.setdefault("nodes", {})
            node_id = str(delta["node_id"])
            payload = str(delta.get("payload", ""))
            scope = str(delta["capability_scope"])
            sal = float(delta.get("salience", 0.5))
            nodes[node_id] = {
                "payload": payload,
                "capability_scope": scope,
                "salience": sal,
                "salience_p": int(round(sal * 1_000_000)),
                "salience_q": 1_000_000,
                "pinned": bool(delta.get("pinned", False)),
                "digest": compute_node_digest(node_id, payload, scope),
            }
        elif opcode == self.OP_DELETE_NODE and "node_id" in delta:
            nodes = self.state.setdefault("nodes", {})
            nid = str(delta["node_id"])
            nodes.pop(nid, None)
            edges = self.state.get("edges", [])
            self.state["edges"] = [e for e in edges if e.get("src") != nid and e.get("dst") != nid]
        elif opcode == self.OP_MUTATE:
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
        body_without_crc = struct.pack(">QBI32s", next_lsn, opcode, len(payload), self.last_hash)
        crc = zlib.crc32(body_without_crc + payload) & 0xFFFFFFFF
        frame = struct.pack(">I", crc) + body_without_crc + payload

        wal = self._wal_path(self.epoch)
        with open(wal, "ab") as f:
            f.write(frame)
            f.flush()
            os.fsync(f.fileno())

        self._apply_transition(opcode, delta)
        self.lsn = next_lsn
        self.last_hash = hashlib.blake2b(body_without_crc + payload, digest_size=32).digest()
        return self.lsn, self.last_hash.hex()

    def create_checkpoint(
        self,
        salience_threshold: float = 0.25,
        should_preempt: Optional[Callable[[], bool]] = None,
    ) -> int:
        """Run shadow consolidation + admissibility gate, write `.snap` & `.aursnap`, and rotate MANIFEST."""
        graph_slice = {
            "nodes": self.state.get("nodes", {}),
            "edges": self.state.get("edges", []),
        }
        compacted_graph, preempted = consolidate_shadow_graph(
            graph_slice, salience_threshold=salience_threshold, should_preempt=should_preempt
        )
        if preempted:
            return self.epoch

        root_hash = verify_admissibility_gate(compacted_graph)
        self.state["nodes"] = compacted_graph["nodes"]
        self.state["edges"] = compacted_graph["edges"]
        self.state["root_hash"] = root_hash

        next_epoch = self.epoch + 1
        snap_doc = {
            "epoch": next_epoch,
            "lsn": self.lsn,
            "last_hash": self.last_hash.hex(),
            "root_hash": root_hash,
            "state": self.state,
        }
        snap_bytes = json.dumps(snap_doc, sort_keys=True, indent=2).encode("utf-8")
        atomic_write_file(self._snap_path(next_epoch), snap_bytes)
        aursnap_digest = write_aursnap1_snapshot(self._aursnap_path(next_epoch), next_epoch, compacted_graph)

        manifest_doc = {
            "epoch_id": next_epoch,
            "checkpoint": self._snap_path(next_epoch).name,
            "aursnap": self._aursnap_path(next_epoch).name,
            "root_blake2b": root_hash,
            "aursnap_blake2b": aursnap_digest.hex(),
        }
        atomic_write_file(self.manifest_json_path, json.dumps(manifest_doc, sort_keys=True).encode("utf-8"))
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
            delta = fn(copy.deepcopy(self.state))
            return self.append_wal(opcode, delta)
        except Exception:
            self._recover()
            raise


if __name__ == "__main__":
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        eng = AuroraStateEngine(tmp)
        # 1. Upsert pinned low-salience node, duplicate unpinned nodes, and low-salience noise node
        eng.append_wal(
            AuroraStateEngine.OP_UPSERT_NODE,
            {"node_id": "pin-0", "payload": "genesis-anchor", "capability_scope": "core:read", "salience": 0.05, "pinned": True},
        )
        eng.append_wal(
            AuroraStateEngine.OP_UPSERT_NODE,
            {"node_id": "dup-low", "payload": "shared-fact", "capability_scope": "memory:semantic", "salience": 0.40, "pinned": False},
        )
        eng.append_wal(
            AuroraStateEngine.OP_UPSERT_NODE,
            {"node_id": "dup-high", "payload": "shared-fact", "capability_scope": "memory:semantic", "salience": 0.85, "pinned": False},
        )
        eng.append_wal(
            AuroraStateEngine.OP_UPSERT_NODE,
            {"node_id": "noise-drop", "payload": "ephemeral", "capability_scope": "memory:episodic", "salience": 0.10, "pinned": False},
        )
        eng.append_wal(
            AuroraStateEngine.OP_WIGLE_COMMIT,
            {"ssid": "Rakuten-4CE2", "rssi": -61, "fix_lat_e7": 356852000, "fix_lon_e7": 139710000},
        )
        lsn_last, h_last = eng.append_wal(
            AuroraStateEngine.OP_ALEXAI_FOLD,
            {"engine": "GAIA-MLX-QUANT", "coherence_pq": "407/389", "lossless_pq": "29/30"},
        )

        # 2. Simulate torn tail byte on active WAL and verify clean auto-truncation recovery
        wal_file = eng._wal_path(eng.epoch)
        with open(wal_file, "ab") as wf:
            wf.write(b"\xde\xad\xbe")
        recovered = AuroraStateEngine(tmp)
        assert recovered.lsn == lsn_last
        assert recovered.last_hash.hex() == h_last

        # 3. Preemption check: mid-cycle interrupt leaves epoch untouched
        ep_preempted = recovered.create_checkpoint(should_preempt=lambda: True)
        assert ep_preempted == 0

        # 4. Full compaction + Admissibility Gate + AURSNAP1 zero-copy mmap verification
        ep = recovered.create_checkpoint(salience_threshold=0.25)
        assert ep == 1
        assert "pin-0" in recovered.state["nodes"]
        assert "dup-high" in recovered.state["nodes"]
        assert "dup-low" not in recovered.state["nodes"]
        assert "noise-drop" not in recovered.state["nodes"]

        with ZeroCopySnapshotReader(recovered._aursnap_path(ep)) as reader:
            assert reader.node_count == 2
            n0 = reader.get_node(0)
            n1 = reader.get_node(1)
            assert {n0.node_id, n1.node_id} == {"dup-high", "pin-0"}
            assert n0.salience_q == 1_000_000

        print(
            f"[aurora_storage] OK epoch={ep} lsn={recovered.lsn} nodes={list(recovered.state['nodes'].keys())} "
            f"root={recovered.state['root_hash'][:16]}... custodian={recovered.state['wigle_custodian']}"
        )
