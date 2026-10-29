# MEM|8 in Aurora

Aurora owns two explicitly versioned MEM|8 encodings. They are **not wire-compatible**
with Smart Tree's `.aye_consciousness.m8` or the orchestrator's 79-byte WaveInt frame.
Never point Aurora at either file. Legacy Aurora snapshots also require an explicit
migration; v2 refuses them and leaves the bytes intact.

## Integer contract

- Frequencies: integer Hz; timestamps: unsigned 64-bit microseconds.
- Amplitude: `value / 1_000_000`, bounded to `[0,1]`.
- Phase: `value / 1_000_000` turns, bounded to `[0,1)`.
- Salience and validation ratios: reduced signed numerator / positive denominator.
- Rational operations reject zero denominators and overflow.
- Phoenix's `1/8` EWMA projects to millionths each step: truncation is less than
  `1/1_000_000` per update, with steady accumulated error below `8/1_000_000`.
- Phase cosine uses an integer Bhaskara approximation. Entropy uses integer square
  root and a truncated log series. These are approximations, not exact irrational
  values. Tests pin known reference points; they are not worst-case benchmarks.
- The audio fade is now integer smoothstep, with PCM16 samples.

## Files

Both formats start with an eight-byte magic and a little-endian u64 record count.
They end with IEEE CRC32 of all preceding bytes (integrity, not authentication).
Maximum file size is 64 MiB. Capacity exhaustion is an error; history is never
silently discarded.

`MEM8AU02`: each record contains an eight-byte lattice key and 32-byte tuple:
Hz u64, timestamp u64, amplitude u32, phase u32, spin u8 (`0..2`), scar u8,
coherence u16, four reserved zero bytes. Coordinate rewrites append history;
the lookup index returns the latest value. Grid bounds are checked.

`MEM8EV02`: each record contains ID u64, timestamp u64, salience numerator i64,
denominator u64, payload length u32, then UTF-8 payload. IDs start at one and
increase without gaps. A payload is at most 256 KiB. The maintenance cortex stores
actual task receipts here and recalls previous outcomes into subsequent prompts.

## Durability and recovery

Writers lock a separate `.lock` file, create a unique snapshot beside the destination,
sync the file, rename it atomically, and sync the parent directory. Wave snapshots
also reject stale writers. Loads validate the whole snapshot before changing memory.
Truncated files, invalid trits, invalid denominators, and CRC failures are errors.

After interruption, the final path contains the old or new complete snapshot on a
filesystem supporting these semantics. Orphan `.tmp` snapshots are not silently
promoted. A corrupted final snapshot requires restoration from a verified backup.
Use a local filesystem for the live memory store; replicate backups separately.

## Zero-copy snapshots (`AURSNAP1`)

A third, separate format serves reads with no deserialization and no heap
allocation. It is **not** an MEM|8 format and does not replace `MEM8AU02`,
`MEM8EV02`, or the JSON `checkpoint.<epoch>.snap`; it is written only to a
`.aursnap` path.

A 64-byte header records magic `AURSNAP1`, version, flags, epoch, node count,
`expected_size`, and the arena offset. A 32-byte index entry per node holds
salience (f64), `payload_off`, `payload_len`, `id_off` (all u32), `id_len`
(u16), scope id, pinned flag, and a digest prefix. IDs and payloads share one
UTF-8 arena; a 32-byte BLAKE2b-256 footer covers every preceding byte. Pointers
are relative offsets, so the file is position-independent. Because offsets are
u32, writers reject any snapshot above 4 GiB instead of wrapping — note this is
a wider ceiling than the 64 MiB MEM|8 limit, and a different checksum
(BLAKE2b-256, not CRC32).

Readers map exactly the `fstat`-verified size, prefault with `MADV_WILLNEED`,
and verify the footer digest before exposing `memoryview` slices. This is what
keeps a shrinking or replaced inode from delivering `SIGBUS`: the mapping can
never outrun EOF, and rotation publishes a new inode rather than truncating a
live one. Returned views borrow the mapping — release them before `close()`.

See `tools/zero_copy_snapshot.py` and the `SIGBUS` section of `SECURITY.md`.
