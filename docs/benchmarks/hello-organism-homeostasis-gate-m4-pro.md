# Hello Organism Homeostasis Gate — M4 Pro

Date: 2026-07-25

## Correctness load

The Metal test `organism_sustains_a_bounded_homeostatic_state` starts with one
organizer and runs the complete local developmental program for 30,000 ticks.
No global population, morphology, radius, or energy correction feeds back into
cell decisions.

Ticks 10,000–11,000 establish the specimen's population, energy, fate, area,
perimeter, compactness, centroid, and radial-density envelopes. Ticks
29,000–30,000 form a disjoint validation window. The run reports:

```text
population:                    39
components:                     1
organizers:                     1
boundary:                      16
interior:                      22
reference samples:           1000
validation samples:          1000
validation violations:          0
post-development violations:    0
neighbor overflow:              0
physical overflow:              0
perception truncation:           0
deposit saturation:             0
```

The per-tick ledger records previous and current total energy, absorbed nutrient,
maintenance, developmental decisions, signaling, successful division,
accepted motion, environmental death loss, instantaneous residual, and
cumulative residual. Accepted motion is explicitly zero for this non-locomoting
specimen. The acceptance test bounds the instantaneous residual to `0.001`
energy units and the mean cumulative residual to `0.00002` energy units per
tick.

## Perturbation and return

`organism_returns_to_its_reference_envelope_after_nutrient_perturbation` executes
the canonical `homeostasis_perturbation` scenario:

```text
tick 12,000: nutrient supply 1.00 → 0.25
tick 14,000: nutrient supply 0.25 → 1.00
```

Both changes are emitted as recorded scenario events with explicit value
overrides. Total energy falls below the pre-perturbation reference minimum.
After restoration, all 1,000 validation ticks remain inside the original
expanded morphology and energy envelopes. The final body is the same connected
39-cell-class morphology with one organizer, 16 boundary cells, and 22 interior
cells, with zero long-horizon invariant violations.

This proves bounded equilibrium plus return after a non-destructive
environmental perturbation. It does not yet prove repair after structural loss.

## Clean Gate 4 timing

```text
cargo run --release -q -p loom-metal --bin hello-particle-view -- \
  organism 1024 --bench headless --warmup 30000 --samples 100
```

Device: Apple M4 Pro. macOS: 26.6. Host profile: release. Rust:
`rustc 1.88.0`. Metal SDK: 26.0. Source:
`57f01c5cd04a822d4641e391f6ca39d94ddbcc0c`, clean.

```text
GPU mean: 1.389 ms
GPU p50:  1.339 ms
GPU p95:  1.648 ms
GPU p99:  1.663 ms
GPU max:  1.717 ms
stream buffers: 7,943,216 bytes
```

Runtime artifact:
`428d467c13cec3a4009a90fb04b774ce3e3c64f30c78373f4bdcf8aa6df985ae`.
Runtime fingerprint:
`33605320d5d80e7bfae74947aeb4ad5dca9f243805160f298b924b7f0fa891a2`.

The 30,000-tick warm-up establishes the accepted homeostatic state before
sampling. This remains a 39-cell-class result at 1,024 declared capacity, not a
populated scaling claim.
