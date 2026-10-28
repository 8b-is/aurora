# 🌌 AURORA

Local-first companion and repository-maintenance research for the 8b-is family.
Rust memory and arithmetic; Go agent coordination; locally installed models.

![AURORA hero](brand/aurora-hero.svg)

## Working capabilities

- Checked, reduced `p/q` arithmetic (`aurora-core::rational`, including exact rational solvers for Zeno's four motion paradoxes) and explicit integer-scaled wave state.
- `8b-is/wupz` C ABI v1 compatible 32-byte `WaveMemoryTuple::to_wupz_bytes` serialization, 4-way unrolled `wupz_xor_fold` and `wupz_popcount`, 64-bit SWAR `wupz_byte_diff`, `0x62`-gated 32-byte frame sealing (`seal_wupz_frame_32b`), and `BFP-64` bitplane fold packing (`pack_fold_bfp64` / `unpack_fold_bfp64` at `qt:55`).
- Versioned, checksum-verified MEM|8 wave history and semantic task receipts.
- Integer EWMA dynamics in `aurora-phoenix` and priority-driven fault recovery (`hamilton_executive_recovery` for `1201`/`1202` executive load shedding) in `aurora-dyad`.
- Loopback-only Ollama inference, defaulting to local `granite4.2:30b`.
- A serial maintenance worker: isolated clone → baseline tests → remembered context
  → bounded model proposal → no-float check → repository tests → agent branch → PR.
- Exact-match edits to existing Rust/Go source. Existing test modules, test files,
  build configuration, Git commands, and publishing destinations are not model-controlled.
- Honest outcomes: no-change, dirty checkout, blocked validation, validated commit,
  pushed branch, and open PR are recorded separately. There is no automatic merge.

The ternary network is an integer primitive, **not a pretrained language model**.
Packing uses five trits per byte; there is no hardcoded speedup or compression score.
The browser visuals and `breathe`/`dyad` commands are labelled demonstrations.
No code establishes consciousness or infers a person's feelings from a ternary value.

## Build and check

Requires Rust supporting `std::fs::File::try_lock` (1.89+), Go 1.22+, Git,
and, for the Linux maintenance worker, coreutils `timeout`, util-linux `flock`,
and authenticated GitHub CLI. Rust and Go modules have no external dependencies.

```sh
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo fmt --all -- --check
go test ./...
go vet ./...
go run ./cmd/aurora-agy audit .
cargo build --release -p aurora-cli
go build -o target/aurora-agy ./cmd/aurora-agy
```

The lexical no-float guard checks Rust/Go/JS source and inline HTML scripts.
It supplements compiler/tests; it does not prove arithmetic inside external tools,
GPU kernels, models, the browser, or the operating system. Browser application
arithmetic uses BigInt, converting integer coordinates at the rendering boundary.

## Local inference and memory

```sh
aurora-agy status
aurora-agy cortex
aurora-agy chat 'Explain a checked p/q addition in one sentence.'
aurora-agy hop +-+
printf '%s' '{"repo":"aurora","status":"tests_passed"}' | aurora memory append /var/lib/aurora/example.m8 1 1
aurora memory recall /var/lib/aurora/example.m8 8
```

`AURORA_ENDPOINT` and `AURORA_MODEL` select the local chat backend. The endpoint
must be HTTP on a literal loopback address (localhost is normalized to loopback).
Redirects and HTTP proxies are disabled. Models must already have local weights;
no cloud fallback or automatic model download exists. Generation uses an 8192-token
context, integer sampling parameters, a bounded output, and a request deadline.
Ollama/model internals are **not** asserted to use exclusively integer arithmetic.

See [the MEM|8 v2 format and recovery contract](docs/memory-v2.md). Older `.m8`
files are never silently interpreted as v2 or overwritten. Task memories are fed
back into later work, rather than merely written to a folder named `mem8`.

## Continuous maintenance

Copy `deploy/maintenance.example.json` to `/etc/aurora/maintenance.json`, adjust the
repository list, and install the two built binaries plus the systemd unit.

```sh
aurora-agy maintain /etc/aurora/maintenance.json --once
systemctl enable --now aurora-maintenance.service
```

Repositories are explicit, exact `8b-is` origins. User checkouts are read-only to
the worker. Dirty checkouts are skipped (`.st/` metadata is ignored). One open
Aurora agent PR per repository prevents repeated proposals while review is pending.
Cargo, Go, C and Swift validation lanes are detected; missing tools and failed tests
block publication. Agent branches use `alexiai/aurora-*`; pushes never force-update
or target the default branch. GitHub transport is for Git/PR publication only;
language-model requests remain local.

MEM|8 files and logs live under the configured root. The persisted cursor resumes
rotation after restart. Started/validated/pushed/finished receipts expose interrupted
work. A single OS lock prevents overlapping workers. Keep backups of this root.

## Layout

`aurora-core`: arithmetic and tuples · `aurora-mem8`: durable memory ·
`aurora-phoenix`/`aurora-dyad`: integer dynamics · `aurora-dharma-quantum`: capability
bitmasks and rational statistics · `aurora-infer`: ternary primitives ·
`aurora-voice-shell`: PCM16/HUD · `aurora-cli`: memory bridge · `cmd/aurora-agy`: worker.

## Brand & Live HUD (`via agy` · `1b-go` ↔ `b8-go`)

- **Interactive Live HUD (Wave & Dyad Oscilloscope + Brand Showcase):** [`brand/hud.html`](brand/hud.html) · [https://aurora.vaked.dev/hud](https://aurora.vaked.dev/hud)
- **Brand Gallery:** [`brand/gallery.html`](brand/gallery.html) · [https://aurora.vaked.dev/gallery](https://aurora.vaked.dev/gallery)
- **Apple Silicon Go Toolchain (`b8-go` & `1b-go` alias):** [https://github.com/8b-is/go-setup](https://github.com/8b-is/go-setup)

MIT OR Apache-2.0. From love, from within. `{<3,<3,<3}+1`
