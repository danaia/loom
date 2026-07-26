# Emergent Systems Runtime

## Implemented Substrate

The schema-v3 graph adds the minimum generic semantics required by emergent
systems:

- count-backed dynamic stream lengths,
- packaged backend source text,
- per-slot `PerInvocation` or `WholeResource` indexing reach,
- protected stream writes and explicit pass capability grants,
- state-mutation and membership-mutation capabilities,
- tick-addressed scenario interventions with canonical value overrides,
- GPU indirect compute dispatch and indirect rendering,
- explicit per-pass threadgroup-width contracts,
- global stable-ID LSD radix ordering, canonical bounded spatial bins,
  hierarchical prefix scans, stable-ID birth allocation, and parallel
  structure-of-arrays compaction,
- quantized developmental neighborhoods, exact eight-sector exposure masks,
  convergence-audited contact components, and GPU morphology reductions,
- deterministic exposed-sector daughter search with authoritative overlap,
  boundary, parent-contact, and simultaneous-candidate qualification,
- explicit activator/inhibitor transport controls, cumulative overflow and
  Q16.16 deposit-saturation audits, and per-lineage logical trajectory hashes,
- a per-tick energy ledger, declared nutrient-supply intervention, disjoint
  reference/validation envelopes, and long-horizon invariant counters,
- checkpoint-forked control and lesion scenarios, a local injury field,
  authoritative damaged-state repair, injury-gradient daughter placement, and
  sustained regeneration acceptance,
- deterministic reference rules for transitions, quantization, deposits,
  neighborhoods, contact connectivity, stable compaction, allocation, energy,
  and sustained recovery envelopes.

The Metal runtime fingerprints its internal indirect-argument lowering kernels
alongside application kernels.

## Causal Loop

```text
committed state T
→ rebuild canonical neighborhoods
→ observe contact, density, and exposure
→ reduce morphology for committed state T
→ sample fields
→ quantize perception
→ decide intents
→ resolve protected state
→ deposit signals
→ diffuse into next fields
→ commit fields
→ resolve deaths and births
→ reconcile energy ledger
→ audit sustained morphology, energy, injury, and wound closure
→ committed state T+1
```

Decision kernels cannot write protected cell state. `Hello Organism` grants:

- `mutate_cell_state` to state and population resolvers,
- `mutate_cell_membership` only to the membership resolver,
- `mutate_field_state` only to diffusion and field-commit passes.

All dynamic cell kernels guard their rounded Metal dispatch lanes against the
authoritative active-count stream. Padded threadgroup lanes never participate in
perception, decisions, deposits, component labels, or reductions.

## Reference Numerics

- Field grid: 256×256.
- Diffusion: reflective five-point Laplacian with declared stable coefficients.
- Deposits: unsigned Q16.16, normalized 3×3 radial kernel.
- Deposit saturation and all neighborhood overflows accumulate for the complete
  run; a clean final read cannot conceal an earlier violation.
- Maximum per-cell request: 1.0 unit per channel per tick.
- Decision bins: integers in `0...4095`.
- Contact: quantized distance no greater than the radii sum plus 25% of the
  smaller radius.
- Physical neighbor bound: 128.
- Perception neighbor bound: 64.
- Surface exposure: exact integer eight-sector occupancy.
- Component labels: 64 deterministic contact-relaxation rounds; metrics are exact
  only when `component_unresolved == 0`.
- Morphology: population, fate counts, components, quantized area/perimeter,
  centroid, compactness, and eight-bin radial density.
- Homeostasis: ticks 10,000–11,000 establish the organism's own morphology and
  energy envelope; ticks 29,000–30,000 must remain inside the declared expanded
  bounds. A separate counter audits connectivity, organizer uniqueness,
  differentiated fates, and all overflow conditions after tick 3,200.
- Regeneration: a peripheral quantized circular lesion at tick 30,000 removes
  8–12 non-organizer cells and damages its contact shell. Recovery requires 500
  consecutive ticks inside the expanded pre-lesion morphology envelope, at
  least 90% cell-disk wound-region occupancy, one converged component, one organizer,
  injury below 5% of its post-lesion peak, bounded energy residual, and no
  overflow, truncation, or saturation.

The backend-neutral reference implementation lives in `loom_core::emergent`.
Metal implementations are required to match its logical rules and declared
tolerances.

## Current Proof Boundary

Implemented and tested:

- packaged kernels compile on Metal,
- dynamic indirect dispatch executes on GPU,
- field passes execute in declared order,
- the coupled organism executes 300 ticks,
- the organizer produces at least one daughter,
- one organizer reaches a deterministic 39-cell connected body by tick 3,200,
  with one organizer, 16 boundary cells, 22 interior cells, complete radial
  accounting, nonzero area/perimeter/compactness, converged components, and
  zero cumulative overflow, truncation, or deposit saturation,
- a second reference run reproduces the complete integer morphology state,
  stable IDs, parent IDs, fates, phases, health, quantized inhibitor decisions,
  and per-lineage birth/transition trajectory hashes exactly,
- disabling activator transport arrests development at five cells with no
  interior tissue; disabling inhibitor transport reaches 55 cells and exits the
  declared 24–48 reference population envelope,
- one GPU tick compacts deaths and allocates daughters from 1,024 simultaneous
  parents while retaining canonical stable-ID order,
- a 256-cell contact body converges to one component, differentiates into
  nonempty boundary and interior populations, and reports complete morphology
  metrics with zero overflow or truncation,
- unauthorized writes and malformed membership capabilities are rejected,
- transition tables, quantization, reflective diffusion, storage-order
  independence, contact connectivity, stable allocation, energy ledgers, and
  sustained recovery checks have deterministic reference tests,
- a 30,000-tick run records 1,000 reference samples and 1,000 disjoint
  validation samples with zero envelope or post-development invariant
  violations,
- a recorded nutrient perturbation lowers supply to 25% at tick 12,000,
  restores it at tick 14,000, measurably lowers total energy, and returns the
  connected 39-cell differentiated organism to its original envelope for all
  1,000 validation ticks,
- every tick reports previous/current energy, absorption, maintenance,
  decisions, accepted motion, signaling, division, death loss, per-tick
  residual, and cumulative residual; accepted motion is explicitly zero for
  this non-locomoting specimen, and the 30,000-tick proofs bound both
  instantaneous and mean cumulative accounting error,
- a byte-identical committed tick-30,000 checkpoint forks into control,
  structural-lesion, no-injury-transport, and no-repair branches,
- the canonical lesion preserves the organizer, records its Q16.16 geometry,
  removed stable IDs, damaged-shell count, and removed energy, then the enabled
  branch recovers a connected 40-cell morphology inside the 39-cell reference
  envelope and sustains every declared regeneration predicate for at least 500
  consecutive ticks by tick 50,000,
- recorded lesion geometry is used only to apply and measure the intervention;
  cell decisions and daughter placement receive the local injury field, not the
  lesion center, radius, removed IDs, or pre-lesion coordinates,
- the no-injury branch receives the identical lesion but records no transported
  injury peak and grows without coordination to 97 cells, while the no-repair
  branch receives the same injury and lesion but remains at 30 cells; neither
  ablation reaches the regeneration criterion,
- the reference repair branch first completes 500 consecutive accepted ticks at
  tick 30,806 and remains accepted through tick 50,000.

Not yet claimed:

- real-time 16,384-cell morphogenesis,
- Gate 5 performance timing or populated regeneration scaling,
- measurement cadence optimization and populated 256/1,024/4,096/16,384 scaling,
- adaptive fine-to-coarse aggregation,
- cross-domain material and abstract-network proofs.

The one-seed development, sustained-homeostasis, and causal structural-
regeneration gates are now closed. Adaptive hierarchy and cross-domain proofs
remain separate claims.
