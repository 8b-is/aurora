# Aurora engineering contract

- Model inference targets literal loopback only; no cloud fallback or remote model keys.
- Application arithmetic is checked integer, reduced p/q, or explicitly scaled integer.
  Browser arithmetic uses BigInt; rendering receives integer coordinates. State the
  approximation and rounding rule when replacing a transcendental calculation.
- No third-party Rust or Go dependencies without demonstrating a stdlib blocker.
- Never overwrite or reinterpret a legacy MEM|8 file. Preserve errors and verify
  format, lengths, checksum, bounds and denominators before mutating state.
- Changes must fix a real issue. Do not generate unrelated audit functions or
  artificial benchmark wins. Do not weaken tests to obtain a passing result.
- Publication uses isolated clones, agent branches and pull requests. Never modify
  a shared checkout, force-push, or auto-merge.
- Required checks: `cargo test --workspace --offline`,
  `cargo clippy --workspace --all-targets --offline -- -D warnings`,
  `cargo fmt --all -- --check`, `go test ./...`, `go vet ./...`,
  `go run ./cmd/aurora-agy audit .`, and clean `gofmt -l cmd` output.
