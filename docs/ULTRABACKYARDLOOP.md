# ULTRABACKYARDLOOP — measured laps

Each lap is a measurement, not a claim: run the Dream Phase bench at one chunk
size, record the result, append it to [`lap-ledger.jsonl`](lap-ledger.jsonl).
Chunk sizes cycle 16 → 1024 so every pass is a real A/B against the previous.

Reproduce:

```bash
cargo build --release --example bench -p aurora-consolidation
python3 tools/laps.py --laps 108 --events 200000
```

## 108 laps, 200 000 events, 512 distinct payloads

| chunk | median ev/s | laps |
| ----: | ----------: | ---: |
|  1024 |     990 466 |   15 |
|   512 |     581 467 |   15 |
|   256 |     420 557 |   15 |
|   128 |     254 793 |   15 |
|    64 |     117 234 |   16 |
|    32 |      74 388 |   16 |
|    16 |      60 278 |   16 |

Best single lap: **1 807 498 ev/s** at chunk 1024 (lap 70). Footprint was
−97.4% on every lap; summaries stayed at 512, which is the distinct-payload
count, so the global dedupe holds regardless of chunking.

## What the laps changed

The default `chunk_size` moved from 64 to 1024. At 990 k ev/s a 1024-event chunk
is about 1 ms of work, so preemption is still observed well inside the 5 ms
contract, and the per-chunk sort overhead that dominated small chunks is gone.

Measurements are from this machine only. They are a regression guard, not a
published benchmark.
