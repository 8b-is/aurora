# Security Policy

AURORA treats security as a non-negotiable design law, not a bolt-on. The
substrate enforces the following by construction:

- **No unsafe Rust** — every crate is `#![forbid(unsafe_code)]`.
- **Capability attenuation** — `MahakalaGuard` rejects any requested capability
  not covered by the granted one, under a strict `<117 ms` budget.
- **No secrets in-repo** — `OPENROUTER_API_KEY` is read from the environment,
  never committed. `.aye_consciousness.m8` and `Aurora.toml` are git-ignored
  where they hold local state.

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
