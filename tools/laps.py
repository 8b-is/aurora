#!/usr/bin/env python3
"""laps — the ULTRABACKYARDLOOP driver, one measured lap at a time.

A lap is not a claim, it is a measurement: run the Dream Phase bench at one
chunk size, record what actually came out, and append it to the ledger. Laps
alternate through the chunk sizes so each pass is a real A/B against the last,
never a repeated slogan.

    python3 tools/laps.py --laps 108

Ledger: docs/lap-ledger.jsonl (one JSON object per lap, append-only).
"""

from __future__ import annotations

import argparse
import json
import pathlib
import statistics
import subprocess
import sys

CHUNKS = [16, 32, 64, 128, 256, 512, 1024]


def run_lap(binary: pathlib.Path, chunk: int, events: int, distinct: int) -> dict:
    env = {
        "AURORA_BENCH_CHUNK": str(chunk),
        "AURORA_BENCH_EVENTS": str(events),
        "AURORA_BENCH_DISTINCT": str(distinct),
    }
    completed = subprocess.run(
        [str(binary)],
        capture_output=True,
        text=True,
        env={**__import__("os").environ, **env},
        check=True,
    )
    for line in completed.stdout.splitlines():
        if line.startswith("json "):
            return json.loads(line[5:])
    raise RuntimeError(f"no json line in bench output:\n{completed.stdout}")


def main() -> int:
    parser = argparse.ArgumentParser(description="measured ULTRABACKYARDLOOP laps")
    parser.add_argument("--laps", type=int, default=108)
    parser.add_argument("--events", type=int, default=200_000)
    parser.add_argument("--distinct", type=int, default=512)
    parser.add_argument("--ledger", default="docs/lap-ledger.jsonl")
    parser.add_argument("--binary", default="target/release/examples/bench")
    args = parser.parse_args()

    binary = pathlib.Path(args.binary)
    if not binary.exists():
        print(f"missing {binary}; build with: cargo build --release --example bench -p aurora-consolidation", file=sys.stderr)
        return 2

    ledger = pathlib.Path(args.ledger)
    ledger.parent.mkdir(parents=True, exist_ok=True)

    by_chunk: dict[int, list[float]] = {c: [] for c in CHUNKS}
    best = None
    with ledger.open("a", encoding="utf-8") as handle:
        for lap in range(1, args.laps + 1):
            chunk = CHUNKS[(lap - 1) % len(CHUNKS)]
            record = run_lap(binary, chunk, args.events, args.distinct)
            record["lap"] = lap
            handle.write(json.dumps(record, sort_keys=True) + "\n")
            handle.flush()

            by_chunk[chunk].append(record["ev_per_s"])
            if best is None or record["ev_per_s"] > best["ev_per_s"]:
                best = record
            if lap % 12 == 0 or lap == args.laps:
                print(
                    f"lap {lap:3d} chunk={chunk:4d} {record['ev_per_s']:8.0f} ev/s "
                    f"saved={record['saved_pct']:.1f}% summaries={record['summaries']}",
                    flush=True,
                )

    print("\nper-chunk median ev/s (median of the laps at that size)")
    ranked = []
    for chunk in CHUNKS:
        samples = by_chunk[chunk]
        if samples:
            ranked.append((statistics.median(samples), chunk, len(samples)))
    for median, chunk, count in sorted(ranked, reverse=True):
        print(f"  chunk {chunk:4d}  {median:8.0f} ev/s   ({count} laps)")

    if best:
        print(f"\nbest single lap: chunk={best['chunk']} {best['ev_per_s']:.0f} ev/s "
              f"(lap {best['lap']}, saved {best['saved_pct']:.1f}%)")
    print(f"ledger: {ledger} (+{args.laps} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
