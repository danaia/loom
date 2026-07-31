# Hello Organism Neighborhood Gate — M4 Pro

Date: 2026-07-25

## Correctness Load

The Metal test
`developmental_neighborhoods_produce_connected_differentiated_metrics` begins
with a 16×16 contact body in reversed storage order. After two ticks it proves:

- one converged contact component,
- nonempty boundary and interior fates,
- nonzero quantized area, perimeter, and compactness,
- radial-density bins accounting for all 256 cells,
- zero physical-neighbor overflow,
- zero developmental-perception truncation.

Component metrics are accepted only when the final relaxation round reports no
label changes.

## Declared-Capacity Smoke Benchmark

```text
cargo run -q -p pqo-metal --bin hello-particle-view -- \
  organism 16384 --bench headless --warmup 10 --samples 50
```

Device: Apple M4 Pro. Host profile: debug.

```text
GPU mean: 1.527 ms
GPU p95:  1.683 ms
GPU max:  1.802 ms
stream buffers: 12,100,732 bytes
```

This begins with one active organizer and measures the 16,384-capacity
neighborhood and reduction infrastructure. It is not a populated 16,384-cell
morphogenesis result.
