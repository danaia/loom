# Hello Organism Population Gate — M4 Pro

Date: 2026-07-25

## Correctness Load

The Metal test
`parallel_population_compacts_and_allocates_one_thousand_parents` starts 1,024
parents in reversed storage order. One tick:

- removes every fourth parent through accepted death intents,
- accepts non-overlapping daughters,
- reports zero spatial-bin or observation overflow,
- restores increasing stable-ID storage order,
- allocates child IDs in increasing stable-parent order.

This exercises global radix ordering, spatial qualification, hierarchical scans,
parallel scatter, staged commit, and authoritative count publication together.

## Declared-Capacity Smoke Benchmark

```text
cargo run -q -p loom-metal --bin hello-particle-view -- \
  organism 16384 --bench headless --warmup 25 --samples 100
```

Device: Apple M4 Pro. Host profile: debug.

```text
GPU mean: 0.966 ms
GPU p95:  1.111 ms
GPU max:  1.212 ms
stream buffers: 11,248,668 bytes
```

The benchmark begins with one active organizer. It measures the cost of the
16,384-capacity pipeline and is not evidence that 16,384 interacting cells sustain
that rate. A populated, isolated timing benchmark remains required before making
that claim.
