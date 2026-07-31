# Pqo Water Simulation Capabilities

## Executive summary

Pqo is a strong foundation for realistic real-time water simulation, but it is not yet a complete fluid-rendering engine.

The current system is designed around the right simulation principles:

- GPU-resident typed streams;
- explicit Metal compute kernels;
- validated resource access and pass ordering;
- bounded capacities and dynamic population support in the core model;
- indirect GPU dispatch and drawing in the Metal runtime;
- direct rendering from simulation state;
- no required steady-state CPU readback;
- deterministic validation, artifact identity, and performance measurement.

That architecture is well suited to height-field water, surface diagnostics, foam fields, spray, bubbles, and other secondary effects. The largest gaps are in the render-resource model and the native `.pqo` syntax—not in the core GPU simulation model.

A layered realistic-water implementation is feasible after a small set of engine capability gates. A full 3D FLIP/APIC, SPH, or PBF solver is also representable through external Metal, but would currently require substantial hand-written infrastructure for sorting, scans, compaction, textures, and surface reconstruction.

---

## What Pqo is made for

Pqo v0 is a typed compute/render graph targeting Metal. It separates physical meaning from backend implementation:

```text
Pqo streams and values
→ exact kernel effects
→ bound passes and dispatch domains
→ validated dependencies and hazards
→ Metal compute/render pipelines
→ GPU-resident execution
```

This model is a good fit for water simulation because fluid systems naturally decompose into dense fields and ordered parallel passes:

```text
water state
→ force/stencil solve
→ integration
→ surface diagnostics
→ secondary-effect emission
→ particle integration
→ render preparation
→ presentation
```

Complex fluid arithmetic does not need to fit inside the current native Pqo kernel subset. It can remain in explicit external Metal kernels while Pqo owns the state, units, access boundaries, bindings, dispatch domains, and schedule.

Relevant implementation boundary:

- `docs/gpu-authoring-handbook.md:30` describes Pqo v0 as a typed compute/render graph with native and external Metal kernels.
- `crates/pqo-metal/src/runtime.rs` performs the validated Metal lowering and execution.
- `examples/marble-water/marble-water.pqo` already expresses a GPU-resident water and rigid-body coupling graph.

---

## Capability matrix

| Water-system requirement | Current status | Notes |
|---|---|---|
| Large GPU-resident scalar/vector fields | Supported | Streams allocate fixed declared capacities in device storage. |
| 2D shallow-water or height-field solver | Supported | The current Marble Water example already runs a spacing-aware stencil. |
| Surface normal and curvature derivation | Supported | Implement as explicit Metal passes over the water grid. |
| Marble/water force coupling | Supported | Existing position, velocity, impact, and response streams demonstrate the pattern. |
| Persistent foam-density field | Supported | Represent foam as a scalar stream aligned with the water grid. |
| Fixed-capacity foam/spray/bubble pools | Supported | Use explicit streams and alive/lifetime fields. |
| Atomic spawn and overflow counters | Supported | Atomic kernel slots are represented and validated. |
| GPU-side impulse accumulation | Supported | Use atomic or staged contribution streams followed by reduction. |
| Explicit reset and lifecycle passes | Supported | Reset can clear bounded streams deterministically. |
| Direct rendering from simulation streams | Supported | Views bind stream buffers directly as vertex inputs. |
| Dynamic indirect compute dispatch | Supported in core/runtime | Count-backed streams lower to bounded indirect dispatch. |
| Dynamic indirect drawing | Supported in core/runtime | Count-backed render domains lower to indirect draw arguments. |
| Dynamic stream length in text `.pqo` | Not yet supported | The parser currently accepts only integer `len` values. |
| General indexed mesh rendering | Not yet supported | Generic views currently issue six-vertex instanced triangle draws. |
| Depth attachment and depth testing | Not yet supported | The generic render pipeline configures a color attachment only. |
| First-class texture resources | Not yet supported | Compute/view graph bindings currently expose stream buffers, not typed textures. |
| Multipass render composition | Limited | Text flows expose one presentation view and generic views clear their target. |
| Multiple independent simulation rates | Not yet supported | The first Metal runtime slice requires exactly one schedule. |
| Native scans, sorting, and compaction | Not yet supported | These can be external Metal pass sequences but are not native library operations. |
| Screen-space fluid reconstruction | Not yet supported cleanly | Requires textures, offscreen targets, depth/thickness passes, and multipass composition. |
| Full 3D FLIP/APIC, SPH, or PBF | Possible but infrastructure-heavy | External Metal can implement it, but current rendering and population ergonomics need extension. |

---

## Capabilities available now

### 1. GPU-resident water fields

The current Marble Water graph declares water position and velocity streams with a maximum population of 30,704 samples. The same stream model can represent larger regular grids or additional aligned fields such as:

- surface normal;
- curvature;
- compression;
- wave energy;
- impact energy;
- foam density;
- accumulated secondary-particle impulse.

All of these can remain in device-private Metal buffers across ticks. No CPU-side water object model is required.

### 2. Explicit neighborhood compute

External Metal kernels may use whole-resource stream access for bounded neighborhood operations. This supports:

- finite-difference stencils;
- spacing-aware Laplacians;
- height gradients;
- curvature estimates;
- local energy classification;
- grid-based advection;
- local surface sampling for body and particle coupling.

Pqo validates that the kernel declares every stream it reaches and that conflicting passes are ordered.

### 3. Atomic effects and reductions

The Pqo syntax and semantic model include atomic stream access. This is sufficient to represent:

- spawn counters;
- overflow counters;
- contribution counts;
- impact accumulation;
- bounded allocation indices;
- diagnostic counters.

Contention-sensitive algorithms still need careful Metal design. Declaring an access as atomic establishes authority and hazards; it does not make a high-contention algorithm scalable automatically.

### 4. Bounded secondary effects

Foam, spray, and bubbles can be implemented today using fixed-capacity streams:

```text
secondary.position
secondary.velocity
secondary.age
secondary.lifetime
secondary.size
secondary.kind
secondary.alive
```

Every tick can dispatch over the fixed capacity and skip dead entries. This is simple, valid, and bounded. Compaction and dynamic indirect dispatch should be introduced only after profiling shows sparse dispatch to be a bottleneck.

### 5. Performance and resource evidence

The Metal runtime records GPU timing, CPU orchestration timing, end-to-end timing, deadline misses, stream-buffer bytes, indirect-buffer bytes, and resident-set information.

Existing M4 Pro evidence includes:

- one million simple particles;
- 52 MB of GPU stream state;
- paced 120 Hz execution;
- approximately 6 ms render GPU p95 in the recorded presented experiment;
- no recorded deadline misses in that experiment.

Reference: `docs/benchmarks/hello-batch-compact-paced-m4-pro.md`.

This proves that Pqo can execute and render large simple populations efficiently. It does not prove equivalent performance for a million-particle pressure solver, neighborhood search, or transparent fluid reconstruction. Fluid workloads must be benchmarked independently.

---

## Current engine limitations

### 1. Generic views are six-vertex instanced draws

The generic Metal view path currently uses a fixed call equivalent to:

```text
draw triangle primitives
vertex start = 0
vertex count = 6
instance count = view population
```

Reference: `crates/pqo-metal/src/runtime.rs:1981`.

This is ideal for billboard particles and can be adapted to a procedural water grid by treating each cell as a six-vertex instance. It is not equivalent to a general indexed-mesh renderer.

Immediate workaround:

- one instance per water cell;
- six procedural vertices per cell;
- fetch four corner heights in the vertex shader;
- emit two triangles;
- suppress cells outside the active width/height.

Recommended engine capability:

- configurable primitive topology;
- configurable vertex count;
- optional index stream;
- direct and instanced draw modes;
- explicit procedural draw domains.

### 2. No depth attachment or depth-stencil state

The current generic render pipeline configures a BGRA color attachment and blending. It does not allocate or bind a depth attachment.

This limits:

- correct marble/water intersection;
- underwater bubble occlusion;
- spray visibility behind solid objects;
- opaque/transparent layer composition;
- reconstructed surface depth.

Recommended engine capability:

- typed depth attachments;
- configurable depth format;
- clear/load/store policy;
- depth comparison function;
- per-view depth-write policy;
- depth lifetime validation.

### 3. Dynamic populations are not expressible in text `.pqo`

The core graph and Metal runtime support count-backed dynamic stream lengths, bounded indirect compute dispatch, and bounded indirect drawing.

References:

- `docs/04-execution-scheduling.md:26` describes fixed and dynamic logical lengths.
- `crates/pqo-metal/src/runtime.rs:1849` lowers a dynamic count to indirect compute dispatch.

The text parser currently handles `len` as an integer only:

- `crates/pqo-syntax/src/lib.rs:528`.

Consequences:

- builder-created graphs can use dynamic populations;
- the current distributable `marble-water.pqo` cannot declare a count-backed pool;
- fixed-capacity streams with alive flags are the portable text-source solution today.

Recommended engine capability:

```text
stream spray.count: u32 {
  cap=1 len=1 access=rw storage=device
}

stream spray.position: f32x3<m> {
  cap=50000 len=spray.count access=rw storage=device
}
```

This requires coordinated parser, model-lowering, validator, canonical-format, runtime, package, and test coverage.

### 4. Textures are not first-class executable resources

The runtime owns the final drawable texture, but Pqo compute and view bindings currently use stream buffers. There is no complete graph path for declaring and binding sampled, storage, depth, or offscreen textures.

This blocks clean implementations of:

- foam masks stored as textures;
- screen-space depth and thickness;
- reflection/refraction targets;
- bilateral smoothing;
- reconstructed normals from depth;
- post-processing;
- environment maps managed as graph resources.

Buffer-backed 2D fields remain viable for simulation. First-class textures become important when rendering moves beyond a procedural single-pass surface.

Recommended engine capability:

- typed 2D/3D texture resources;
- dimensions and pixel formats;
- sampled/read/write access modes;
- sampler declarations;
- color/depth render-target bindings;
- explicit usage and storage modes;
- texture hazard and lifetime validation.

### 5. Render composition is limited

The text flow syntax currently supports one `draw` presentation entry, and the generic render path clears the color target for a view.

Reference: `crates/pqo-syntax/src/lib.rs:856`.

A production water renderer normally needs ordered stages such as:

```text
opaque scene
→ water depth/thickness
→ water surface shading
→ bubbles
→ foam and spray
→ post-processing
→ presentation
```

Immediate workaround:

- stage all visible objects into one aligned render population;
- branch in one Metal pipeline by render kind;
- use fixed ordering and approximated depth where acceptable.

Recommended engine capability:

- multiple ordered views per presentation;
- explicit color/depth attachments;
- clear/load/store declarations;
- per-view blend and depth state;
- offscreen render targets;
- post-process views.

### 6. One execution schedule per Metal runtime

The current Metal runtime requires exactly one schedule:

- `crates/pqo-metal/src/runtime.rs:760`.

Presentation can be driven independently from the fixed simulation clock, but the graph cannot yet declare separate rates for water, foam, bubbles, or other subsystems.

Immediate approach:

- keep all simulation passes in one fixed schedule;
- preserve a stable fixed timestep;
- reduce work through active limits rather than independent subsystem clocks.

Possible future capability:

- multiple compatible schedules;
- integer rate divisors inside one schedule;
- explicit cross-rate state publication;
- validated interpolation between committed simulation states.

### 7. No native scan, radix-sort, or compaction primitives

Dynamic fluid and particle algorithms commonly require:

- prefix scans;
- stream compaction;
- radix sorting;
- spatial bin construction;
- scatter/gather;
- indirect count publication.

Pqo can represent these as explicit external Metal passes with declared temporary streams and dependencies. The native kernel language and standard library do not yet expose them as reusable primitives.

Reference: `docs/native-compiler-gates.md:118` identifies scans and compaction as a future compiler gate.

This is not a hard execution blocker, but it increases implementation effort and makes algorithmic correctness the responsibility of external Metal plus tests.

---

## Realistic-water feasibility

### Layered height-field water

**Feasibility: High after modest renderer work.**

Suitable features:

- continuous surface mesh;
- normals and optical shading;
- local wakes and impact waves;
- buoyancy and body coupling;
- persistent foam field;
- detached spray;
- underwater bubble particles;
- particle re-entry impulses;
- bounded quality presets.

Required engine work:

1. Add a usable depth attachment.
2. Generalize procedural grid drawing or indexed mesh drawing.
3. Add dynamic length syntax eventually, although fixed-capacity pools work first.
4. Add multipass rendering later for higher-quality optics.

This is the recommended direction for Marble Water.

### Screen-space particle fluid

**Feasibility: Medium after render-resource expansion.**

Required capabilities:

- offscreen depth and thickness textures;
- depth-aware particle splatting;
- bilateral smoothing passes;
- normal reconstruction;
- refraction/absorption shading;
- ordered multipass composition;
- depth testing.

The current stream compute model is adequate, but first-class textures and multipass rendering are prerequisites.

### Full 3D FLIP/APIC, SPH, or PBF

**Feasibility: Technically possible, not yet ergonomic.**

The compute graph can represent:

- particle integration;
- grid transfer;
- pressure solve stages;
- spatial binning;
- neighbor passes;
- collision stages;
- compaction;
- render preparation.

However, the first implementation would require substantial external Metal for:

- scans and sorting;
- dynamic population maintenance;
- pressure iterations;
- grid/particle transfer;
- surface reconstruction;
- render textures and multipass composition.

A full 3D solver should be approved only after a measured spike demonstrates the required behavior and frame budget. It should not replace the height-field path merely to increase particle count.

---

## Recommended Pqo capability gates

### Gate 1: Generalized view drawing

Deliver:

- configurable primitive type;
- configurable vertex count;
- optional instance count domain;
- optional index stream and index type;
- procedural grid draw support;
- validator and execution-plan representation;
- Metal lowering and conformance tests.

Unlocks:

- continuous height-field rendering;
- general meshes;
- less duplicated surface geometry.

### Gate 2: Depth and render-state declarations

Deliver:

- depth attachment allocation;
- depth format;
- clear/load/store behavior;
- comparison and write policy;
- explicit blend configuration;
- render-state fingerprinting.

Unlocks:

- correct marble/water intersections;
- bubbles and spray with occlusion;
- layered transparent rendering.

### Gate 3: Dynamic population text syntax

Deliver:

- count-backed `len` syntax;
- membership capability declarations;
- aligned dynamic stream validation;
- canonical serialization;
- package round-trip support;
- indirect dispatch/draw tests from parsed text.

Unlocks:

- compact spray and bubble pools;
- GPU-published active counts;
- bounded indirect work without fixed-capacity dead dispatch.

### Gate 4: Multipass render composition

Deliver:

- multiple ordered presentation views;
- attachment load/store declarations;
- shared color/depth targets;
- offscreen target identity;
- presentation dependency validation.

Unlocks:

- separate opaque, water, bubble, foam, and post-process stages.

### Gate 5: First-class textures

Deliver:

- texture nodes in the resource graph;
- dimensions, formats, usage, and storage;
- kernel and view texture slots;
- samplers;
- offscreen render targets;
- hazard, lifetime, and fingerprint coverage.

Unlocks:

- screen-space water reconstruction;
- reflection/refraction buffers;
- thickness, foam, and post-processing textures.

### Gate 6: GPU algorithm primitives

Deliver reusable, validated operations for:

- prefix scan;
- compaction;
- radix sort;
- histogram/bin construction;
- bounded scatter;
- indirect count publication.

Unlocks:

- efficient dynamic secondary particles;
- spatial neighbor structures;
- practical full 3D particle-fluid experiments.

### Gate 7: Multi-rate execution if measurements require it

Deliver only after profiling demonstrates value:

- integer pass-rate divisors or multiple schedules;
- cross-rate committed-state semantics;
- interpolation policy;
- overload and presentation rules;
- deterministic timing tests.

Unlocks:

- water at one fixed rate;
- foam or bubbles at a lower rate;
- presentation at display cadence.

---

## Recommended implementation order

1. Preserve the current GPU-resident height-field solver.
2. Add generalized procedural surface drawing and depth support.
3. Render the water as a continuous surface.
4. Add surface diagnostics and a persistent foam field.
5. Add fixed-capacity spray and bubble pools.
6. Add dynamic count-backed text syntax and indirect pool execution.
7. Profile and optimize measured bottlenecks.
8. Add multipass render composition and first-class textures for advanced optics.
9. Add scan/sort/compaction primitives before attempting a full 3D fluid solver.
10. Keep the height-field implementation as the high-performance fallback even if a volumetric solver is added later.

---

## Final assessment

Pqo is made for the simulation architecture required by realistic water:

- explicit state;
- exact effects;
- bounded GPU work;
- deterministic pass ordering;
- no hidden copies or allocation;
- direct simulation-to-render dataflow;
- measurable contracts and reproducible artifacts.

The current system can already support the compute side of smooth height-field water, foam diagnostics, foam fields, spray integration, bubbles, and bounded coupling. The primary limitations are generalized geometry, depth, textures, multipass rendering, and text-level dynamic population declarations.

Therefore, the recommended strategy is to extend Pqo rather than replace it. Build the renderer and resource capabilities needed by the layered-water plan while preserving the existing validated stream-and-pass model. That keeps Marble Water aligned with Pqo’s intended purpose and turns the example into a useful capability driver for the engine itself.
