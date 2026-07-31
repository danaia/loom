# Hello Organism v0.1 — Frozen Proof Manifest

Date: 2026-07-25

This milestone closes Pqo's first complete emergent-system arc:

```text
one organizer
→ differentiated development
→ sustained homeostasis
→ deterministic structural lesion
→ local injury-guided repair
→ sustained morphological recovery
```

The proof view is
[`hello-organism-gate-5.svg`](../visuals/hello-organism-gate-5.svg). Its cell
imagery is schematic; every displayed metric below is measured.

## Source identity

```text
Gate 5 implementation: b9b3f6d4a070423982f33867e3f3c3dc4fcc2449
proof-hash logging:    d843081
branch:                swarm-3
device:                Apple M4 Pro
operating system:      macOS 26.6
Rust:                  rustc 1.88.0
Metal SDK:             26.0
```

The proof-hash commit changes test reporting only. Runtime graph, shader, and
acceptance behavior are identical to the clean benchmarked implementation.

## Reproduction

```text
cargo test --release -p pqo-metal \
  runtime::tests::committed_homeostatic_checkpoint_branches_into_causal_regeneration_proofs \
  -- --nocapture
```

The complete test passed in 195.79 seconds on the recorded device.

## Committed checkpoint

Every GPU stream is copied after the canonical tick-30,000 homeostatic state.
Full readback proves the control, lesion, no-injury, and no-repair branches are
byte-identical before intervention.

```text
checkpoint sha256:
58b307aef094fa5204a0ebda443c2ccf38ac6a8f998f2950f086bf7e19d4b1b8
```

## Canonical lesion event

```text
tick:                    30,000
center Q16.16:           (5,571, 0)
radius Q16.16:           3,932
injury impulse:          4.0
organizer preserved:     yes
removed cells:           9
damaged shell cells:     10
removed energy:          35.3692626953125
removed stable IDs:      7, 13, 19, 20, 22, 30, 32, 33, 34
```

Canonical scenario-event hashes:

```text
lesion:
fd0235d82ffe55ac5f0ae95f6d344f35cc8d508f2d42e2aa2d2d2732593b5406

no injury transport:
219ffa41d678566156ef06f0559d9054f506d2c3430e869164ff7f193b4510ce

no repair behavior:
074dfffc4e1b5247cecbc18b48ad4f105063728d5aca326b9fc888e52920e6ed
```

Lesion center, radius, removed IDs, and old coordinates are not bound to cell
decisions or daughter placement. Repair consumes local quantized injury,
density, contact, energy, nutrient, inhibitor, overlap, and boundary state.

## Recovery result

```text
first accepted tick:             30,806
accepted ticks through 50,000:   19,694
population:                      40
components / unresolved:         1 / 0
organizers:                       1
boundary / interior:             16 / 23
area Q16.16:                      1,160
compactness Q16.16:               8,393
cell-disk wound occupancy:       16
final injury total:               0
post-lesion injury peak:          2,490,368
```

Acceptance requires 500 consecutive ticks with one converged component, one
organizer, population/fates/area inside the expanded pre-lesion envelope,
compactness within 15%, centroid drift within 10% of reference radius, wound
occupancy at least 90% of removed-cell count, injury no greater than 5% of its
peak, bounded energy residual, and zero overflow, truncation, or saturation.

## Causal branches

```text
control:                  39 cells, canonical homeostasis
injury + repair:          40 cells, accepted recovery
no injury transport:      97 cells, zero injury peak, failed morphology
no repair behavior:       30 cells, persistent injury, failed recovery
```

## Clean single-branch timing

Source `b9b3f6d4a070423982f33867e3f3c3dc4fcc2449`, clean:

```text
GPU mean: 1.422 ms
GPU p50:  1.425 ms
GPU p95:  1.462 ms
GPU p99:  1.521 ms
GPU max:  1.578 ms
stream buffers: 8,217,972 bytes
```

```text
runtime artifact:
bbaeb3c6b37930a098672899830ad0bc6d0612f1fe2ffea7ea116b1355da9411

runtime fingerprint:
55d9573f27d1cb7b6ad34ffd9a792067576c01d834aee8298d550d30178120f6

organism shader:
1751a13058b12bf922ca8b7d0539d21f190a94b699df7bdc81e39aa800ca902e
```

This is a 39-cell-class correctness workload at 1,024 declared capacity, not a
populated scaling result.

## Explicit boundary

This milestone does not claim arbitrary wound generalization, organizer
succession, multiple morphologies, 3D tissue, locomotion, learning, biological
fidelity, or populated regeneration scaling. Those remain independent
experiments.
