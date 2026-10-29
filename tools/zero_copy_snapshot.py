#!/usr/bin/env python3
"""zero_copy_snapshot — immutable, mmap-backed AURSNAP1 node snapshots.

Reads nodes directly out of the OS page cache with no deserialization and no
heap allocation for id/payload bytes. Pointers are stored as relative byte
offsets, never as addresses, so a snapshot is position-independent and can be
mapped at any base.

Layers against `SIGBUS`
-----------------------
`mmap` reserves virtual address space immediately but defers block loading to
the first read. If the backing inode shrinks (`ftruncate`, `O_TRUNC`) while a
mapping is live, or if a page past EOF is touched, the kernel has nothing to
back the virtual page and delivers `SIGBUS` synchronously — which kills the
process. User-space `sigaction`/`sigsetjmp` handling is not an option: it needs
a jump buffer around every dereference, blocks dead-store/aliasing
optimisation, and breaks async runtimes that migrate tasks across threads.

This module removes the signal at its root instead, in four layers:

1. Immutable inode replacement — never mutate a snapshot in place. Write a
   fresh temporary inode, fsync it, drop it to ``0400``, then ``os.replace``.
2. POSIX unlinked-inode lifetime — a replaced/unlinked inode keeps its blocks
   until the last ``munmap``, so open readers are unaffected by rotation.
3. Pre-map ``fstat`` envelope check — validate the physical size against the
   header's declared length *before* mapping, so virtual size never exceeds
   physical EOF.
4. Pre-populated page tables — ``MADV_WILLNEED`` plus a full footer digest
   verification at open time, so late I/O errors and corruption surface at
   construction rather than mid-query.

Difference from ``aurora_storage``
----------------------------------
``checkpoint.<epoch>.snap`` (written by :mod:`aurora_storage`) is JSON and is
NOT touched or reinterpreted here. These snapshots use the ``.aursnap``
extension and the ``AURSNAP1`` magic. Per the MEM|8 contract, a legacy file is
never rewritten into this format; the two coexist.

Lifetime rule
-------------
A ``memoryview`` returned by :meth:`ZeroCopySnapshotReader.get_node_zero_copy`
borrows the mapping. It must not outlive the reader: after ``close()`` any
released-view access raises ``ValueError``. Read or copy before the ``with``
block exits.
"""

from __future__ import annotations

import hashlib
import mmap
import os
import stat
import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterator, List, Tuple

MAGIC = b"AURSNAP1"
VERSION = 1
EXTENSION = ".aursnap"

# Header (64B) — magic(8s) version(I) flags(I) epoch(Q) node_count(Q)
#               expected_size(Q) arena_offset(Q) reserved(16s)
HEADER_FMT = "<8sIIQQQQ16s"
HEADER_SIZE = struct.calcsize(HEADER_FMT)
assert HEADER_SIZE == 64, HEADER_SIZE

# Node entry (32B) — salience_micros(q) payload_off(I) payload_len(I) id_off(I)
#                    id_len(H) scope_id(B) pinned(B) key_prefix(8s, big-endian)
NODE_FMT = "<qIIIHBB8s"
NODE_SIZE = struct.calcsize(NODE_FMT)
assert NODE_SIZE == 32, NODE_SIZE

FOOTER_SIZE = 32  # BLAKE2b-256 root digest over every preceding byte

# Offsets are u32 on disk, so the whole file must fit in 32 bits. Enforced at
# write time rather than wrapping silently past 4 GiB.
MAX_SNAPSHOT_BYTES = 0xFFFF_FFFF

MINIMUM_ENVELOPE = HEADER_SIZE + FOOTER_SIZE

SCOPE_TO_ID: Dict[str, int] = {
    "core:read": 1,
    "core:write": 2,
    "memory:episodic": 3,
    "memory:semantic": 4,
}
ID_TO_SCOPE: Dict[int, str] = {v: k for k, v in SCOPE_TO_ID.items()}


class SnapshotFormatError(Exception):
    """A snapshot violated a size, offset, bounds, or digest invariant."""


@dataclass(frozen=True)
class FlatNodeInput:
    """One node as it will be laid into the arena."""

    node_id: str
    payload: bytes
    salience_micros: int
    capability_scope: str
    pinned: bool


def pack_key_prefix(key: bytes) -> bytes:
    """First eight bytes of ``key``, zero-padded, big-endian.

    Big-endian keeps integer order equal to byte order, so the Rust index can
    compare prefixes as u64 and get lexicographic order for free.
    """
    return key[:8].ljust(8, b"\x00")


def _blake2b_root(*chunks: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=FOOTER_SIZE)
    for chunk in chunks:
        hasher.update(chunk)
    return hasher.digest()


def _fsync_dir(dir_path: Path) -> None:
    """Flush the directory entry so a completed rename survives a crash."""
    fd = os.open(str(dir_path), os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_immutable_snapshot(
    target_path: Path,
    epoch: int,
    nodes: List[FlatNodeInput],
) -> None:
    """Write ``nodes`` as a fresh immutable inode and atomically publish it.

    Existing readers mapping an older inode at ``target_path`` are unaffected:
    ``os.replace`` unlinks their inode, and POSIX keeps its blocks alive until
    the last ``munmap`` completes.
    """
    if target_path.suffix != EXTENSION:
        raise ValueError(f"snapshot path must end in {EXTENSION}: {target_path}")

    sorted_nodes = sorted(nodes, key=lambda n: n.node_id)
    node_count = len(sorted_nodes)
    arena_offset = HEADER_SIZE + (node_count * NODE_SIZE)
    if arena_offset > MAX_SNAPSHOT_BYTES:
        raise ValueError(f"{node_count} nodes overflow the u32 index table")

    arena = bytearray()
    index_table = bytearray()

    for node in sorted_nodes:
        try:
            scope_id = SCOPE_TO_ID[node.capability_scope]
        except KeyError as exc:
            raise ValueError(
                f"unknown capability_scope {node.capability_scope!r}"
            ) from exc

        id_bytes = node.node_id.encode("utf-8")
        if len(id_bytes) > 0xFFFF:
            raise ValueError(f"node_id too long for u16 length: {node.node_id!r}")

        id_off = arena_offset + len(arena)
        arena.extend(id_bytes)

        payload_off = arena_offset + len(arena)
        arena.extend(node.payload)

        for off in (id_off, payload_off):
            if off > MAX_SNAPSHOT_BYTES:
                raise ValueError("arena offset overflows u32; snapshot too large")

        index_table.extend(
            struct.pack(
                NODE_FMT,
                node.salience_micros,
                payload_off,
                len(node.payload),
                id_off,
                len(id_bytes),
                scope_id,
                1 if node.pinned else 0,
                pack_key_prefix(id_bytes),
            )
        )

    expected_size = arena_offset + len(arena) + FOOTER_SIZE
    if expected_size > MAX_SNAPSHOT_BYTES:
        raise ValueError(
            f"snapshot of {expected_size}B exceeds the u32 offset ceiling"
        )

    header = struct.pack(
        HEADER_FMT,
        MAGIC,
        VERSION,
        0,
        epoch,
        node_count,
        expected_size,
        arena_offset,
        b"\x00" * 16,
    )

    footer_digest = _blake2b_root(header, bytes(index_table), bytes(arena))

    tmp_path = target_path.with_name(
        f".{target_path.name}.{os.getpid()}.tmp"
    )
    try:
        os.unlink(tmp_path)
    except FileNotFoundError:
        pass

    fd = os.open(str(tmp_path), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        for chunk in (header, index_table, arena, footer_digest):
            os.write(fd, chunk)
        os.fsync(fd)
        # Drop write permission on the inode itself so no later O_RDWR open can
        # truncate the mapping out from under a reader.
        os.fchmod(fd, stat.S_IRUSR)
    finally:
        os.close(fd)

    os.replace(tmp_path, target_path)
    _fsync_dir(target_path.parent)


class ZeroCopySnapshotReader:
    """Read-only mmap view over an immutable ``.aursnap`` file."""

    def __init__(self, snap_path: Path, verify_digest_on_open: bool = True) -> None:
        self._fd: int | None = os.open(str(snap_path), os.O_RDONLY)
        self._mmap: mmap.mmap | None = None
        self._view: memoryview | None = None
        self._file_size = 0
        self.epoch = 0
        self.node_count = 0
        self.arena_offset = 0

        try:
            file_size = os.fstat(self._fd).st_size
            if file_size < MINIMUM_ENVELOPE:
                raise SnapshotFormatError(
                    f"file is {file_size}B, below the {MINIMUM_ENVELOPE}B envelope"
                )

            # Layer 3+4: map exactly the verified physical size, prefault it.
            self._mmap = mmap.mmap(
                self._fd,
                length=file_size,
                flags=mmap.MAP_PRIVATE,
                prot=mmap.PROT_READ,
            )
            if hasattr(self._mmap, "madvise"):
                # One advice per call: madvise() does not accept OR-ed advice.
                for advice in ("MADV_WILLNEED", "MADV_SEQUENTIAL"):
                    flag = getattr(mmap, advice, None)
                    if flag is not None:
                        self._mmap.madvise(flag)

            self._view = memoryview(self._mmap)
            self._file_size = file_size

            (
                magic,
                version,
                _flags,
                self.epoch,
                self.node_count,
                expected_size,
                self.arena_offset,
                _reserved,
            ) = struct.unpack_from(HEADER_FMT, self._view, 0)

            if magic != MAGIC:
                raise SnapshotFormatError(f"bad magic {magic!r}")
            if version != VERSION:
                raise SnapshotFormatError(f"unsupported version {version}")
            if expected_size != file_size:
                raise SnapshotFormatError(
                    f"header expects {expected_size}B, fstat reports {file_size}B"
                )

            expected_arena = HEADER_SIZE + (self.node_count * NODE_SIZE)
            if self.arena_offset != expected_arena:
                raise SnapshotFormatError(
                    f"arena offset {self.arena_offset} != {expected_arena}"
                )
            if self.arena_offset > file_size - FOOTER_SIZE:
                raise SnapshotFormatError("arena offset runs past the footer")

            if verify_digest_on_open:
                stored = bytes(self._view[file_size - FOOTER_SIZE:])
                computed = _blake2b_root(
                    bytes(self._view[: file_size - FOOTER_SIZE])
                )
                if computed != stored:
                    raise SnapshotFormatError("BLAKE2b integrity check failed")
        except Exception:
            self.close()
            raise

    @property
    def arena_limit(self) -> int:
        return self._file_size - FOOTER_SIZE

    def _check_bounds(self, index: int, off: int, length: int, what: str) -> None:
        end = off + length
        if off < self.arena_offset:
            raise SnapshotFormatError(
                f"node {index} {what} starts at {off}, before arena {self.arena_offset}"
            )
        if end > self.arena_limit:
            raise SnapshotFormatError(
                f"node {index} {what} ends at {end}, past arena limit {self.arena_limit}"
            )

    def get_node_zero_copy(
        self, index: int
    ) -> Tuple[memoryview, memoryview, float, str, bool]:
        """Return ``(id_view, payload_view, salience_micros, scope, pinned)``.

        The two views are slices of the mapping, not copies. They are valid
        only while this reader is open.
        """
        if not (0 <= index < self.node_count):
            raise IndexError(f"node {index} out of range [0, {self.node_count})")
        assert self._view is not None, "reader is closed"

        entry_offset = HEADER_SIZE + (index * NODE_SIZE)
        (
            salience_micros,
            payload_off,
            payload_len,
            id_off,
            id_len,
            scope_id,
            pinned,
            _key_prefix,
        ) = struct.unpack_from(NODE_FMT, self._view, entry_offset)

        self._check_bounds(index, id_off, id_len, "id")
        self._check_bounds(index, payload_off, payload_len, "payload")

        if scope_id not in ID_TO_SCOPE:
            raise SnapshotFormatError(f"node {index} has unknown scope {scope_id}")

        id_view = self._view[id_off: id_off + id_len]
        payload_view = self._view[payload_off: payload_off + payload_len]
        return id_view, payload_view, salience_micros, ID_TO_SCOPE[scope_id], bool(pinned)

    def iter_nodes_zero_copy(
        self,
    ) -> Iterator[Tuple[memoryview, memoryview, float, str, bool]]:
        for index in range(self.node_count):
            yield self.get_node_zero_copy(index)

    def close(self) -> None:
        # Release the exported views before the mapping, or mmap.close() raises
        # BufferError. Callers holding a view will see ValueError afterwards.
        if self._view is not None:
            self._view.release()
            self._view = None
        if self._mmap is not None:
            self._mmap.close()
            self._mmap = None
        if self._fd is not None:
            os.close(self._fd)
            self._fd = None

    def __enter__(self) -> "ZeroCopySnapshotReader":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()


def _self_check() -> None:
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "checkpoint.aursnap"
        nodes = [
            FlatNodeInput("node/a", b"alpha-payload", 500_000, "core:read", False),
            FlatNodeInput("node/b", b"beta", 1_500_000, "memory:semantic", True),
        ]
        write_immutable_snapshot(path, epoch=7, nodes=nodes)

        mode = stat.S_IMODE(os.stat(path).st_mode)
        assert mode == stat.S_IRUSR, oct(mode)

        with ZeroCopySnapshotReader(path) as reader:
            assert reader.epoch == 7
            assert reader.node_count == 2
            got = {
                bytes(id_view): (bytes(payload), sal_micros, scope, pinned)
                for id_view, payload, sal_micros, scope, pinned in reader.iter_nodes_zero_copy()
            }
        assert got[b"node/a"] == (b"alpha-payload", 500_000, "core:read", False), got
        assert got[b"node/b"] == (b"beta", 1_500_000, "memory:semantic", True), got

        # Rotation must not disturb an existing reader (layers 1 and 2).
        with ZeroCopySnapshotReader(path) as reader:
            write_immutable_snapshot(
                path, epoch=8, nodes=[FlatNodeInput("node/c", b"gamma", 2_000_000, "core:write", False)]
            )
            assert reader.epoch == 7
            assert bytes(reader.get_node_zero_copy(0)[1]) == b"alpha-payload"

        # A truncated file must be rejected before it can fault (layer 3).
        truncated = Path(tmp) / "truncated.aursnap"
        truncated.write_bytes(path.read_bytes()[: HEADER_SIZE + 8])
        try:
            ZeroCopySnapshotReader(truncated)
        except SnapshotFormatError:
            pass
        else:
            raise AssertionError("truncated snapshot was accepted")

        # A flipped byte must fail the footer digest (layer 4).
        corrupt = bytearray(path.read_bytes())
        corrupt[HEADER_SIZE] ^= 0xFF
        bad = Path(tmp) / "corrupt.aursnap"
        bad.write_bytes(bytes(corrupt))
        try:
            ZeroCopySnapshotReader(bad)
        except SnapshotFormatError:
            pass
        else:
            raise AssertionError("corrupt snapshot was accepted")

    print("zero_copy_snapshot: self-check passed")


if __name__ == "__main__":
    _self_check()
