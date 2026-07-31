# Pqo GPU Authoring Handbook

This handbook explains how to author Pqo programs that lower cleanly to efficient
GPU execution. It is written for human developers and for agents modifying Pqo
graphs, Metal implementations, benchmarks, or future native Pqo kernel bodies.

It is guidance, not a replacement for the language charter or validator. If this
handbook conflicts with the typed graph, validator, or a language decision record,
the implementation and normative documents win.

## Contents

1. [Know the current boundary](#know-the-current-boundary)
2. [Use the GPU execution model](#use-the-gpu-execution-model)
3. [Optimize in the right order](#optimize-in-the-right-order)
4. [Design state as streams](#design-state-as-streams)
5. [Declare kernels as exact effect boundaries](#declare-kernels-as-exact-effect-boundaries)
6. [Build passes around coherent work](#build-passes-around-coherent-work)
7. [Schedule for correctness and overlap](#schedule-for-correctness-and-overlap)
8. [Keep dynamic populations on the GPU](#keep-dynamic-populations-on-the-gpu)
9. [Use scalable GPU algorithms](#use-scalable-gpu-algorithms)
10. [Render directly from simulation state](#render-directly-from-simulation-state)
11. [Author the current Metal implementation](#author-the-current-metal-implementation)
12. [Measure instead of guessing](#measure-instead-of-guessing)
13. [Avoid common failure modes](#avoid-common-failure-modes)
14. [Follow the authoring workflow](#follow-the-authoring-workflow)
15. [Use the agent protocol](#use-the-agent-protocol)
16. [Review with the final checklist](#review-with-the-final-checklist)

## Know the current boundary

Pqo v0 is a typed compute/render graph with a validated execution model. Its
first native kernel subset generates Metal for f32 scalar/vector indexed
arithmetic. Complex arithmetic is still supplied by explicit external Metal.

The executable path today is:

```text
Pqo typed declarations
→ native expression type/unit/effect checking
→ validator
→ execution plan
→ pqo-metal
→ generated or explicit packaged Metal
→ Metal pipelines
→ GPU execution
```

The executable agent-native source is
[`examples/hello-particle/hello-particle.agent.pqo`](../examples/hello-particle/hello-particle.agent.pqo).
Its integration kernel is native Pqo and its contact kernel demonstrates the
explicit Metal escape hatch. The larger crystal and organism programs are still
constructed through the Rust builder API.

The growing native path is:

```text
Pqo declarations plus native kernel bodies
→ validated IR
→ optimized execution graph
→ generated Metal
→ compiled pipelines
```

The optimization rules in this handbook apply to both native and external
paths. Only the place where kernel arithmetic is written changes.

Use these labels when documenting examples:

- **Executable v0** — represented by native Pqo and/or explicit Metal lowered
  through the current typed graph.
- **Canonical text** — valid semantic shape illustrated by the checked-in `.pqo`
  specimen; punctuation may still evolve.
- **Conceptual future syntax** — branches, loops, atomics, threadgroup memory,
  SIMD-group operations, and other forms not accepted by the current parser.

## Use the GPU execution model

Think in dense data and parallel passes, not objects and callbacks.

```text
GPU-resident streams
→ one invocation per active element
→ bounded reads
→ explicit writes
→ explicit pass dependencies
→ GPU-resident render inputs
```

Each Pqo construct has a performance role:

| Pqo construct | GPU meaning | Cost to make explicit |
| --- | --- | --- |
| `value` | Immutable parameter buffer | Value upload and binding |
| `stream` | Dense typed GPU buffer | Capacity, element width, buffering |
| `kernel` | Parallel operation and effect signature | Reads, writes, ABI |
| `pass` | Bound dispatch | Dispatch domain and pipeline launch |
| `schedule` | Dependency and lifetime graph | Ordering and concurrency |
| `view` | Projection of completed state | Render pipeline and state lifetime |
| `capability` | Exceptional mutation or observation authority | Synchronization or trust boundary |
| `contract` | Verifiable performance or correctness claim | Measurement scope |

Pqo makes expensive behavior visible. It does not make a poor algorithm fast.
Performance still comes from locality, parallelism, bounded communication, low
contention, GPU residency, and controlled synchronization.

## Optimize in the right order

Work from the largest cost to the smallest:

1. **Eliminate CPU work proportional to element count.**
2. **Keep authoritative state on the GPU across ticks.**
3. **Choose an algorithm with bounded or hierarchical communication.**
4. **Use dense stream layouts and coherent dispatch domains.**
5. **Remove readback, copying, allocation, and unnecessary synchronization.**
6. **Reduce memory traffic and pass count where measurement supports it.**
7. **Tune threadgroup size and arithmetic last.**

A threadgroup tweak cannot rescue an `O(n²)` neighborhood search, a per-element
host update, or a full-state readback every tick.

Start with explicit budgets:

```text
target rate:              120 Hz
tick budget:              8.33 ms
maximum population:       1,000,000
state bytes per element:  52
steady-state readback:    none
steady-state allocation:  none
determinism tier:         declared
```

Treat the element count, byte budget, tick budget, and observation policy as part
of program design, not cleanup work.

## Design state as streams

### Prefer structure of arrays

Represent independently consumed properties as separate streams:

```pqo
stream particles.position: vec3<f32> unit m { ... }
stream particles.velocity: vec3<f32> unit m/s { ... }
stream particles.radius: f32 unit m { ... }
stream particles.color: vec4<f32> { ... }
```

This lets each pass bind only what it needs. It also avoids pulling cold fields
through cache when a kernel touches only position and velocity.

Use a struct element only when its fields are normally consumed together and the
measured memory behavior is better. Do not reproduce a CPU object graph inside one
large GPU struct by habit.

### Account for every byte

Estimate stream storage before implementation:

```text
stream bytes = capacity × element stride × buffering
total bytes  = sum(stream bytes) + indirect arguments + textures + staging
```

Do not assume source-language vector width equals storage stride. The current
Metal kernels use `packed_float3` for Pqo `vec3<f32>` buffers. Keep the declared
type, generated ABI, upload representation, and MSL parameter type consistent.

Use `f16`, quantized integers, or bit packing only when range, precision,
determinism, and conversion cost have been proved acceptable. Smaller data often
helps a bandwidth-bound kernel, but lossy representation is a semantic decision.

### Separate capacity from logical length

- **Capacity** is the maximum allocated element count.
- **Logical length** is the current dispatchable element count.
- **Buffering** is the number of physical versions needed for lifetime safety.

Never use one as a synonym for another.

For a fixed population, dispatch over a fixed-length stream. For a changing
population, use a scalar count stream and a dynamic length:

```text
cells.active_count: capacity 1, fixed length 1
cells.position: capacity N, dynamic length cells.active_count
```

All streams sharing a dispatch domain must have compatible logical lengths. The
validator rejects mismatched domains.

### Keep steady-state state device-private

Use `device_private` for simulation and render streams that do not require direct
host mutation:

```pqo
storage device_private
access device_read_write
```

Initialization can upload once and blit into private buffers. Requested inspection
can copy an explicit snapshot later. Do not choose host-shared storage merely
because it is convenient to print values.

Use host visibility only at a declared capability boundary, and include its
synchronization and copy cost in the relevant phase.

### Use compact initializers

Use typed `repeat`, `linear`, and `grid_2d` initializers when state follows a
pattern. Expanding one million literals makes the graph, serialization, validation,
and host startup proportional to the population before GPU work begins.

Compact initialization is canonical graph data, not a hidden backend shortcut.
The validator still checks count, capacity, length, type, and shape.

### Separate committed and transient state

Committed streams survive across ticks. Transient streams hold perception,
intent, scan, reduction, sort, or render-preparation data.

This distinction helps answer:

- Which data must be buffered across overlapping ticks?
- Which data can be overwritten after its last consumer?
- Which streams belong in replay identity?
- Which streams can be recomputed instead of stored?

Pqo does not yet infer all transient aliasing. Declare separate resources unless
aliasing and lifetime safety are explicitly proven.

## Declare kernels as exact effect boundaries

A kernel signature must declare every resource it reaches:

```pqo
kernel integrate {
  slot position: stream<vec3<f32>, m> read_write
  slot velocity: stream<vec3<f32>, m/s> read_write
  slot gravity: value<vec3<f32>, m/s^2> read
  slot dt: value<f32, s> read

  implementation metal {
    source "kernels/euler_integrate.metal"
    entry "integrate_main"
  }
}
```

This signature is a correctness contract and an optimization surface. It lets the
validator derive hazards and lets the backend bind resources without reflection or
hidden lookup.

### Use the narrowest access mode

- `read` — consumes existing state.
- `write` — produces state without depending on its old value.
- `read_write` — consumes and replaces state.
- `atomic` — performs explicitly atomic access.

Do not mark every slot `read_write`. Excess write effects create false hazards,
restrict overlap, and conceal the actual algorithm.

### Distinguish per-invocation and whole-resource access

The default stream slot is per invocation: invocation `gid` owns element `gid`.
Use whole-resource indexing only when a kernel intentionally reaches other
elements, such as a stencil, scan, sort, component relaxation, or reduction.

Whole-resource access is more expensive to reason about and can create broad
hazards. It should answer a specific algorithmic need, not compensate for an
unclear signature.

### Keep aliasing forbidden by default

Binding one stream to multiple kernel slots is illegal unless the ABI explicitly
allows that slot pair. Preserve this default.

If an optimization needs aliasing:

1. Prove that the access pattern is safe for every invocation.
2. Declare only the required slot pair.
3. Add a scenario that detects overwritten or order-dependent results.
4. Benchmark the aliased and non-aliased variants.

### Keep the ABI exact

The current Metal runtime binds buffers in `KernelAbi.binding_order`. The MSL
`[[buffer(n)]]` indices must match that order exactly. Pqo values and streams are
both buffer bindings.

The current dispatch index is a global linear `u32`:

```metal
uint gid [[thread_position_in_grid]]
```

Do not insert an undeclared buffer into Metal, reorder MSL parameters without
updating the ABI, or rely on an ambient constant.

### Let the backend derive threadgroup width first

With no explicit pass override, `pqo-metal` chooses the smaller of:

- the pipeline's `threadExecutionWidth`, and
- its `maxTotalThreadsPerThreadgroup`.

An explicit `threads_per_threadgroup` request must be between 1 and 1024 and must
not exceed the compiled pipeline limit. Override the derived value only after a
representative benchmark demonstrates a stable improvement.

Fixed threadgroup dimensions are appropriate for algorithms whose shared-memory
layout, scan width, or tiling logic requires them. Document that invariant next to
the kernel.

## Build passes around coherent work

A pass binds a reusable kernel to concrete resources and a dispatch domain:

```pqo
pass fall uses integrate {
  bind position = particles.position
  bind velocity = particles.velocity
  bind gravity = world.gravity
  bind dt = simulation.fixed_dt

  dispatch over particles.position
}
```

Choose a dispatch stream whose logical length exactly matches the work. Do not
dispatch maximum capacity and branch away most lanes when a count-backed dynamic
stream can encode the active length.

### Use one invocation per natural work item

Good work items include:

- one particle,
- one field cell,
- one active population member,
- one spatial bin,
- one scan block,
- one reduction partial,
- one render instance.

If one invocation loops over the full population, the program is serial work
wearing a GPU costume.

### Fuse only when reuse beats lost flexibility

Fuse adjacent operations when they:

- share the same dispatch domain,
- read much of the same state,
- require no global synchronization between them,
- and remove meaningful memory traffic or launch overhead.

Keep operations separate when they:

- require a grid-wide completion boundary,
- use different dispatch domains,
- produce reusable or inspectable intermediate state,
- have different execution frequencies,
- or become register-heavy and reduce occupancy when fused.

Pass count alone is not a performance metric. A clean extra pass can be cheaper
than spilling registers or rereading a large neighborhood.

### Use ping-pong state for neighborhood evolution

Never update a field in place when neighboring invocations must all observe the
same prior generation.

Use:

```text
evolve: field.current → field.next
commit or swap: field.next → field.current
```

The crystal follows this pattern for phase, solute, and temperature. The extra
storage and commit pass buy deterministic generation semantics. A future backend
may optimize a proven swap, but the source semantics must remain explicit.

### Make interventions exceptional

Host input, slicing, lesions, picking, and inspection are not ambient mutation.
Represent them as explicit intervention passes or capabilities with declared
authority. Autonomous simulation ticks should not poll mutable CPU object state.

## Schedule for correctness and overlap

Order every conflicting producer and consumer:

```pqo
schedule simulation fixed 120 Hz {
  run fall
  run bounce after fall
  show viewport after bounce
  ...
}
```

The dependency is semantic. The Metal backend chooses encoder, command-buffer, and
queue mechanisms that satisfy it.

### Avoid fake serialization

Do not add dependencies merely to mirror source order. Add them when data,
observation, intervention, or presentation semantics require completion.
Unnecessary edges reduce the backend's freedom to overlap independent work.

Conversely, never omit an edge to chase parallelism. The validator rejects
unordered hazards, but a falsely narrow effect signature can hide the real hazard.

### Choose an explicit reuse policy

The current scheduling policies include:

- `RequireResourceVersions` — allocate enough physical versions for overlap.
- `SerializeConflictingTicks` — prevent overlapping conflicting accesses.
- `QueueOrderedReuse` — reuse through a proved single serial queue.

Presentation has corresponding lifetime policies. Queue-ordered reuse is efficient
when compute and render share one proven serial queue and the state lifetime fits
that ordering. It is not a generic claim that one buffer is always safe.

Increasing in-flight ticks can improve queue occupancy, but it also increases
resource lifetime, buffering requirements, and latency. Measure all three.

### Preserve fixed simulation time under overload

Keep fixed `dt` independent of render cadence. Drop presentation before corrupting
simulation time when that is the declared overload policy. Record decisions needed
for the claimed determinism tier.

## Keep dynamic populations on the GPU

A scalable changing population uses:

```text
capacity-backed member streams
+ scalar active_count
+ membership authority
+ GPU compaction/allocation
+ count-backed dispatch
```

The current Metal backend prepares indirect compute and draw arguments from the
GPU count stream. The host does not read the count before each dispatch.

Required rules:

- Make the count stream a fixed, scalar `u32` stream.
- Keep every member stream at the same capacity and dynamic length source.
- Grant one authoritative membership capability.
- Bound every append by capacity.
- Define deterministic identity and survivor ordering when replay matters.
- Test empty, one-element, full-capacity, birth, death, and overflow cases.

Do not implement population change as:

```text
read active count to CPU
→ resize host arrays
→ upload members
→ dispatch
```

That adds a synchronization point, a transfer, and CPU work proportional to the
population—the exact costs count-backed streams are intended to remove.

## Use scalable GPU algorithms

### Bound neighborhoods spatially

For local interaction, use a grid, spatial bins, or another bounded acceleration
structure:

```text
assign elements to bins
→ count bounded occupancy
→ scan offsets
→ compact indices
→ inspect nearby bins only
```

State the maximum bin occupancy or overflow policy. An unbounded bin silently
reintroduces worst-case quadratic work.

### Use hierarchical scan and compaction

For filtering or population updates:

```text
mark candidates
→ scan within fixed-size blocks
→ scan block totals
→ scatter survivors or births
→ commit authoritative count
```

Keep the scan block width consistent with its fixed threadgroup assumptions. Use a
stable ordering key when deterministic replay requires one.

### Use reductions in stages

Avoid one global atomic for a hot metric. Prefer:

```text
per-invocation contribution
→ per-threadgroup partial
→ small final reduction
```

Atomics are appropriate for low-contention counters, bounded reservations, or
integer reductions whose ordering semantics are acceptable. Floating-point atomics
and non-associative reductions need an explicit determinism analysis.

### Iterate relaxation with a bound

Connected-component labeling and constraint propagation often use repeated local
relaxation. Declare a fixed conservative round count or a GPU-resident convergence
test with a bounded maximum.

Do not read a convergence flag back to the CPU after every round. If correctness
depends on convergence, audit the chosen bound in scenarios across maximum supported
dimensions.

### Quantize only for a reason

Quantized perception bins, integer ledgers, and stable IDs can improve deterministic
comparison and reduce bandwidth. Record scale, saturation, and rounding behavior.
Never let an undocumented cast become part of the physics.

## Render directly from simulation state

Prefer:

```text
simulation streams
→ GPU render-preparation pass, if needed
→ view
→ instanced or point rendering
```

Do not build a CPU render list each frame.

A render-preparation pass is useful when the view needs:

- compacted visible instances,
- derived color, radius, or normal,
- camera-space projection data,
- stable interpolation versions,
- or a different active set from simulation.

Otherwise, let the view read committed simulation streams directly. Treat render
streams as transient projections, not a second authoritative world.

When presentation falls behind, apply the schedule's explicit view state and
overload policy. Never allow a dropped frame to change simulation results.

## Author the current Metal implementation

Metal supplies the hot-path arithmetic in v0. Keep it mechanically aligned with the
Pqo graph.

### Match types and bindings

For every kernel:

1. Copy the Pqo ABI binding order into a review table.
2. Verify each MSL `[[buffer(n)]]` index.
3. Verify scalar/vector width and signedness.
4. Verify `constant` versus `device` address space.
5. Verify read-only `const` on read slots.
6. Verify atomic types match atomic slots and operations.
7. Verify the global index type and dispatch domain.

Example:

```metal
kernel void integrate_main(
    device packed_float3* position [[buffer(0)]],
    device packed_float3* velocity [[buffer(1)]],
    constant packed_float3& gravity [[buffer(2)]],
    constant float& dt [[buffer(3)]],
    uint gid [[thread_position_in_grid]])
{
    float3 v = float3(velocity[gid]) + float3(gravity) * dt;
    velocity[gid] = packed_float3(v);
    position[gid] = packed_float3(float3(position[gid]) + v * dt);
}
```

The conversion between packed storage and arithmetic vectors is deliberate.

### Make bounds semantics obvious

For fixed and dynamic Pqo dispatch, the runtime emits the declared logical thread
count. A kernel may rely on `gid` being inside that dispatch domain only if every
bound stream shares the validated length.

For fixed dispatches that can exceed a resource's logical length, add an explicit
bound value and guard. Never rely on buffer over-allocation.

### Favor coherent branches

Branches based on global phase, tick parity, or spatially coherent regions are
usually cheaper than random per-element divergence. Move rare exceptional work
into a separate compacted pass when the branch causes most lanes to idle.

Do not remove a clear early exit without measuring. Skipping expensive neighborhood
work for inactive elements may outweigh divergence.

### Control memory traffic

- Load a reused scalar or element once into a local value.
- Coalesce neighboring accesses where the data layout permits it.
- Avoid writing an unchanged stream only when the Pqo effect and downstream
  semantics also permit omission.
- Use threadgroup memory for measured reuse, not as a reflex.
- Watch register and threadgroup-memory growth because both can lower occupancy.

### Keep backend tricks behind target-neutral semantics

SIMD-group operations, threadgroup tiling, function constants, and packed formats
may optimize Metal lowering. They must not change the target-neutral meaning of the
program. Fingerprint backend artifacts when making reproducibility claims.

## Measure instead of guessing

Use the smallest benchmark that answers the question.

### Establish correctness first

Run:

```text
./scripts/test-language.sh
cargo test --workspace
```

Add deterministic scenarios for boundaries, interventions, invariants, and dynamic
population changes before performance tuning.

### Separate benchmark modes

- **Headless** isolates scheduled compute.
- **Rendered** adds the plan-driven offscreen view.
- **Presented** includes drawable acquisition and actual presentation.

Example:

```text
./scripts/run-hello-particle.sh batch 1m --bench headless \
  --warmup-seconds 5 --duration-seconds 10

./scripts/run-hello-particle.sh batch 1m --bench presented \
  --pace 120 --warmup-seconds 5 --duration-seconds 10
```

Use sustained wall-time runs for release claims. Tiny sample counts are smoke
proofs, not baselines.

### Read the full result

At minimum inspect:

- GPU p50, p95, p99, and maximum,
- end-to-end latency and deadline misses,
- CPU encoding and queue-admission time,
- initialization blits,
- steady-state copies and blits,
- GPU buffer bytes and peak resident set,
- skipped presentations and drawable starvation,
- artifact, source, device, OS, pipeline, layout, dispatch, and schedule identity.

Mean time can hide deadline-breaking tails.

### Compare orchestration fairly

The `pqo` and `direct-metal` benchmark runners share initialized buffers, MSL,
pipeline states, threadgroup sizing, dispatch, command-buffer grouping, and render
target. Use this comparison to isolate steady-state Pqo orchestration overhead,
not to compare two different algorithms.

Use a clean tree and interleaved ordering for publishable evidence:

```text
./scripts/benchmark-hello-batch-clean.sh 100k rendered 30 60 4
```

### Change one performance hypothesis at a time

Record:

```text
hypothesis
→ graph or kernel change
→ correctness evidence
→ benchmark command
→ fingerprint
→ before/after distributions
→ memory and determinism effects
→ keep or revert decision
```

## Avoid common failure modes

| Failure | Why it is slow or unsafe | Pqo-shaped repair |
| --- | --- | --- |
| CPU loop over particles | Serial host work and upload | Dispatch over a GPU stream |
| Per-element object graph | Poor locality and hidden allocation | Dense typed streams |
| Read active count each tick | GPU/CPU synchronization | Count-backed indirect dispatch |
| Dispatch capacity, ignore most lanes | Wasted work | Dynamic logical length or compaction |
| In-place stencil update | Cross-lane order dependence | Current/next streams |
| Every slot is `read_write` | False hazards and unclear effects | Narrow access modes |
| Hidden kernel resource | Validator cannot prove safety | Add a typed slot and binding |
| Broad whole-resource access | Conceals neighborhood and hazards | Bound the reach explicitly |
| Global hot atomic | Contention and unstable tails | Hierarchical reduction |
| Unbounded neighbor scan | Quadratic worst case | Spatial bins with bounded occupancy |
| CPU render list | Readback, allocation, and upload | GPU render preparation and instancing |
| Extra dependency “for safety” | Prevents legal overlap | Declare actual effects and hazards |
| Excess in-flight work | Memory growth and latency | Tune bounded concurrency |
| Benchmark only the mean | Misses tail latency | Report p95, p99, maximum, misses |
| Claim generated Pqo kernels today | Misstates compiler maturity | Label external Metal boundary |

## Follow the authoring workflow

### 1. Write the execution budget

Declare maximum capacity, update rate, memory budget, observation needs, and
determinism tier.

### 2. Inventory state

For each property, record:

```text
name | type/unit | capacity | length source | committed/transient
readers | writer | storage | buffering | initialization
```

### 3. Draw the pass graph

Name one natural work item per pass. Mark reads, writes, atomics, whole-resource
reach, dispatch domains, and completion edges.

### 4. Choose scalable algorithms

Replace global searches with bounded neighborhoods, quadratic interaction with
spatial indexing, host population management with GPU scan/compaction, and hot
atomics with staged reductions.

### 5. Declare authority

Assign state, membership, host mutation, inspection, and external capabilities.
Keep ordinary kernel effects separate from exceptional authority.

### 6. Implement the typed graph

Use canonical Pqo text where supported by the current language path and the Rust
builder for implemented specimens that do not yet have parser coverage. Preserve
stable names and exact units, types, bindings, and dependencies.

### 7. Implement or generate the backend kernel

For v0, author MSL against the declared ABI. For a future native kernel compiler,
inspect generated Metal and retain the Pqo-level performance contract.

### 8. Validate correctness and lifetime

Run the validator and scenarios. Repair missing bindings, type/unit mismatches,
illegal aliasing, unordered hazards, insufficient versions, unproven queue reuse,
and unsafe presentation lifetime before benchmarking.

### 9. Benchmark representative scale

Test small correctness cases, boundary capacity, and the intended production
population. Warm the pipelines and run long enough to observe tails and overload.

### 10. Record the evidence boundary

Call a short run a smoke proof. Call a dirty-tree result development evidence.
Publish a baseline only with a clean source identity, command, environment,
fingerprint, distribution, and stated limitations.

## Use the agent protocol

When an agent is asked to write or optimize Pqo code, follow this procedure:

1. Read the language charter, semantic model, scheduling rules, and this handbook.
2. Inspect the actual typed graph, external kernel, runtime lowering, and benchmark
   for the target specimen. Do not infer capability from proposed syntax.
3. State whether the change targets executable v0, canonical text, or future syntax.
4. Produce a state inventory and pass/effect table before making a structural
   optimization.
5. Preserve explicit types, units, authorities, bindings, dispatch domains, and
   completion dependencies.
6. Reject CPU work, allocation, copying, readback, or synchronization proportional
   to population unless the user explicitly accepts the cost.
7. Prefer bounded GPU algorithms and document capacity or convergence limits.
8. Change graph declarations and Metal ABI together.
9. Add or update correctness scenarios before trusting benchmark improvements.
10. Report commands, fingerprints, distributions, memory changes, and limitations.

Use this compact response template:

```text
Status:
  executable v0 | canonical text | conceptual future

Budget:
  capacity, rate, memory, readback, determinism

State:
  committed streams, transient streams, dynamic count

Graph:
  passes, effects, dispatch, dependencies, capabilities

GPU algorithm:
  work item, neighborhood, scan/reduction strategy, bounds

ABI:
  binding order, types, indexing, threadgroup assumptions

Proof:
  validation, scenarios, benchmark command, fingerprint

Result:
  correctness, latency distribution, memory, transfers, limitations
```

An agent must not:

- fabricate compiler commands or syntax,
- silently weaken a contract,
- hide an external Metal dependency,
- remove a dependency without re-deriving hazards,
- turn inspection into steady-state readback,
- or describe an unmeasured optimization as a performance result.

## Review with the final checklist

### Semantics

- [ ] Persistent mutable state is in named streams.
- [ ] Types and physical units match every binding.
- [ ] Each stream has explicit capacity, logical length, storage, and authority.
- [ ] Kernel slots declare every read, write, atomic, and whole-resource access.
- [ ] ABI binding order matches the Metal buffer indices.
- [ ] Aliasing is forbidden or narrowly declared and tested.
- [ ] Every producer/consumer hazard has a semantic dependency.
- [ ] Tick overlap and presentation lifetime are proven.

### GPU scalability

- [ ] No CPU loop, allocation, upload, or readback scales with active elements per tick.
- [ ] State remains GPU-resident through compute and render.
- [ ] The dispatch domain matches logical work.
- [ ] Neighborhood size, bin occupancy, iteration count, and capacity are bounded.
- [ ] Scans, compaction, sorting, and reductions are hierarchical where needed.
- [ ] Hot global atomics and random whole-population access are absent or justified.
- [ ] Stencil updates use stable prior-generation state.
- [ ] Dynamic populations use GPU-resident count-backed dispatch.

### Performance evidence

- [ ] Correctness passes before performance is claimed.
- [ ] Initialization and steady state are reported separately.
- [ ] Headless, rendered, and presented costs are not conflated.
- [ ] p95, p99, maximum, and deadline misses accompany averages.
- [ ] GPU bytes, resident set, copies, blits, and readbacks are recorded.
- [ ] Threadgroup overrides have benchmark evidence.
- [ ] The artifact fingerprint and source cleanliness are recorded.
- [ ] Smoke proofs, development evidence, and release baselines are labeled honestly.

The target is not “Metal-looking Pqo.” The target is a program whose semantics
make dense memory, GPU residency, bounded communication, explicit hazards, direct
rendering, and reproducible measurement the easiest correct implementation.
