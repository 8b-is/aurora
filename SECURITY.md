# Security Policy

AURORA treats security as a non-negotiable design law, not a bolt-on. The
substrate enforces the following by construction:

- **No unsafe Rust** — every crate is `#![forbid(unsafe_code)]`.
- **Capability accounting** — `MahakalaGuard` checks bitmask inclusion and
  accounts caller-supplied elapsed durations against a strict `<117 ms` budget.
  It is a library check, not an operating-system sandbox.
- **Loopback inference** — the Go transport validates a literal loopback IP
  before opening a socket. Redirects, proxies, remote providers and model
  downloads are not supported. GitHub publishing is a separate Git/CLI path.
- **Durable memory** — versioned snapshots, CRC32, exclusive writer locks,
  atomic replacement and synchronization; no silent legacy-file conversion.
- **Zero-copy snapshots never fault** — `.aursnap` files are written to a fresh
  inode, fsynced, reduced to `0400`, and renamed into place. Sizes are checked
  against the header before mapping and the footer BLAKE2b digest is verified
  at open, so a reader cannot receive `SIGBUS` from a shrinking inode. See
  [Memory-mapped snapshots](#memory-mapped-snapshots-sigbus).
- **Branch-only maintenance** — model output is an exact source replacement,
  never executable shell instructions. Validation precedes push; PRs never auto-merge.
- **Secrets** — GitHub credentials remain in the system credential store.

## Memory-mapped snapshots (`SIGBUS`)

`tools/zero_copy_snapshot.py` serves nodes straight from the page cache with
`mmap` + `memoryview`. A mapping reserves address space eagerly and loads blocks
lazily, so a page fault on an inode that has since shrunk — or on a page past
EOF — raises `SIGBUS`, which terminates the process rather than returning an
error to Python.

We do not try to catch it. A user-space `sigaction`/`sigsetjmp` handler would
require a jump buffer around every dereference, inhibit dead-store and aliasing
optimisation, and be unsound across threads as async runtimes migrate tasks.
Four layers remove the condition instead:

1. **Immutable inode replacement** — never mutate a snapshot in place. Write a
   new temporary inode, `fsync`, `fchmod` to `0400`, then `os.replace`.
2. **Unlinked-inode lifetime** — POSIX keeps a replaced inode's blocks alive
   until the last `munmap`, so open readers survive rotation untouched.
3. **Pre-map `fstat` envelope** — physical `st_size` must match the header's
   declared `expected_size` before mapping; virtual size never exceeds EOF.
4. **Prefault and verify** — `MADV_WILLNEED`, then a full BLAKE2b-256 footer
   check at open, so late I/O errors and corruption fail at construction, not
   mid-query.

`.aursnap` is a new format. It is not a conversion of, and never overwrites, the
JSON `checkpoint.<epoch>.snap` files or any MEM|8 file.

## Reporting a vulnerability

Do **not** open a public issue. Email `security@8b.is` with:

1. The affected repository and version (or commit).
2. A minimal reproduction.
3. The impact you believe it has.

We respond within 72 hours and treat every report as a Kintsugi seam to repair,
never a fault to hide.

## Supported versions

Only the latest `main` and the latest tagged release are supported. Fixes land
on `main` first, then are backported to the current release line.
