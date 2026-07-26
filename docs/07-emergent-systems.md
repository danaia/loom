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
  with one organizer, 17 boundary cells, 21 interior cells, complete radial
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
  sustained recovery checks have deterministic reference tests.

Not yet claimed:

- real-time 16,384-cell morphogenesis,
- 30,000-tick homeostasis and 50,000-tick regeneration acceptance,
- measurement cadence optimization and populated 256/1,024/4,096/16,384 scaling,
- adaptive fine-to-coarse aggregation,
- cross-domain material and abstract-network proofs.

The one-seed developmental gate is now closed. Homeostasis and regeneration
remain separate claims and must be measured before promotion.
