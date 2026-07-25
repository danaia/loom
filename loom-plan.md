# Loom Implementation Roadmap

A gate-driven plan for building **Loom**, an agent-managed, low-level 3D physics engine for Apple Silicon.

The canonical system vision lives in `loom-hello-particle-xmind.md`. This roadmap deliberately starts with an executable vertical slice rather than the full language and compiler stack:

```text
minimal typed IR
→ validate Hello Particle
→ compile/load Metal
→ simulate
→ render
→ inspect asynchronously
→ measure
```

Only after that path works do we generalize it into Loom Agent Text (LAT), a deterministic bundle, and a shell application. Every gate must leave behind a runnable artifact and measurements. A later gate may refine an earlier design, but it may not replace evidence with an untested abstraction.

---

## Gate 0 — Native Metal Proof

**Goal:** Show one visibly falling and bouncing particle in the first milestone, using the resource and execution model intended to scale.

This is a native macOS proof with `CAMetalLayer`; Tauri and LAT are explicitly out of scope. State may be declared directly in code. The proof is architectural, not a claim that one GPU particle is faster than one CPU particle.

### Execution slice

- Native AppKit window backed by `CAMetalLayer`.
- One private Metal buffer per hot structure-of-arrays field, each with logical capacity one.
- A small shared control ring containing camera data and per-submission constants.
- Ahead-of-time Metal compute, contact, vertex, and fragment functions.
- An ordered command buffer containing:

  ```text
  integrate
  → integrate-to-contact dependency
  → ground contact
  → contact-to-render dependency
  → render
  ```

- Semi-implicit Euler integration at a fixed timestep of `1 / 120 s`.
- Sphere-plane contact that corrects penetration and updates both `position` and `velocity`.
- Point or instanced-quad rendering as a sphere impostor.
- No synchronous CPU readback in the simulation or rendering loop.

The runtime derives dispatch dimensions from the compiled compute pipeline's `threadExecutionWidth` and `maxTotalThreadsPerThreadgroup`, following [Apple's threadgroup sizing guidance](https://developer.apple.com/documentation/metal/calculating-threadgroup-and-grid-sizes). A target label such as `apple-m4` selects a compatibility profile; it does not hard-code dispatch sizing.

### Clock and overload policy

- The simulation accumulator advances in fixed `1 / 120 s` ticks. The runtime never stretches `dt`.
- At most four simulation ticks may be encoded to catch up before one render.
- Rendering may be skipped while catching up; physics ticks are not skipped within the retained accumulator.
- Wall-clock debt beyond four ticks is discarded and reported as an overload event. This makes simulation time fall behind wall time instead of destabilizing physics with a larger timestep.
- No more than four simulation ticks and two rendered frames may be in flight.
- A tick whose measured GPU duration exceeds `8.33 ms` is a budget miss. It does not change `dt`; it increments telemetry and suppresses nonessential rendering while the runtime catches up.
- The benchmark fails after a configurable sustained-miss threshold. Gate 0 uses 30 consecutive budget misses.

### Initialization and inspection

Private buffers require an explicit initialization path. Gate 0 may initialize through a staging upload and application-issued blit before steady state begins, consistent with [Metal's private-resource upload model](https://developer.apple.com/documentation/metal/copying-data-to-a-private-resource).

`inspect` uses a pooled shared staging buffer and an asynchronous GPU copy. A completion callback publishes an immutable snapshot only after the command buffer completes. Requested snapshots are measured and reported separately; they never introduce a wait into the normal frame loop.

### Measurement vocabulary

- **Heap allocation:** an allocation issued by Loom's host code and observed by the configured allocator instrumentation.
- **Application copy:** a CPU memory copy explicitly issued by Loom.
- **Application blit:** a Metal blit command explicitly encoded by Loom.
- **GPU time:** timestamps or counter samples for the measured compute/contact/render interval, with the method recorded.
- Driver, compositor, and framework-internal work is not claimed as zero; the report distinguishes what Loom can directly attribute.

### Deliverables

- `prototypes/hello-particle-metal/` — native `CAMetalLayer` host.
- `metal/hello-particle/` — integration, contact, and rendering shaders.
- A repeatable benchmark/telemetry capture with allocation, copy, blit, GPU-time, and working-set results.
- An asynchronous position/velocity snapshot command.

### Acceptance

- A particle starts at `(0, 1, 0) m`, falls, contacts the plane, and visibly bounces.
- Contact leaves `position.y >= radius - 1e-4 m` and finite position/velocity.
- Compute, contact, and render observe the required order in one command buffer.
- Simulation remains fixed-step at 120 Hz under the declared overload policy.
- After warmup, ordinary ticks issue zero Loom heap allocations, CPU copies, and Metal blits.
- Startup uploads and explicitly requested inspector snapshots are reported outside the steady-state result.
- GPU time and dispatch properties are captured on the test machine.

---

## Gate 1 — Minimal Semantic Core

**Goal:** Describe exactly the proven Gate 0 path with the smallest useful typed semantic model.

A hard-coded or programmatically constructed graph is acceptable. There is no parser, general-purpose effect calculus, or finalized binary format in this gate.

### Required nodes

- Target and build fingerprint.
- World clock and gravity.
- Space and ground-plane boundary.
- Particle schema and instance initializer.
- Structure-of-arrays buffers and physical buffering policy.
- Kernel inputs, outputs, constant bindings, and read/write effects.
- Ordered schedule and resource dependencies.
- Render projection.
- Correctness, steady-state, determinism, and performance contracts.
- Scenario and benchmark definitions needed by Hello Particle.

Types are limited to the scalar/vector forms and MKS units used by the example. Effects need only prove that the two compute kernels and render pass have valid bindings and dependencies.

### Contracts

The Hello Particle contract is scoped:

```loom
(contract hello-particle
  (correctness
    (finite particle-state.position particle-state.velocity)
    (maximum-ground-penetration (q 1e-4 m)))
  (steady-state
    (begins-after warmup-and-initialization)
    (excludes explicitly-requested-inspector-snapshots)
    (heap-allocations-per-tick 0)
    (application-copies-per-tick 0)
    (application-blits-per-tick 0))
  (scheduling
    (maximum-simulation-ticks-in-flight 4)
    (maximum-render-frames-in-flight 2)
    (maximum-catch-up-ticks 4)
    (overload discard-excess-wall-clock-debt)
    (render-policy drop-while-catching-up))
  (budget
    (gpu-time-per-tick (q 8.33 ms))
    (working-set (q 64.0 KB))))
```

Initialization uploads and requested inspection copies remain legal, separately attributed operations.

### Tier-1 determinism fingerprint

Tier-1 replay applies only when all recorded identity fields match:

- Exact Mac model and GPU identity/family.
- OS version and build.
- Metal language version and compiler identity.
- Host compiler identity and relevant build flags.
- Source, metallib, function-constant, pipeline-descriptor, and pipeline hashes.
- Resource layout, dispatch dimensions, and schedule hash.

Results from a different fingerprint require a tolerance-based determinism tier rather than a bit-identical Tier-1 claim.

### Deliverables and acceptance

- `crates/loom-core` — the minimal graph, IDs, types, and units.
- `crates/loom-validator` — only the validation rules required by Hello Particle.
- `crates/loom-runtime` — consumes a validated graph and drives the proven Metal path.
- A programmatic `hello_particle_graph()` fixture.

**Acceptance:** the fixture validates, runs through the Gate 0 runtime, produces the same visible behavior, passes the same scenarios, and emits the complete determinism fingerprint.

---

## Gate 2 — LAT Projection

**Goal:** Add a readable, writable text projection for the already-proven semantic model.

LAT source uses `.loom`. It parses into the Gate 1 graph and prints deterministically from that graph. The initial grammar covers only constructs exercised by Hello Particle; later features grow from validated use cases.

### Minimal grammar sketch

```text
program         ::= declaration*
declaration     ::= '(' keyword form* ')'
form            ::= atom | declaration
atom            ::= identifier | string | number | bool

quantity        ::= '(' 'q' value unit ')'
unit-annotation ::= '(' 'unit' unit ')'
value           ::= number
                  | '(' 'vec' number+ ')'
                  | '(' 'mat' number+ ')'
type            ::= scalar
                  | '(' 'vec' dimension scalar? ')'
                  | '(' 'mat' dimension scalar? ')'
scalar          ::= 'f16' | 'f32' | 'f64' | 'i32' | 'u32'
dimension       ::= '2' | '3' | '4'

predicate       ::= '(' 'compare' selector comparator value ')'
                  | '(' 'finite' selector+ ')'
                  | '(' 'for-all-ticks' predicate ')'
selector        ::= identifier ('.' identifier)*
comparator      ::= 'eq' | 'ne' | 'lt' | 'le' | 'gt' | 'ge'
```

A type is not a value: `(vec 4 f32)` is a four-component vector type, while `(vec 1.0 0.2 0.4 1.0)` is a vector value. A field's unit is written `(unit m)`, while a quantity includes both a value and unit, such as `(q 1.0 m)`.

### Corrected Hello Particle projection

```loom
(version "0.1.0")
(target apple-m4)

(objective "One particle falls under gravity and bounces on a ground plane")

(world
  (coordinate-system right-handed)
  (units mks)
  (clock (q 120.0 Hz))
  (gravity (q (vec 0.0 -9.81 0.0) m/s^2)))

(space world-space
  (origin (q (vec 0.0 0.0 0.0) m))
  (bounds
    (box
      (q (vec -10.0 0.0 -10.0) m)
      (q (vec 10.0 10.0 10.0) m)))
  (precision f32))

(particle-schema sphere
  (field position (vec 3 f32) (unit m))
  (field velocity (vec 3 f32) (unit m/s))
  (field radius f32 (unit m))
  (field mass f32 (unit kg))
  (field restitution f32)
  (field friction f32)
  (field color (vec 4 f32)))

(buffer-schema simulation-controls
  (field gravity (vec 3 f32) (unit m/s^2))
  (field dt f32 (unit s)))

(instance particle-1 sphere
  (position (q (vec 0.0 1.0 0.0) m))
  (velocity (q (vec 0.0 0.0 0.0) m/s))
  (radius (q 0.004 m))
  (mass (q 0.001 kg))
  (restitution 0.8)
  (friction 0.3)
  (color (vec 1.0 0.2 0.4 1.0)))

(buffer particle-state
  (schema sphere)
  (layout soa)
  (logical-capacity 1)
  (buffering 1)
  (storage private)
  (mutability read-write))

(buffer control-state
  (schema simulation-controls)
  (layout aos)
  (logical-capacity 1)
  (buffering 3)
  (storage shared)
  (mutability read-write))

(kernel integrate
  (domain gpu-compute)
  (in particle-state control-state)
  (out particle-state)
  (bind gravity control-state.gravity)
  (bind dt control-state.dt)
  (effect
    (read particle-state.position
          particle-state.velocity
          control-state.gravity
          control-state.dt)
    (write particle-state.position particle-state.velocity))
  (implementation
    (metal "kernels/euler_integrate.metal" entry "integrate_main")))

(kernel ground-contact
  (domain gpu-compute)
  (in particle-state)
  (out particle-state)
  (effect
    (read particle-state.position
          particle-state.velocity
          particle-state.radius
          particle-state.restitution
          particle-state.friction)
    (write particle-state.position particle-state.velocity))
  (implementation
    (metal "kernels/ground_contact.metal" entry "ground_contact_main")))

(render viewport
  (reads particle-state.position
         particle-state.radius
         particle-state.color)
  (pass particle-pass
    (vertex "shaders/particle_vertex.metal" entry "particle_vertex")
    (fragment "shaders/particle_fragment.metal" entry "particle_fragment")
    (topology points)
    (depth-test true)
    (target color depth)))

(schedule main-loop
  (fixed (q 120.0 Hz))
  (step integrate)
  (dependency integrate-to-contact
    (resource particle-state)
    (from compute-write)
    (to compute-read-write))
  (step ground-contact)
  (dependency contact-to-render
    (resource particle-state)
    (from compute-write)
    (to vertex-read))
  (step render))

(contract hello-particle
  (correctness
    (finite particle-state.position particle-state.velocity)
    (maximum-ground-penetration (q 1e-4 m)))
  (determinism tier-1)
  (steady-state
    (begins-after warmup-and-initialization)
    (excludes explicitly-requested-inspector-snapshots)
    (heap-allocations-per-tick 0)
    (application-copies-per-tick 0)
    (application-blits-per-tick 0))
  (scheduling
    (maximum-simulation-ticks-in-flight 4)
    (maximum-render-frames-in-flight 2)
    (maximum-catch-up-ticks 4)
    (overload discard-excess-wall-clock-debt)
    (render-policy drop-while-catching-up))
  (budget
    (gpu-time-per-tick (q 8.33 ms))
    (working-set (q 64.0 KB))))

(scenario default
  (run (q 5.0 s))
  (expect
    (for-all-ticks
      (compare particle-1.position.y ge (q 0.0039 m)))
    (finite particle-1.position particle-1.velocity)))

(benchmark baseline
  (duration (q 5.0 s))
  (warmup (q 1.0 s))
  (metrics
    gpu-time
    application-heap-allocations
    application-copies
    application-blits
    working-set
    overload-events))
```

### Diagnostics and serialization

- Stable diagnostic codes, source spans, and related-reference spans.
- Undefined bindings, unit mismatches, invalid effects, and missing dependencies are rejected.
- Printing uses canonical declaration and field ordering.
- Parse → graph → print → parse preserves semantic identity.

### Deliverables and acceptance

- `crates/loom-syntax` — parser, source AST, printer, and diagnostics.
- `crates/loom-cli` — `loom check` and `loom project`.
- `examples/hello-particle/hello-particle.loom`.
- `docs/lat-reference.md`.

**Acceptance:** the example parses into the Gate 1 graph, runs through the same runtime without a special code path, round-trips deterministically, and invalid fixtures produce stable, line-numbered diagnostics.

---

## Gate 3 — Ahead-of-Time Bundle

**Goal:** Compile validated `.loom` source and Metal inputs into a reproducible `.loomb` artifact, then reproduce the native proof entirely from that artifact.

`.loom` is LAT source. `.loomb` is the compiled binary bundle. They never share an extension.

### Bundle contents

- Versioned, typed semantic graph.
- Stable semantic IDs and reproducible serialization.
- Target compatibility requirements.
- Validated resource layouts and schedule.
- Compiled metallib and pipeline descriptors.
- Contract, scenario, and benchmark definitions.
- Source, compiler, pipeline, and artifact hashes.
- Complete build provenance and determinism fingerprint.

The bundle does not serialize device-specific runtime values such as `threadExecutionWidth` as universal truths. At load time, the runtime creates each pipeline, queries its actual limits, derives dispatch dimensions, validates them against the contract, and records the resolved values in run provenance.

### CLI

```text
loom check    <source.loom>
loom build    <source.loom> --target <target> -o <artifact.loomb>
loom run      <artifact.loomb>
loom bench    <artifact.loomb> [scenario] --output <report.json>
loom inspect  <artifact.loomb> --next-snapshot
loom project  <artifact.loomb> -o <projection.loom>
loom version
```

### Deliverables and acceptance

- `crates/loom-bundle` — encoder, decoder, versioning, and hashes.
- `crates/loom-compiler` — validation, Metal compilation, and bundle assembly.
- `loom build` produces byte-identical output for identical hermetic inputs and compiler identity.

**Acceptance:** a clean runtime process loads only the `.loomb`, recreates pipelines, derives legal dispatch sizing, shows the Gate 0 particle, passes its contracts, and emits matching provenance. Corrupt, incompatible, or unvalidated bundles are rejected before execution.

---

## Gate 4 — Tauri Shell

**Goal:** Embed the already-working native viewport in the product shell without changing the engine contract.

- Tauri owns application chrome, commands, and noncritical UI.
- The viewport remains a native `CAMetalLayer`.
- WebView work does not enter the simulation hot path.
- Resize, scale-factor, occlusion, focus, and shutdown behavior are tested.
- If native-layer integration fails, the Gate 3 runner remains the reference executable and performance oracle.

**Acceptance:** the Tauri build reproduces the native runner's simulation, rendering, inspection, and benchmark results within the declared envelope, with no new steady-state allocations, copies, synchronous readbacks, or schedule changes in the engine.

---

## Gate 5 — Batch Engine (1,024 to 1,000,000 Particles)

**Goal:** Scale the proven structure-of-arrays path into a batched, GPU-resident engine.

### Components

- Capacity growth without changing the semantic schema.
- Dispatch sizing derived per compiled pipeline.
- GPU culling and compaction.
- GPU-generated indirect render commands.
- CPU/GPU crossover benchmark.
- Deterministic replay harness.

### Acceptance

- 1,024 particles simulate and render correctly.
- The 1,000,000-particle target is reported with measured frame time, bandwidth, occupancy, and working set on the named device; it is not accepted by target label alone.
- Identical replay is bit-for-bit only under the recorded Tier-1 fingerprint.

---

## Gate 6 — Spatial Physics

**Goal:** Add local particle-to-particle and particle-boundary interactions at scale.

- Chunked world space.
- Uniform-grid or spatial-hash construction.
- Neighbor-list generation.
- Bounded-radius interactions.
- Collision/constraint solver passes.
- Race-free scheduling with an explicit determinism policy.

**Acceptance:** correct bounded-volume collisions at the target batch size; measured and budgeted index rebuilds; no all-pairs production path; declared determinism tier preserved.

---

## Gate 7 — Hierarchical Representation

**Goal:** Represent populations larger than active GPU memory without pretending every particle receives a dense 120 Hz update.

- Quantized cell-local chunks.
- Particle clusters with center of mass, distributions, and bounds.
- Continuous density, velocity, pressure, and signed-distance fields.
- Procedural populations.
- Multi-rate scheduling.
- Bounded active and visible sets.

**Acceptance:** demonstrate one billion **represented** particles with a bounded active set, report memory and update coverage, and keep measured error within contract against a smaller full-resolution reference.

---

## Gate 8 — Agent Optimizer

**Goal:** Let an agent propose bounded implementation variants outside the trusted runtime and accept them only through deterministic validation and measurement.

- Legal transformations for layout, fusion, precision, dispatch, and scheduling.
- Candidate generation with explicit change scope.
- Correctness, determinism, and contract verification.
- Metal performance-counter capture.
- Repeated measurement and an acceptance policy for reproducible improvements.
- Full provenance for every accepted artifact.

**Acceptance:** the agent proposes several valid variants, all pass the same contracts, and selection is based on reproducible counters rather than an assumed optimization.

---

## Gate 9 — Additional Backends

**Goal:** Preserve hardware-neutral physics semantics while adding execution targets.

- CUDA compute backend.
- Vulkan compute backend.
- CPU SIMD fallback.
- Backend-specific precision, performance, and determinism contracts.

**Acceptance:** the same semantic program builds for supported targets; physical outcomes meet declared tolerances; provenance records backend-specific precision and performance differences. Cross-backend results do not claim Tier-1 bit identity.

---

## Cross-Gate Principles

- **Visible proof first.** The first milestone ends with a GPU-simulated, rendered particle.
- **Semantics follow evidence.** Generalize the graph, language, and bundle from an executable path.
- **Validation before execution.** No unvalidated bundle or kernel enters the trusted runtime.
- **One model, two encodings.** The typed semantic graph is the canonical model; `.loom` is its source projection and `.loomb` its validated compiled artifact.
- **Scoped performance claims.** Initialization, steady state, inspection, and framework-internal work are reported separately.
- **Measure on the actual pipeline.** Dispatch and performance decisions use compiled pipeline properties and recorded hardware, not marketing labels.
- **No runtime AI.** Agents propose variants offline; simulation never calls a model.
- **Determinism is fingerprinted.** Bit-identical claims are limited to an exact declared execution identity.
- **Scale is honest.** Billions means explicit, aggregated, field, and procedural representations—not a billion dense 120 Hz updates on one chip.

---

## Immediate Next Steps

1. Create the native `CAMetalLayer` Hello Particle proof with private SoA buffers of logical capacity one.
2. Encode integrate → contact → render in one ordered command buffer and implement penetration correction.
3. Add the fixed-step accumulator, four-tick catch-up cap, render-drop policy, and in-flight limits.
4. Add asynchronous staged inspection and separately scoped telemetry for startup, steady state, and requested snapshots.
5. Capture the first device/pipeline fingerprint and benchmark report.
6. Only then extract the minimal typed graph consumed by that working path.
