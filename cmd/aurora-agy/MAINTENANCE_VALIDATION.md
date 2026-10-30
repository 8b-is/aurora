Rust maintenance validation scope
=================================

Rust candidates must resolve to the package at their nearest Cargo.toml via
`cargo metadata --format-version 1 --no-deps --offline`. That exact manifest
and package ID must appear among a configured plan's workspace members.
Default plans are the existing root, rust, core, and wupz-rs workspaces.
Unresolved, excluded, and orphan sources are skipped before model inference.
Sources included across package boundaries are conservatively unsupported.

Operators can opt an excluded package into a standalone plan in its repository
configuration, for example:

    "standalone_rust_manifests": ["retired/Cargo.toml"]

This is an explicit manifest allowlist, not a model-provided command. Cargo must
still resolve the package successfully. It does not change workspace membership.
The worker runs `cargo test --manifest-path <manifest> --package <name> --offline`
at baseline and again after the proposal, in addition to existing validation.
A successful root-workspace check alone never authorizes a Rust proposal.

Receipts add `validation_scope_version` and `rust_checks` with the manifest,
package name, Cargo package ID, baseline/candidate phase, exact command, and
success flag. Only successful commands increment checks_passed. Legacy recovery
receipts without the new scope version require manual review instead of automatic
publication. Existing MEM8 serialization is untouched.

Regression coverage supplied
----------------------------

`TestRustValidationScope` constructs a real, dependency-free Cargo workspace with
an active package and an excluded sibling. It deletes ResourceSample.cpu_pct
while leaving a field read and test initializer. Root tests must pass, uncovered
validation must reject the proposal, and explicit standalone tests must fail.
The fixture also checks valid member coverage, successful standalone baseline,
receipt JSON, orphan rejection, and malformed-manifest rejection.
`TestSafeStandaloneManifest` checks configuration path boundaries.
The Cargo fixture skips explicitly if Cargo is unavailable.

Verification in the authoring environment
----------------------------------------

RUN: `git diff --check` (passed).
NOT RUN: Go tests, vet, gofmt, Cargo tests/clippy/fmt, or Aurora audit: neither Go
nor Cargo is installed in the authoring environment. The patch requires gofmt
before merge. No runtime or production validation is claimed.

Required checks on a machine with toolchains:

    gofmt -w cmd/aurora-agy/maintenance.go cmd/aurora-agy/maintenance_rust.go cmd/aurora-agy/maintenance_rust_test.go
    go test ./...
    go vet ./...
    cargo test --workspace --offline
    cargo clippy --workspace --all-targets --offline -- -D warnings
    cargo fmt --all -- --check
    go run ./cmd/aurora-agy audit .
    gofmt -l cmd

No crabcc manifests or AURSNAP1 compatibility behavior are changed.
