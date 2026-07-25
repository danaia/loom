# Loom Implementation Roadmap

A gate-driven plan for building **Loom**, the language agents use to create physical worlds and compile them into low-level execution.

The canonical system vision lives in `loom-hello-particle-xmind.md`. The language charter and semantic model live in `docs/00-language-charter.md` and `docs/01-semantic-model.md`.

“Language first” means locking Loom's constitution and primary composition patterns before building the runtime. It does not mean completing a general parser or polishing every syntax rule before anything executes.

```text
language charter
→ minimal typed semantic graph
→ Hello Particle program
→ validation and Metal lowering
→ simulation, rendering, inspection, measurement
→ stable text projection
→ compiled bundle
```

The first implementation milestone must still drive an agent-authored Hello Particle program through the complete path.

---

## Gate 0 — Agent Language Foundation

**Goal:** Give agents a coherent, enjoyable way to describe state, computation, execution, proof, and observation.

### Constitution

Lock the rules in `docs/00-language-charter.md`:

- Agents propose; deterministic validators and verifiers decide.
- Persistent state is explicit and stream-based.
- Kernels declare every effect.
- Passes bind every concrete resource.
- Schedules contain semantic dependencies rather than Metal mechanisms.
- Units participate in type checking.
- Capacity and physical buffering are distinct.
- Allocation, copying, readback, and synchronization are never hidden.
- Contracts are scoped, executable claims.
- Target-specific lowering does not change target-neutral physics.
- Determinism claims carry an execution fingerprint.

### Primary language patterns

Lock the responsibilities of:

| Construct | Role |
| --- | --- |
| `module` | Versioned program and namespace boundary |
| `value` | Immutable typed data |
| `stream` | Typed indexed state; structure-of-arrays by construction |
| `kernel` | Reusable computation and effect signature |
| `pass` | Concrete bindings and dispatch domain for a kernel |
| `schedule` | Ordering, dependencies, timing, and overload policy |
| `contract` | Static or measurable required properties |
| `scenario` | Deterministic setup, execution, and expectations |
| `view` | Render, inspector, or telemetry projection |
| `capability` | Exceptional host, inspection, or external authority |

The repeated composition rhythm is:

```text
declare
→ transform
→ bind
→ schedule
→ constrain
→ prove
→ view
```

### Version 0 semantic surface

- `bool`, `i32`, `u32`, `f16`, and `f32`.
- Fixed vectors and matrices.
- Deterministic-layout structs.
- Values, streams, textures, and views.
- Length, time, mass, velocity, acceleration, frequency, and dimensionless units.
- `read`, `write`, `read_write`, `atomic`, `render`, `inspect`, and `external` effects, with capabilities required when work crosses ordinary declared dataflow.
- Stable qualified names, semantic IDs, schema versions, canonical ordering, and content hashes.
- Explicit rejection of missing bindings, undeclared mutation, incompatible units, ambiguous layouts, dependency cycles, unordered hazards, capability escalation, and unsupported contracts.

### Agent experience

- One semantic responsibility per construct.
- Deterministic formatting and canonical JSON diagnostics.
- Stable diagnostic codes with source and related spans.
- `loom explain` can reveal inferred defaults, resolved bindings, effects, dependencies, layouts, and contract instrumentation.
- Experiments are named variants that can be checked and compared without mutating an accepted artifact.
- The smallest valid program remains small; layout and backend controls appear only when needed.

### Deliverables

- `docs/00-language-charter.md`.
- `docs/01-semantic-model.md`.
- `docs/04-execution-scheduling.md`.
- `docs/06-canonical-representation.md`.
- `docs/decisions/0001-language-shape.md`.
- `examples/hello-particle/hello-particle.loom` as the v0 language specimen.
- `crates/loom-core` with typed graph nodes and a direct builder API.
- `crates/loom-validator` with only the rules required by Hello Particle.
- A programmatic `hello_particle_graph()` conformance fixture.

### Acceptance

- The builder represents every declaration and edge in the Hello Particle specimen without untyped escape hatches.
- The graph contains explicit stream effects, pass bindings, schedule dependencies, contract scopes, scenario predicates, views, and inspection capability.
- Missing bindings, unit mismatches, undeclared writes, unordered hazards, dependency cycles, and illegal capabilities fail with stable diagnostic codes.
- Unsafe buffer reuse fails unless the graph provides enough versions, serializes conflicting ticks, or carries a valid queue-order proof.
- Simulation overlap and presentation lifetime are validated independently.
- Dependencies mean completion; contracts, views, and inspection name exact observation/snapshot semantics.
- Malformed references stop at structural validation without panicking.
- Hash-bound repair plans verify old values, apply atomically, and rerun validation.
- Only `ValidatedModuleGraph` produces an `ExecutionPlan` and executable artifact fingerprint.
- Constructing the same program twice produces the same canonical graph and hash.
- The language review identifies punctuation as provisional but treats the semantic patterns as binding.

---

## Gate 1 — Hello Particle End to End

**Goal:** Prove that the Gate 0 program and graph can become a visible, measured Metal simulation.

This is a native macOS proof using `CAMetalLayer`; Tauri is out of scope. The runtime consumes the validated semantic graph rather than a separate hard-coded model.

### Execution slice

- Native AppKit window backed by `CAMetalLayer`.
- One private Metal resource per hot structure-of-arrays stream, each with logical capacity one.
- A small shared control ring for camera and per-submission constants.
- Ahead-of-time Metal integration, contact, vertex, and fragment functions.
- An ordered command buffer containing:

  ```text
  fall
  → fall-to-bounce dependency
  → bounce
  → bounce-to-viewport dependency
  → viewport
  ```

- Semi-implicit Euler integration at a fixed timestep of `1 / 120 s`.
- Sphere-plane contact that corrects penetration and writes both position and velocity.
- Point or instanced-quad rendering as a sphere impostor.
- No synchronous CPU readback in the simulation or render loop.

Semantic dependency edges are lowered to the appropriate Metal ordering mechanism. They do not encode backend-specific barriers in Loom.

Dispatch dimensions come from the compiled pipeline's `threadExecutionWidth` and `maxTotalThreadsPerThreadgroup`, following [Apple's threadgroup sizing guidance](https://developer.apple.com/documentation/metal/calculating-threadgroup-and-grid-sizes). A target label never substitutes for pipeline properties.

### Clock and overload policy

- Fixed `1 / 120 s` simulation ticks; `dt` is never stretched.
- At most four retained ticks may be encoded to catch up before presenting.
- Rendering may be dropped while catching up.
- Excess wall-clock debt is discarded and reported rather than changing the timestep.
- At most four simulation ticks may be pending, but single-version conflicting ticks execute serially until versioning or a queue-order proof permits greater effective concurrency.
- At most two rendered frames may be in flight.
- A tick above `8.33 ms` is a measured budget miss.
- Thirty consecutive budget misses fail the Gate 1 benchmark.

### Initialization and inspection

Private resources use an explicit staging upload before steady state begins, consistent with [Metal's private-resource upload model](https://developer.apple.com/documentation/metal/copying-data-to-a-private-resource).

The inspection capability uses a pooled shared staging resource and an asynchronous GPU copy. Completion publishes an immutable snapshot without a wait in the normal frame loop. Requested snapshots are measured separately from steady state.

### Scoped contracts

```loom
contract realtime on simulation {
  steady_state after initialization {
    exclude requested_inspection
    heap_allocations_per_tick == 0
    application_copies_per_tick == 0
    application_blits_per_tick == 0
  }

  gpu_time_per_tick <= 8.33 ms
}
```

- **Heap allocation:** issued by Loom host code and observed by configured allocator instrumentation.
- **Application copy:** CPU memory copy explicitly issued by Loom.
- **Application blit:** Metal blit explicitly encoded by Loom.
- **GPU time:** measured interval whose timestamp or counter method is recorded.

Driver, compositor, and framework-internal work is reported separately rather than claimed as zero.

### Tier-1 determinism fingerprint

Tier 1 applies only when all recorded identity fields match:

- exact Mac model and GPU identity/family,
- OS version and build,
- Metal language and compiler identity,
- host compiler identity and relevant flags,
- source, metallib, function-constant, descriptor, and pipeline hashes,
- resource layout, dispatch dimensions, schedule hash, and ordered inputs.

A different fingerprint requires a tolerance-based tier.

### Deliverables and acceptance

- `crates/loom-runtime` consuming a validated Gate 0 graph.
- `prototypes/hello-particle-metal/` native host.
- `metal/hello-particle/` compute and rendering implementations.
- Repeatable scenario and benchmark reports.
- Asynchronous position/velocity inspection.

**Acceptance:** the language specimen and programmatic graph take the same validation/lowering path; the particle visibly falls and bounces; dependencies, fixed-step behavior, overload policy, inspection, scoped zero-work claims, GPU timing, and fingerprint all pass.

---

## Gate 2 — Canonical `.loom` Projection

**Goal:** Stabilize the text agents read and write after its semantic patterns have survived Gate 1.

The block-style Hello Particle specimen is the starting point, not an immutable grammar. Gate 2 freezes only spelling that improves clarity without weakening explicit semantics.

### Text rules

- `.loom` maps one-to-one onto the typed graph.
- Qualified references resolve independently of declaration order.
- Values and types have distinct forms.
- Units remain attached through parsing and printing.
- All pass bindings and schedule dependencies remain explicit.
- Canonical formatting removes stylistic ambiguity.
- Parse → graph → print → parse preserves semantic identity.

### Diagnostics and tools

```text
loom check    <source.loom>
loom format   <source.loom>
loom explain  <source.loom> [qualified-name]
loom project  <graph-or-bundle> -o <projection.loom>
```

`loom explain` is central to the agent experience. It shows resolved identity, types, units, bindings, effects, hazards, dependency paths, layout decisions, capabilities, and contract evidence.

### Deliverables and acceptance

- `crates/loom-syntax` parser, source AST, formatter, and source mapping.
- `crates/loom-cli` with `check`, `format`, `explain`, and `project`.
- `docs/02-types-units-effects.md`.
- `docs/03-memory-model.md`.
- `docs/05-contracts-scenarios.md`.

**Acceptance:** Hello Particle round-trips deterministically, produces the same Gate 0 graph and hash, executes through Gate 1 without a special path, and invalid fixtures yield stable human-readable and JSON diagnostics.

---

## Gate 3 — Ahead-of-Time `.loomb` Bundle

**Goal:** Compile validated `.loom` and backend inputs into a reproducible `.loomb`, then reproduce Gate 1 from that artifact alone.

`.loom` is the canonical textual projection. `.loomb` is the validated compiled bundle. They never share an extension.

### Bundle contents

- Versioned typed graph.
- Stable semantic IDs and reproducible serialization.
- Target compatibility requirements.
- Validated resource layouts and schedule.
- Compiled metallib and pipeline descriptors.
- Contracts, scenarios, benchmarks, capabilities, and views.
- Source, compiler, pipeline, artifact, and provenance hashes.
- Complete determinism fingerprint.

The runtime still queries pipeline-specific limits after pipeline creation and records resolved dispatch values in run provenance.

### CLI

```text
loom build    <source.loom> --target <target> -o <artifact.loomb>
loom run      <artifact.loomb>
loom bench    <artifact.loomb> [scenario] --output <report.json>
loom inspect  <artifact.loomb> --next-snapshot
loom project  <artifact.loomb> -o <projection.loom>
loom compare  <candidate-a> <candidate-b> --scenario <name>
```

### Acceptance

- Identical hermetic inputs and compiler identity produce byte-identical output.
- A clean runtime loads only the `.loomb`, recreates legal pipelines and dispatch, displays Hello Particle, and passes the same contracts.
- Corrupt, incompatible, capability-escalating, or unvalidated bundles fail before execution.

---

## Gate 4 — Tauri Shell

**Goal:** Embed the proven native viewport without changing engine semantics.

- Tauri owns application chrome, commands, and noncritical UI.
- A native `CAMetalLayer` remains the viewport.
- WebView work stays outside the simulation hot path.
- Resize, scale factor, occlusion, focus, and shutdown are tested.
- The Gate 3 native runner remains the reference executable and performance oracle.

**Acceptance:** the shell reproduces simulation, rendering, inspection, and benchmark results without new steady-state allocations, copies, synchronous readbacks, capabilities, or schedule changes.

---

## Gate 5 — Batch Engine

**Goal:** Scale the proven stream model from 1,024 to 1,000,000 particles.

- Capacity growth without changing semantic schemas.
- Active counts without steady-state allocation.
- Pipeline-derived dispatch sizing.
- GPU culling, compaction, and indirect rendering.
- CPU/GPU crossover measurement.
- Deterministic replay harness.

**Acceptance:** 1,024 particles pass correctness; the million-particle target is reported with measured frame time, bandwidth, occupancy, and working set on the named device; bit identity is claimed only under the Tier-1 fingerprint.

---

## Gate 6 — Spatial Physics

**Goal:** Add bounded local interaction.

- Chunked spaces.
- Uniform-grid or spatial-hash construction.
- Neighbor lists and bounded-radius queries.
- Collision and constraint passes.
- Race-free scheduling with an explicit determinism policy.

**Acceptance:** correct bounded-volume collisions at the target scale; measured and budgeted index rebuilds; no all-pairs production path; declared determinism tier preserved.

---

## Gate 7 — Hierarchical Representation

**Goal:** Represent populations larger than active GPU memory without implying dense full-rate updates.

- Quantized cell-local chunks.
- Particle clusters with distributions and bounds.
- Continuous density, velocity, pressure, and signed-distance fields.
- Procedural populations.
- Multi-rate scheduling.
- Bounded active and visible sets.

**Acceptance:** demonstrate one billion **represented** particles, report memory and update coverage, and keep measured error within contract against a smaller full-resolution reference.

---

## Gate 8 — Agent Optimizer

**Goal:** Make safe experimentation a first-class agent workflow.

- Named graph and implementation variants.
- Legal transformations for layout, fusion, precision, dispatch, and scheduling.
- Correctness, determinism, and contract verification for every candidate.
- Repeated counter measurement.
- Promotion only for reproducible improvement.
- Complete provenance for accepted artifacts.

**Acceptance:** agents can propose, explain, compare, reject, and promote variants through the same language patterns and contracts used by the base program.

---

## Gate 9 — Additional Backends

**Goal:** Preserve Loom semantics across execution targets.

- CUDA compute backend.
- Vulkan compute backend.
- CPU SIMD fallback.
- Backend-specific precision, performance, and determinism contracts.

**Acceptance:** the same graph builds for supported targets; physical outcomes meet declared tolerances; provenance records differences; cross-backend results do not claim Tier-1 bit identity.

---

## Cross-Gate Principles

- **Language patterns first.** Agents get stable concepts before runtime architecture hardens around accidental APIs.
- **Execution immediately follows.** Hello Particle is the first implementation proof, not a distant language demo.
- **Semantics before punctuation.** The graph and its invariants stabilize before every text detail.
- **Explicit composition is the fun.** Agents can rearrange understandable parts and receive immediate, trustworthy feedback.
- **Validation before execution.** No unvalidated graph, bundle, capability, or kernel enters the runtime.
- **One model, two encodings.** The graph is canonical; `.loom` projects it as text and `.loomb` packages a compiled result.
- **Scoped claims.** Initialization, steady state, inspection, and framework-internal work are distinct.
- **Measure the actual pipeline.** Hardware decisions use compiled pipeline properties and recorded identity.
- **No runtime AI.** Agents work outside the trusted simulation loop.
- **Scale honestly.** Billions means explicit, aggregated, field, and procedural representations—not a billion dense 120 Hz updates on one chip.

---

## Immediate Next Steps

1. Ratify the hardened graph trust boundary and execution semantics.
2. Define the narrow `ExecutionPlan` → Metal lowering interface.
3. Drive `ValidatedModuleGraph` through the native `CAMetalLayer` proof.
4. Capture pipeline-derived dispatch, timing, and determinism evidence.
5. Freeze the smallest useful `.loom` grammar only after the end-to-end path passes.
