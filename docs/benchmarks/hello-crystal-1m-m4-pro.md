# Hello Crystal 1M smoke proof — M4 Pro

Date: 2026-07-25

This is a short execution proof for the first Hello Crystal vertical slice, not a
sustained performance baseline.

```text
cargo run --release -p loom-metal --bin hello-particle-view -- \
  crystal 1m --bench headless --warmup 1 --samples 3
```

Observed on Apple M4 Pro:

```text
declared elements:             1,000,000
GPU stream buffers:           92,000,068 bytes
peak resident set:            147,980,288 bytes
GPU time min / mean / p95:    2.03 / 3.24 / 5.42 ms
end-to-end min / mean / p95:  5.01 / 6.34 / 8.38 ms
steady-state app copies:      0 per tick
steady-state app blits:       0 per tick
```

The populated `32³` correctness probe separately ran 100 ticks, read the GPU metric
buffer back, and proved that the solid phase grew beyond the seed, exposed a
surface, and accumulated impact-driven cleavage damage.

The short sample is intentionally described as a smoke proof. A publishable gate
still needs a clean-tree, long-duration, paced run that spans growth, impact,
component convergence, and detached-fragment motion.
