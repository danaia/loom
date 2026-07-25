# Hello Batch — Compact Initialization and Pacing

- Date: 2026-07-25
- Device: Apple M4 Pro
- macOS: 26.6
- Particles: 1,000,000
- GPU stream state: 52 MB
- Host profile: release

## Compact initialization

Hello Batch now represents initial state with typed `repeat`, `linear`, and
`grid_2d` generators. They are canonical graph data, not Metal-specific hidden
setup. The validator checks count, capacity, logical length, element type, vector
shape, arithmetic type, and positive grid dimensions.

| 1M initialization model | Maximum resident set |
| --- | ---: |
| Expanded literals | ~3.38 GB |
| Compact typed generators | ~92 MB |

That is approximately a 36× host-memory reduction while preserving the same 52 MB
of GPU stream buffers. The measured release startup also fell from about five
seconds to well under one second after the binary was built.

## Paced offscreen rendering

Command:

```text
./scripts/run-hello-particle.sh batch 1m --bench rendered \
  --pace 120 --pace-lead-us 2000 \
  --warmup-seconds 5 --duration-seconds 10
```

The 2 ms lead is explicit, remains shorter than one 8.33 ms tick, and allows bounded
queue admission to absorb ordinary host scheduling jitter.

| Metric | Result |
| --- | ---: |
| Measured ticks | 1,200 |
| GPU p95 | 5.591 ms |
| GPU p99 | 6.233 ms |
| End-to-end p95 | 6.575 ms |
| End-to-end p99 | 7.218 ms |
| End-to-end maximum | 7.732 ms |
| Deadline misses | 0 |

This closes the paced offscreen 120 Hz gate.

## Presented experiment

Presented mode attaches a 960×720 `CAMetalLayer`, acquires actual drawables, encodes
the same view, and calls `presentDrawable`.

At 1M particles with a 4 ms lead over a five-second sample:

- GPU p95: 6.209 ms
- GPU p99: 6.640 ms
- Deadline misses: 8 of 600
- One GPU outlier exceeded the 8.33 ms budget

Drawable acquisition and display cadence are now visible in CPU and end-to-end
timings. A larger lead does not solve the problem reliably because acquiring too
early blocks on drawable availability. The next implementation step is
display-synchronized admission rather than more queue lead.

## Reproducibility status

These development measurements correctly report `source_dirty: true`. After the
implementation is committed, rerun the clean-tree commands and commit result files
separately before using them as release evidence.
