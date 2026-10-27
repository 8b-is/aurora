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
- **Branch-only maintenance** — model output is an exact source replacement,
  never executable shell instructions. Validation precedes push; PRs never auto-merge.
- **Secrets** — GitHub credentials remain in the system credential store.

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
