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
GPU stream buffers:           100,000,072 bytes
peak resident set:            153,747,456 bytes
GPU time min / mean / p95:    1.84 / 2.87 / 4.54 ms
end-to-end min / mean / p95:  4.46 / 5.44 / 7.14 ms
steady-state app copies:      0 per tick
steady-state app blits:       0 per tick
```

The populated `32³` correctness probe separately ran 100 ticks, read the GPU metric
buffer and rendered texture back, and proved that the solid phase grew beyond the
seed, exposed visible shaded surface pixels, and accumulated no autonomous damage.
It then injected one pointer slice and proved that material was removed and exactly
one slice event was recorded.

The short sample is intentionally described as a smoke proof. A publishable gate
still needs a clean-tree, long-duration, paced run that spans growth, interactive
slicing, component convergence, and detached-fragment motion.
