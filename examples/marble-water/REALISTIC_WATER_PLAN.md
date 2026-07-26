# Realistic Water Implementation Plan

> **For Hermes:** Use the `subagent-driven-development` skill to implement this plan task-by-task.

**Goal:** Evolve Marble Water from a disc-rendered 2D wave demonstration into a smooth, convincing real-time water surface with foam, spray, and bubbles on Apple M4-class GPUs.

**Architecture:** Keep the existing GPU-resident shallow-water height field as the bulk solver. Render it as a continuous grid surface, derive surface-energy data on Metal, and use fixed-capacity GPU-resident pools for secondary foam, spray, and bubble particles. Do not introduce CPU/GPU simulation readback in the frame loop; rendering consumes simulation streams directly.

**Tech Stack:** Loom 0.1 graphs and schedules, external Metal compute/render kernels, Rust project extension, Vue 3 control panel, `loom check`, `loom explain`, and the `loom-metal` headless benchmark.

---

## Scope and constraints

### In scope

- Continuous water-surface rendering instead of visible particle discs.
- Surface normals, Fresnel response, reflection/refraction approximation, absorption, and depth tint.
- Surface-energy diagnostics used to drive secondary effects.
- Persistent foam, airborne spray/droplets, and underwater bubbles.
- GPU-side particle lifecycle, allocation, reset, and compaction.
- Quality controls, counters, and performance measurement.
- Scaling the base grid from 202×152 toward 512×384 or 512×512 when profiling permits.

### Out of scope for the first implementation

- Replacing the height field with a full 3D FLIP/APIC, SPH, or PBF solver.
- Arbitrary pouring, overhangs, tunnels, or disconnected bulk-water volumes.
- CPU-owned particle state or frame-by-frame GPU readback.
- Offline-quality path tracing or ocean-scale FFT simulation.

### Target budgets

- Base surface: start at 202×152; qualify 512×384 and then 512×512.
- Foam capacity: start at 50,000; tune toward 200,000.
- Spray capacity: start at 10,000; tune toward 50,000.
- Bubble capacity: start at 20,000; tune toward 100,000.
- Primary acceptance target: stable 60 FPS at the default quality preset on the M4 Pro reference machine.
- Stretch target: presentation above 60 FPS while preserving a fixed, deterministic simulation step.
- No unbounded allocation, hidden readback, or per-frame CPU particle creation.

## Definition of done

- The water reads visually as one continuous surface at normal viewing distance.
- Marble wakes and drops create localized foam and detached spray.
- Strong impacts create bubbles that rise, expire, and contribute foam at the surface.
- Reset clears all secondary effects deterministically.
- All particle counts remain within declared capacities.
- `loom check`, `loom explain`, Rust tests, UI build, package build, and the Metal execution test pass.
- Measured FPS and GPU memory are recorded for each quality preset.

---

## Task 1: Capture a reproducible baseline

**Objective:** Record current correctness, visual behavior, frame rate, and allocation before changing simulation or rendering.

**Files:**
- Create during implementation: `examples/marble-water/PERFORMANCE.md`
- Test: `crates/loom-metal/tests/native_loom.rs`

**Steps:**

1. Run `cargo run -q -p loom-cli -- check examples/marble-water/marble-water.loom`; expect `status: valid`.
2. Run `cargo run -q -p loom-cli -- explain examples/marble-water/marble-water.loom` and save the pass order, dispatch sizes, and resource allocations in `PERFORMANCE.md`.
3. Run `cargo test -p loom-metal marble_water_compiles_and_executes_the_particle_simulation -- --nocapture`; expect the Metal program to compile and execute.
4. Run the interactive project at minimum, default, and maximum density. Record presented FPS, Metal allocation, resolution, and observed artifacts.
5. Capture the default scene, a moving wake, and a marble drop as visual regression references.
6. Set the quality gate: no later phase may regress default-preset FPS by more than 10% without an explicit quality/performance tradeoff recorded in `PERFORMANCE.md`.

**Suggested checkpoint:** `docs: record marble water performance baseline`

---

## Task 2: Replace particle discs with a continuous height-field surface

**Objective:** Make the current water grid appear smooth before increasing simulation complexity.

**Files:**
- Modify: `examples/marble-water/marble-water.loom`
- Modify: `examples/marble-water/shaders/marble_water.metal`
- Modify: `examples/marble-water/kernels/marble_water.metal`
- Test: `crates/loom-metal/tests/native_loom.rs`

**Steps:**

1. Add a render stream for per-sample surface normals, with explicit type, capacity, storage, and access.
2. Declare an external Metal kernel that computes each normal from central differences of neighboring water heights.
3. Bind the normal-generation pass after water integration and before scene staging.
4. Update scene staging so water samples expose height and normal data without creating a CPU-side mesh.
5. Replace billboarded water discs with indexed or procedurally generated grid triangles in the Metal view pipeline.
6. Add boundary-safe sampling for active widths and heights at every density setting.
7. Add Fresnel reflection, specular response, depth/angle absorption, shallow/deep tint, and a small procedural normal perturbation for micro-ripples.
8. Keep marbles as impostors or separate instances; do not force them through the grid topology.
9. Add a test assertion that the new normal kernel and surface shader identities appear in the benchmark result.
10. Verify minimum/default/maximum density, 1×/3× plane scale, and viewport resize without cracks, invalid indices, or NaNs.

**Acceptance criteria:** Individual water samples are not visible at the normal camera distance, silhouettes remain continuous, and surface shading responds coherently to moving waves.

**Suggested checkpoint:** `feat: render marble water as a smooth height field`

---

## Task 3: Derive surface diagnostics for secondary effects

**Objective:** Produce stable physical signals for foam and spray emission instead of relying on random spawning.

**Files:**
- Modify: `examples/marble-water/marble-water.loom`
- Modify: `examples/marble-water/kernels/marble_water.metal`
- Modify: `examples/marble-water/shaders/marble_water.metal`
- Modify: `examples/marble-water/ui/src/App.vue`
- Modify: `examples/marble-water/src/runtime.rs`

**Steps:**

1. Add streams for surface normal, curvature, compression/breaking energy, and impact energy.
2. Implement one diagnostic kernel using the same spacing-aware neighborhood and active-grid rules as the water solver.
3. Define a bounded emission score from curvature, vertical speed, compression, and marble impact energy.
4. Clamp and sanitize every diagnostic output; non-finite values must become zero.
5. Add a temporary diagnostic render mode that visualizes normal, curvature, and emission score.
6. Add a panel selector for the debug mode and route it through `src/runtime.rs` as a Loom override.
7. Verify calm water produces near-zero emission, moving marbles create narrow wake signals, and drops create localized high-energy regions.
8. Remove or hide debug visualization from the normal quality preset while keeping it available for development.

**Acceptance criteria:** Emission signals are spatially localized, stable across density settings, and decay when the surface calms.

**Suggested checkpoint:** `feat: derive water surface energy diagnostics`

---

## Task 4: Add a persistent foam field

**Objective:** Produce coherent foam patches that follow wakes and impacts without requiring every foam feature to be an individual particle.

**Files:**
- Modify: `examples/marble-water/marble-water.loom`
- Modify: `examples/marble-water/kernels/marble_water.metal`
- Modify: `examples/marble-water/shaders/marble_water.metal`
- Modify: `examples/marble-water/ui/src/App.vue`
- Modify: `examples/marble-water/src/runtime.rs`

**Steps:**

1. Add a device-resident scalar foam-density stream matching the maximum water-grid capacity.
2. Add a foam update kernel that injects from the emission score, transports foam with local surface motion, diffuses slightly, and decays by lifetime.
3. Use a stable update rule with bounded density in `[0, 1]`.
4. Clear the foam field in the existing reset path.
5. Blend foam into the surface shader as irregular white coverage with reduced transparency and higher roughness.
6. Use deterministic coordinate noise only to break up uniform grid-shaped edges; do not let noise create foam without physical emission.
7. Add panel controls for foam amount and foam lifetime, with clamped values in `src/runtime.rs`.
8. Extend Rust control tests for defaults, clamping, and reset behavior.
9. Verify foam forms behind moving marbles, concentrates around strong impacts, follows the surface, and disappears after the configured lifetime.

**Acceptance criteria:** Foam persists as patches rather than one-frame flashes, does not appear on calm water, and never reveals the underlying grid pattern at normal scale.

**Suggested checkpoint:** `feat: add persistent surface foam`

---

## Task 5: Introduce fixed-capacity GPU secondary-particle pools

**Objective:** Establish bounded GPU-side lifecycle infrastructure shared by spray and bubbles.

**Files:**
- Modify: `examples/marble-water/marble-water.loom`
- Modify: `examples/marble-water/kernels/marble_water.metal`
- Modify: `examples/marble-water/shaders/marble_water.metal`
- Test: `crates/loom-metal/tests/native_loom.rs`

**Steps:**

1. Declare explicit streams for particle position, velocity, age, lifetime, size, kind, and alive state.
2. Declare atomic counters for spawn requests, active counts, dropped spawns, and reset generation.
3. Use fixed capacities initially; never allocate Metal buffers from a simulation tick.
4. Add a reset-counters pass before emission classification.
5. Add a deterministic emission pass that turns high-energy cells into bounded spawn requests.
6. Add a GPU allocation pass using atomic slot reservation. If capacity is exhausted, increment the dropped-spawn counter and skip safely.
7. Add integration and lifecycle passes that mark expired particles dead.
8. Add compaction only after profiling proves sparse fixed-capacity dispatch is a bottleneck; until then, prefer the simplest validated bounded implementation.
9. Render particles directly from their GPU streams and discard dead entries in the shader.
10. Extend the Metal benchmark test to execute the new passes and assert their shader identities.
11. Add scenario or runtime assertions that active counts never exceed capacities and counters reset deterministically.

**Acceptance criteria:** Particle lifecycle remains entirely GPU-resident, capacity overflow is safe and measurable, and reset leaves no live secondary particles.

**Suggested checkpoint:** `feat: add bounded GPU secondary particle pools`

---

## Task 6: Add spray and droplet physics

**Objective:** Create detached splash droplets that leave and re-enter the base water surface.

**Files:**
- Modify: `examples/marble-water/marble-water.loom`
- Modify: `examples/marble-water/kernels/marble_water.metal`
- Modify: `examples/marble-water/shaders/marble_water.metal`

**Steps:**

1. Spawn spray only when emission energy and upward velocity exceed configurable thresholds.
2. Derive initial direction from surface normal, local velocity, impact direction, and deterministic jitter.
3. Integrate gravity and aerodynamic drag in Metal.
4. Detect crossing from above to below the sampled water surface.
5. On re-entry, atomically deposit a bounded impulse into a dedicated water-impulse stream rather than writing water velocity from multiple particles directly.
6. Reduce the impulse stream into the water update on the following pass and clear it deterministically.
7. Convert a fraction of re-entry energy into foam emission.
8. Render droplets as small lit impostors with size/opacity varying over lifetime.
9. Verify that calm motion emits no spray, strong drops emit a burst, droplets fall under gravity, and re-entry creates small ripples without instability.

**Acceptance criteria:** Spray separates visibly from the surface, follows ballistic arcs, and feeds bounded energy back into water and foam.

**Suggested checkpoint:** `feat: add GPU spray and droplet feedback`

---

## Task 7: Add underwater bubble physics

**Objective:** Represent entrained air beneath strong impacts and allow it to rise and pop into surface foam.

**Files:**
- Modify: `examples/marble-water/marble-water.loom`
- Modify: `examples/marble-water/kernels/marble_water.metal`
- Modify: `examples/marble-water/shaders/marble_water.metal`
- Modify: `examples/marble-water/ui/src/App.vue`
- Modify: `examples/marble-water/src/runtime.rs`

**Steps:**

1. Spawn bubbles beneath the impact point when impact energy exceeds a separate threshold.
2. Assign buoyancy, drag toward local water motion, bounded turbulence, lifetime, and size.
3. Keep bubbles below the sampled surface until they reach their pop condition.
4. On pop, kill the bubble and deposit a bounded amount into the foam field.
5. Render bubbles with depth-aware tint and rim lighting; fade them when occluded or far from the camera.
6. Add panel controls for bubble amount and lifetime and test control clamping in Rust.
7. Verify bubbles rise rather than sink, remain bounded to the pool domain, pop at the surface, and leave no stale particles after reset.

**Acceptance criteria:** Strong entries create visible underwater bubble trails that rise and produce localized surface foam without destabilizing the height field.

**Suggested checkpoint:** `feat: add buoyant bubbles and surface popping`

---

## Task 8: Add quality presets and scale the base grid

**Objective:** Scale visual density in measured steps rather than assuming the highest particle count is fastest or best.

**Files:**
- Modify: `examples/marble-water/marble-water.loom`
- Modify: `examples/marble-water/ui/src/App.vue`
- Modify: `examples/marble-water/src/runtime.rs`
- Modify: `examples/marble-water/PERFORMANCE.md`

**Steps:**

1. Define Low, Medium, High, and Ultra presets covering active grid size, foam capacity/use, spray rate, bubble rate, and optional shader quality.
2. Preserve fixed maximum capacities in the Loom graph; presets change active work and emission limits rather than reallocating every frame.
3. Start High at 512×384 and treat 512×512 as an Ultra candidate.
4. Add preset selection to the panel and map it to explicit Loom constants through `src/runtime.rs`.
5. Test preset defaults and clamping in Rust.
6. Benchmark every preset after warm-up and record FPS, GPU time if available, allocation, active counts, and dropped spawns.
7. Inspect `loom explain` to confirm dispatch domains and resource hazards remain explicit.
8. Reduce overdraw, pass count, or effect capacity before reducing physical stability.
9. Select the default preset that sustains the 60 FPS acceptance target on the M4 Pro reference machine.

**Acceptance criteria:** Every preset is bounded and stable; the default meets its frame target; visual quality increases monotonically without changing simulation behavior unexpectedly.

**Suggested checkpoint:** `feat: add measured water quality presets`

---

## Task 9: Optimize only measured bottlenecks

**Objective:** Recover performance without changing the physical model blindly.

**Files:**
- Modify as measurements require: `examples/marble-water/marble-water.loom`
- Modify as measurements require: `examples/marble-water/kernels/marble_water.metal`
- Modify as measurements require: `examples/marble-water/shaders/marble_water.metal`
- Modify: `examples/marble-water/PERFORMANCE.md`

**Steps:**

1. Measure each compute and render pass separately using Metal command-buffer timestamps where available.
2. Check whether the always-dispatched reset pass is measurable during normal ticks; make reset work conditional if Loom/runtime semantics permit it.
3. Check tiny marble pass overhead and fuse only passes whose dependency boundaries and tests remain clear.
4. Inspect threadgroup occupancy, memory access patterns, branch divergence, atomic contention, and transparent overdraw.
5. Keep neighborhood data contiguous and avoid duplicate height/normal sampling across adjacent passes when fusion is demonstrably beneficial.
6. Add compaction and indirect dispatch for secondary particles only if sparse fixed-capacity dispatch is a measured bottleneck.
7. Re-run correctness tests and visual regressions after each optimization class.
8. Record before/after timing and reject optimizations that do not improve the measured target.

**Acceptance criteria:** Optimization decisions have recorded measurements, preserve validation and visual behavior, and introduce no hidden synchronization or allocation.

**Suggested checkpoint:** `perf: optimize measured marble water bottlenecks`

---

## Task 10: Validate, package, and document the completed feature

**Objective:** Prove the implementation is valid, executable, controllable, and distributable.

**Files:**
- Modify: `examples/marble-water/README.md`
- Modify: `examples/marble-water/PERFORMANCE.md`
- Modify: `crates/loom-metal/tests/native_loom.rs`
- Rebuild: `examples/marble-water/marble-water.lmp`

**Steps:**

1. Document the layered solver, secondary effects, controls, capacities, presets, and limitations in the README.
2. Run `cargo fmt --check`.
3. Run `cargo test -p loom-metal marble_water_compiles_and_executes_the_particle_simulation -- --nocapture`.
4. Run `cargo run -q -p loom-cli -- check examples/marble-water/marble-water.loom`; expect `status: valid`.
5. Run `cargo run -q -p loom-cli -- explain examples/marble-water/marble-water.loom`; inspect pass order, hazards, dispatch domains, and generated Metal.
6. Run `npm run build` in `examples/marble-water/ui`.
7. Run `cargo run -q -p loom-cli -- build examples/marble-water/marble-water.loom`.
8. Run `cargo run -q -p loom-cli -- check examples/marble-water/marble-water.lmp`; expect `status: valid`.
9. Run the packaged `.lmp`, exercise every preset/control, reset under peak load, and inspect foam/spray/bubble cleanup.
10. Record final M4 Pro FPS, allocation, active counts, dropped spawns, and known limits in `PERFORMANCE.md`.

**Acceptance criteria:** Source and package validate, Metal executes headlessly, UI assets build, the packaged project runs interactively, and all definition-of-done requirements are documented with measured evidence.

**Suggested checkpoint:** `docs: finalize realistic marble water simulation`

---

## Decision gate for full 3D water

Do not replace the height field merely to increase particle count. Start a separate FLIP/APIC or PBF design only if the required behavior includes pouring, overturning waves, cavities, disconnected volumes, or arbitrary containers.

Before approving that migration:

1. Demonstrate that the layered height-field approach cannot satisfy a named visual requirement.
2. Build a throwaway Metal spike at 100,000–250,000 particles.
3. Measure neighbor/grid construction, pressure iterations, collisions, and surface reconstruction separately.
4. Validate 500,000 particles only after the smaller target meets the frame budget.
5. Choose screen-space surface reconstruction before marching cubes unless geometry export or true silhouettes require a mesh.
6. Keep the current height-field implementation as the performance fallback preset.
