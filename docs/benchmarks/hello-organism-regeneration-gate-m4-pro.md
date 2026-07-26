# Hello Organism Regeneration Gate — M4 Pro

Date: 2026-07-25

## Proof load

The release Metal test
`committed_homeostatic_checkpoint_branches_into_causal_regeneration_proofs`
constructs the canonical one-organizer specimen through tick 30,000, then copies
every committed GPU stream into three independent runtime states. Full stream
readback proves all four branch checkpoints are byte-identical before any
intervention.

The branches then execute through tick 50,000:

```text
control
structural_regeneration
regeneration_without_injury
regeneration_without_repair
```

At tick 30,000 the lesion branches receive the same quantized peripheral circle.
The intervention preserves the organizer; records center, radius, removed stable
IDs, removed energy, and damaged-shell count; marks selected cells for
authoritative removal; and applies a local injury field. Removed IDs are strictly
increasing and identical across all lesion branches.

The recorded lesion circle is intervention and measurement data only. Repair
decisions and daughter placement do not read its center, radius, removed IDs, or
pre-lesion coordinates. Candidate daughters follow quantized local injury
concentration while the existing contact, density, energy, nutrient, inhibitor,
overlap, and simulation-boundary rules remain authoritative.

## Acceptance

The enabled branch must sustain every predicate for 500 consecutive ticks before
tick 50,000:

```text
one converged contact component
one organizer
population inside the expanded pre-lesion envelope
boundary/interior counts and area within 10%
compactness within 15%
centroid drift within 10% of reference radius
cell disks intersecting the wound region at least 90% as often as removed cells
injury no greater than 5% of its post-lesion peak
energy residual no greater than 0.001
zero neighborhood overflow, truncation, or deposit saturation
```

The accepted result is a connected 40-cell morphology inside the expanded
envelope learned from the canonical 39-cell pre-lesion body.

```text
removed cells:                    9
damaged shell cells:             10
removed stable IDs:               7, 13, 19, 20, 22, 30, 32, 33, 34
recovery success tick:       30,806
accepted ticks at 50,000:    19,694
final population:                 40
components / unresolved:        1 / 0
organizers:                        1
boundary / interior:           16 / 23
area Q16.16:                   1,160
compactness Q16.16:            8,393
cell-disk wound occupancy:        16
final injury total:                0
post-lesion injury peak:   2,490,368
```

## Causal ablations

`regeneration_without_injury` disables injury transport immediately before the
otherwise identical lesion. Its transported injury peak remains zero and it
never satisfies the recovery criterion. Uncoordinated ordinary developmental
growth reaches 97 cells and exits the morphology envelope.

`regeneration_without_repair` retains injury transport but disables repair
behavior immediately before the same lesion. It also never satisfies the
recovery criterion, ending with 30 cells and persistent injury.

These branches distinguish causal local injury/repair behavior from passive
regrowth or a hidden global morphology correction.

## Clean post-Gate-5 timing

```text
cargo run --release -q -p loom-metal --bin hello-particle-view -- \
  organism 1024 --bench headless --warmup 30000 --samples 100
```

Device: Apple M4 Pro. macOS: 26.6. Host profile: release. Rust:
`rustc 1.88.0`. Metal SDK: 26.0. Source:
`b9b3f6d4a070423982f33867e3f3c3dc4fcc2449`, clean.

```text
GPU mean: 1.422 ms
GPU p50:  1.425 ms
GPU p95:  1.462 ms
GPU p99:  1.521 ms
GPU max:  1.578 ms
stream buffers: 8,217,972 bytes
```

Runtime artifact:
`bbaeb3c6b37930a098672899830ad0bc6d0612f1fe2ffea7ea116b1355da9411`.
Runtime fingerprint:
`55d9573f27d1cb7b6ad34ffd9a792067576c01d834aee8298d550d30178120f6`.
Organism shader:
`1751a13058b12bf922ca8b7d0539d21f190a94b699df7bdc81e39aa800ca902e`.

This is a 39-cell-class single-branch workload at 1,024 declared capacity. It is
not a populated regeneration-scaling result. Checkpoint branching and the four
20,000-tick causal suffixes remain a correctness workload rather than an
interactive performance benchmark.

Proof command:

```text
cargo test --release -p loom-metal \
  runtime::tests::committed_homeostatic_checkpoint_branches_into_causal_regeneration_proofs \
  -- --nocapture
```

The committed tick-30,000 checkpoint hash is:
`58b307aef094fa5204a0ebda443c2ccf38ac6a8f998f2950f086bf7e19d4b1b8`.
