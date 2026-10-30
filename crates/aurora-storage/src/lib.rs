//! Aurora storage formats.
//!
//! `mmap_index` reads the `AURSNAP1` flat snapshot written by
//! `tools/zero_copy_snapshot.py`. It is a pure function of a byte buffer: no
//! `unsafe`, no dependencies, and no file handling. Whoever owns the buffer
//! (a `Vec`, or a mapping the Python writer produced) owns the lifetime.
//!
//! Integrity note: [`mmap_index::AurSnapIndex::open`] validates magic, version,
//! declared size, and the arena offset. Node access validates slice bounds. It does **not**
//! recompute the BLAKE2b footer, because this crate carries no crypto
//! implementation. Verify the footer with the writer's tool (or a future
//! in-tree BLAKE2b) before trusting a snapshot from an untrusted source.

#![forbid(unsafe_code)]

pub mod mmap_index;
