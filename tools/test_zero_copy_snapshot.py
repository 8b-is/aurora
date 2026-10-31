"""Storage regressions: borrowed lifetimes, atomic publication and format parity."""

import hashlib
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from zero_copy_snapshot import (
    FlatNodeInput, SnapshotFormatError, ZeroCopySnapshotReader,
    write_immutable_snapshot,
)

NODES = [
    FlatNodeInput("node/a", b"alpha\x00payload", -500_000, "core:read", False),
    FlatNodeInput("node/β", b"\xffbeta", 5_000_000_000, "memory:semantic", True),
]
GOLDEN = Path(__file__).resolve().parents[1] / "tests/fixtures/aursnap1.hex"


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "test.aursnap"
        write_immutable_snapshot(self.path, 7, NODES)

    def test_python_and_rust_share_exact_format(self):
        self.assertEqual(self.path.read_bytes(), bytes.fromhex(GOLDEN.read_text()))
        with ZeroCopySnapshotReader(self.path) as reader:
            for i, expected in enumerate(NODES):
                node_id, payload, salience, scope, pinned = reader.get_node_zero_copy(i)
                self.assertEqual((bytes(node_id).decode(), bytes(payload), salience, scope, pinned),
                                 (expected.node_id, expected.payload, expected.salience_micros,
                                  expected.capability_scope, expected.pinned))

    def test_descendant_views_survive_close_and_rotation(self):
        reader = ZeroCopySnapshotReader(self.path)
        node_id, payload, *_ = reader.get_node_zero_copy(0)
        child = payload[2:]
        node_id.release()
        payload.release()
        reader.close()
        reader.close()
        self.assertIsNone(reader._fd)
        write_immutable_snapshot(self.path, 8, [])
        self.assertEqual(bytes(child), b"pha\x00payload")
        child.release()
        with self.assertRaises(ValueError):
            reader.get_node_zero_copy(0)

    def test_short_writes_are_completed(self):
        real_write = os.write
        with patch("zero_copy_snapshot.os.write", side_effect=lambda fd, data: real_write(fd, data[:3])):
            write_immutable_snapshot(self.path, 7, NODES)
        self.assertEqual(self.path.read_bytes(), bytes.fromhex(GOLDEN.read_text()))

    def test_failed_write_preserves_published_file_and_cleans_temp(self):
        original = self.path.read_bytes()
        with patch("zero_copy_snapshot.os.write", return_value=0):
            with self.assertRaises(OSError):
                write_immutable_snapshot(self.path, 8, [])
        self.assertEqual(self.path.read_bytes(), original)
        self.assertEqual(list(self.path.parent.iterdir()), [self.path])

    def test_corruption_reports_format_error(self):
        for offset in (0, 12, 48, -1):
            with self.subTest(offset=offset):
                data = bytearray(bytes.fromhex(GOLDEN.read_text()))
                data[offset] ^= 1
                bad = self.path.parent / "bad.aursnap"
                bad.write_bytes(data)
                with self.assertRaises(SnapshotFormatError):
                    ZeroCopySnapshotReader(bad)

    def test_invalid_node_fields_are_rejected_with_valid_digest(self):
        for offset in (64 + 16, 64 + 22, 64 + 23):
            with self.subTest(offset=offset):
                data = bytearray(bytes.fromhex(GOLDEN.read_text()))
                data[offset] = 255
                data[-32:] = hashlib.blake2b(data[:-32], digest_size=32).digest()
                bad = self.path.parent / "bad.aursnap"
                bad.write_bytes(data)
                with ZeroCopySnapshotReader(bad) as reader:
                    with self.assertRaises(SnapshotFormatError):
                        reader.get_node_zero_copy(0)


if __name__ == "__main__":
    unittest.main()
