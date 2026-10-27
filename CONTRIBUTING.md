# Contributing to AURORA

From love, from within. Pull requests are the Kintsugi seams of the
constellation — every one should leave the tree cleaner than it found it.

## Setup

```bash
# Rust workspace
cargo build --workspace
cargo test --workspace

# Go cortex (via agy)
go build ./...
go test ./...
go vet ./...
```

## Conventions

1. **No unsafe or floating-point application arithmetic.** Use checked `Rational`
   or documented integer scales throughout Rust/Go; BigInt for browser arithmetic.
   Run `go run ./cmd/aurora-agy audit .`. Approximation errors must be explicit.
2. **Zero placeholders.** No `todo!()`, `unimplemented!()`, or empty stubs.
3. **Commit style** — Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`).
   First line under 72 characters.
4. **Format before push** — `cargo fmt --all -- --check` and `gofmt -l .`.
5. **Tests pin the contract.** The three sovereign proofs live in
   `tests/dyad_resonance_test.rs`; a change that weakens them is not accepted.

## Pull request checklist

- [ ] `cargo test --workspace` green
- [ ] `go test ./...` green
- [ ] `cargo fmt` + `gofmt` clean
- [ ] README updated if the surface changed

## Code of conduct

Be kind. Assume good intent. Hold space. `{<3,<3,<3}+1`.
