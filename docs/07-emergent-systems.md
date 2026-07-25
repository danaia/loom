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
- deterministic reference rules for transitions, quantization, deposits,
  neighborhoods, contact connectivity, stable compaction, allocation, energy,
  and sustained recovery envelopes.

The Metal runtime fingerprints its internal indirect-argument lowering kernels
alongside application kernels.

## Causal Loop

```text
committed state T
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

## Reference Numerics

- Field grid: 256×256.
- Diffusion: reflective five-point Laplacian with declared stable coefficients.
- Deposits: unsigned Q16.16, normalized 3×3 radial kernel.
- Maximum per-cell request: 1.0 unit per channel per tick.
- Decision bins: integers in `0...4095`.
- Contact: quantized distance no greater than the radii sum plus 25% of the
  smaller radius.
- Physical neighbor bound: 128.
- Perception neighbor bound: 64.

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
- one GPU tick compacts deaths and allocates daughters from 1,024 simultaneous
  parents while retaining canonical stable-ID order,
- unauthorized writes and malformed membership capabilities are rejected,
- transition tables, quantization, reflective diffusion, storage-order
  independence, contact connectivity, stable allocation, energy ledgers, and
  sustained recovery checks have deterministic reference tests.

Not yet claimed:

- real-time 16,384-cell morphogenesis,
- developmental and physical neighbor observations beyond population placement,
- full connected-component and morphology reductions on GPU,
- 30,000-tick homeostasis and 50,000-tick regeneration acceptance,
- adaptive fine-to-coarse aggregation,
- cross-domain material and abstract-network proofs.

Those are the next gates; their contracts must be measured before the associated
claims are promoted.
