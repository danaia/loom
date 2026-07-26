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

## Performance status

No Gate 4 performance number is promoted from this development tree. The prior
Gate 3 populated timing remains a source-dirty engineering artifact, and the
additional ledger and audit passes change the scheduled workload. A release
benchmark from the clean committed Gate 4 SHA is required before publishing an
updated timing claim.
