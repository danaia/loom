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

## Performance boundary

This record promotes a correctness result, not a Gate 5 timing claim. The clean
pre-Gate-5 timing remains archived in
`hello-organism-homeostasis-gate-m4-pro.md`. Injury transport, regeneration
reductions, checkpoint branching, and four 20,000-tick suffixes make this test a
proof workload rather than an interactive performance benchmark. A clean
post-Gate-5 single-branch benchmark is required before publishing new runtime
timing.

Proof command:

```text
cargo test --release -p loom-metal \
  runtime::tests::committed_homeostatic_checkpoint_branches_into_causal_regeneration_proofs \
  -- --nocapture
```
