# Loom Coding Reference for AI Agents

Status: executable Loom 0.1 reference  
Audience: coding models that need to create or modify meaningful GPU experiments  
Target: Apple Silicon and Metal

This document is deliberately literal. Follow the forms shown here. Do not
invent syntax from CUDA, Rust, Python, WGSL, or future Loom design documents.

## 1. Required agent behavior

When asked to build a Loom experiment:

1. State one testable question.
2. Choose the closest working source:
   - arithmetic or particle motion:
     `examples/hello-particle/hello-particle.loom`
   - fields, fracture, components, healing, or interactive 3D material:
     `examples/hello-crystal/crystal.loom`
3. Copy the working source before changing it.
4. Change one behavior class at a time.
5. Keep every resource type, unit, access mode, and binding explicit.
6. Use native Loom only for the operations listed in this document.
7. Keep unsupported work visibly declared as `extern metal`.
8. Run `loom check` after every structural change.
9. Run `loom explain` before claiming what Metal or pass order Loom produced.
10. Run the program before claiming that it works.

Never report success when `status` is not `valid`. Never claim speed without a
benchmark. Never describe a visual result that was not actually observed.

## 2. The executable mental model

A Loom program is a typed GPU graph:

```text
constants + streams
→ kernels
→ passes with complete bindings
→ one ordered flow
→ optional Metal view
```

- A `const` is immutable typed input.
- A `stream` is an allocated GPU array.
- A `kernel` declares one parallel operation and every resource it can access.
- A `pass` binds a kernel parameter to each concrete resource.
- `over STREAM` dispatches one invocation per logical element of that stream.
- A `flow` orders passes and optionally presents a view.
- A `view` renders completed streams through a declared Metal pipeline.

Names do not imply access. A kernel can access only its parameters. A pass must
bind every parameter.

## 3. Fixed module shape

Every source begins with this exact header:

```loom
loom 0.1
module experiment_name
target metal
```

Rules:

- The only accepted language version is `0.1`.
- The only accepted target is `metal`.
- Use a simple `snake_case` module name.
- `//` starts a line comment.
- Semicolons are optional after top-level constants, streams, passes, flows,
  and Metal properties. Semicolons are required after native kernel statements.
- A module must contain at least one `flow`.

Top-level declarations may be `const`, `stream`, `kernel`, `pass`, `view`, or
`flow`.

## 4. Types, units, and literals

### Accepted data types

```text
bool
i32 u32 f16 f32
i32x2 i32x3 i32x4
u32x2 u32x3 u32x4
f16x2 f16x3 f16x4
f32x2 f32x3 f32x4
```

Important current limits:

- Native Loom kernel generation supports only `f32` and `f32x2..f32x4`.
- `bool`, integer, and `f16` resources can be declared for external Metal
  kernels, subject to validator and runtime support.
- `f16` source literals are not implemented.

### Accepted physical units

The unit bases are:

```text
m   kg   s
```

Use `*`, `/`, and integer exponents:

```loom
f32<m>
f32x3<m/s>
f32x3<m/s^2>
f32<kg*m/s^2>
```

Omitting a unit means dimensionless. `<1>` is also dimensionless.

Unit rules inside a native kernel:

- `+` and `-` require the same type and the same unit.
- `*` multiplies units.
- `/` divides units.
- Assignment must exactly match the target type and unit.

### Accepted literals

```loom
true
false
12
-3
0.25
1e-4
[0, -9.81, 0]
[[0, 1, 0], [0.2, 1, 0]]
```

A stream initializer is always an array containing one element literal per
initialized stream element.

## 5. Constants

Form:

```loom
const NAME: TYPE = LITERAL
const NAME: TYPE<UNIT> = LITERAL
```

Examples:

```loom
const world.gravity: f32x3<m/s^2> = [0, -9.81, 0]
const material.drag: f32 = 0.98
const field.width: u32 = 100
```

Constants are not ambient globals. A kernel declares them as `value`
parameters, and a pass binds them.

`simulation.fixed_dt` is a built-in immutable value created by a fixed-rate
flow. Its type is `f32<s>`. Bind it explicitly when a kernel needs the timestep.

## 6. Streams

Form:

```loom
stream NAME: TYPE {
  cap=CAPACITY len=LENGTH buffers=BUFFER_COUNT access=ACCESS storage=STORAGE
  init=[ELEMENTS]
}
```

Units follow the type:

```loom
stream particles.position: f32x3<m> {
  cap=4 len=4 buffers=1 access=rw storage=device
  init=[[0, 0, 0], [0.2, 0, 0], [-0.2, 0, 0], [0, 0.2, 0]]
}
```

Properties:

| Property | Meaning | Accepted values |
| --- | --- | --- |
| `cap` | allocated element capacity | unsigned integer |
| `len` | fixed logical element count | unsigned integer |
| `buffers` | physical buffer versions | positive integer |
| `access` | maximum stream authority | `r`, `rw`, `host_rw` |
| `storage` | Metal storage class | `device`, `shared` |
| `init` | initial logical elements | typed array literal |

Defaults exist, but agents should write all five structural properties.

Rules:

- `len` cannot exceed `cap`.
- Initial data must match the stream type.
- For a fixed-length stream, the initializer element count must equal `len` and
  cannot exceed `cap`.
- Streams dispatched or rendered together normally need compatible lengths.
- Use the narrowest access that permits all bound kernel effects.
- Use `device` unless host-shared access is genuinely required.
- Increasing a fixed population requires updating every aligned stream's
  `cap`, `len`, and initializer count.

## 7. Native Loom kernels

Native form:

```loom
kernel NAME(
  PARAMETER: ACCESS RESOURCE<TYPE,UNIT>,
  PARAMETER: ACCESS RESOURCE<TYPE>
) each INDEX {
  STATEMENTS
}
```

Accepted parameter access:

| Source | Meaning |
| --- | --- |
| `in` or `r` | read |
| `out` or `w` | write |
| `rw` | read and write |
| `atomic` | atomic access for external Metal |

Accepted resource kinds:

```text
stream
value
```

Values must use `in`. Streams may use the other access modes when the enclosing
stream authority allows them.

### Native operations available now

Native Loom currently accepts:

- reading `value` parameters by name,
- reading the current stream element as `stream_name[i]`,
- assigning the current stream element,
- `=`, `+=`, `-=`, `*=`, `/=`,
- `+`, `-`, `*`, `/`,
- parentheses,
- negative numeric literals,
- multiple sequential assignment statements.

Numeric literals in native expressions are dimensionless `f32` values.

Example:

```loom
kernel integrate(
  position: rw stream<f32x3,m>,
  velocity: rw stream<f32x3,m/s>,
  acceleration: in value<f32x3,m/s^2>,
  dt: in value<f32,s>
) each i {
  velocity[i] += acceleration * dt;
  position[i] += velocity[i] * dt;
}
```

The invocation index owns element `i`. Native code must read and write with
that same index.

### Native operations not available yet

Do not generate these inside `each`:

- `if`, `else`, comparisons, or boolean expressions,
- local variables or `let`,
- vector components such as `.x`,
- constructors or function calls,
- math intrinsics such as `sin`, `sqrt`, or `clamp`,
- loops,
- random numbers,
- indexing a neighbor such as `position[i + 1]`,
- textures or field sampling,
- atomics or reductions,
- scans or compaction,
- threadgroup or SIMD-group operations,
- native vertex or fragment stages.

Use an existing external Metal kernel for those operations. Do not write
conceptual future syntax into an executable `.loom` file.

## 8. External Metal kernels

External form:

```loom
kernel contact_ground(
  position: rw stream<f32x3,m>,
  velocity: rw stream<f32x3,m/s>,
  radius: in stream<f32,m>,
  ground_height: in value<f32,m>
) extern metal {
  source="kernels/ground_contact.metal"
  entry="ground_contact_main"
}
```

`all` means a kernel may access the whole stream rather than only its
corresponding element:

```loom
damage: in all stream<f32>
counter: atomic all stream<u32>
```

Use `all` only for a real whole-resource algorithm such as a stencil,
neighborhood lookup, reduction, component pass, or global counter.

The kernel parameter order is the Metal buffer ABI order:

```text
first parameter  → [[buffer(0)]]
second parameter → [[buffer(1)]]
...
```

Typical Metal mapping:

```text
in stream<f32>      → const device float*
rw stream<f32>      → device float*
in stream<f32x3>    → const device packed_float3*
in value<f32>       → constant float&
atomic all u32      → device atomic_uint*
dispatch index      → uint gid [[thread_position_in_grid]]
```

In the installed Loom 0.1 runtime, executable external sources are packaged:

```text
kernels/ground_contact.metal
shaders/particle.metal
kernels/crystal.metal
shaders/crystal.metal
```

Do not invent another source path and expect the installed runtime to load it.
Adding a new external Metal implementation is compiler-repository work: add the
Metal source, package it in the runtime, test it on Metal, rebuild Loom, and
release it.

## 9. Passes

Form:

```loom
pass PASS_NAME = KERNEL_NAME(
  parameter=resource.name
  parameter=resource.name
) over DISPATCH_STREAM
```

Example:

```loom
pass move = integrate(
  position=particles.position
  velocity=particles.velocity
  acceleration=world.acceleration
  dt=simulation.fixed_dt
) over particles.position
```

Rules:

- Bind every kernel parameter exactly once.
- Use the parameter name on the left and declared resource name on the right.
- Binding type, unit, access, and reach must match.
- Do not add undeclared bindings.
- `over` must name a stream.
- The dispatch count is the logical length of that stream.
- The kernel may not silently access another resource.

## 10. Views

Current views are external Metal:

```loom
view viewport(
  color=particles.color
  position=particles.position
  radius=particles.radius
) extern metal {
  source="shaders/particle.metal"
  entry="particle_pipeline"
}
```

The packaged particle renderer expects the bindings `color`, `position`, and
`radius`. Keep these names and compatible stream types when reusing it.

The packaged Crystal renderer is already wired by
`examples/hello-crystal/crystal.loom`. Modify that graph conservatively instead
of reconstructing the render ABI from memory.

## 11. Flows

Form:

```loom
flow FLOW_NAME rate=INTEGERhz {
  first_pass -> second_pass -> third_pass
  draw VIEW_NAME after PRODUCER_PASS
}
```

Examples:

```loom
flow simulation rate=120hz {
  move
  draw viewport after move
}
```

```loom
flow simulation rate=120hz {
  fall -> bounce
  draw viewport after bounce
}
```

Rules:

- Rate is a positive integer followed immediately by `hz`.
- The arrow chain is the execution order.
- `draw` is optional for validation, but a visible application needs a view.
- Draw only after the pass that finishes all view inputs.
- Put ordinary simulation passes in the flow.
- Unscheduled passes are reserved for runtime interventions. Do not create them
  unless extending an existing interaction protocol such as Hello Crystal.

## 12. Complete runnable experiment

This program asks: how do four particles with different initial velocities move
under the same weak acceleration?

Save it as `four-particle-drift.loom`.

```loom
loom 0.1
module four_particle_drift
target metal

const world.acceleration: f32x3<m/s^2> = [0, -0.15, 0]

stream particles.position: f32x3<m> {
  cap=4 len=4 buffers=1 access=rw storage=device
  init=[[-0.45, 0.35, 0], [-0.15, 0.1, 0], [0.15, -0.1, 0], [0.45, -0.35, 0]]
}

stream particles.velocity: f32x3<m/s> {
  cap=4 len=4 buffers=1 access=rw storage=device
  init=[[0.10, 0.02, 0], [0.04, 0.08, 0], [-0.04, 0.08, 0], [-0.10, 0.02, 0]]
}

stream particles.radius: f32<m> {
  cap=4 len=4 buffers=1 access=r storage=device
  init=[0.035, 0.035, 0.035, 0.035]
}

stream particles.color: f32x4 {
  cap=4 len=4 buffers=1 access=r storage=device
  init=[[1, 0.2, 0.2, 1], [1, 0.8, 0.2, 1], [0.2, 0.8, 1, 1], [0.7, 0.3, 1, 1]]
}

kernel integrate(
  position: rw stream<f32x3,m>,
  velocity: rw stream<f32x3,m/s>,
  acceleration: in value<f32x3,m/s^2>,
  dt: in value<f32,s>
) each i {
  velocity[i] += acceleration * dt;
  position[i] += velocity[i] * dt;
}

pass move = integrate(
  position=particles.position
  velocity=particles.velocity
  acceleration=world.acceleration
  dt=simulation.fixed_dt
) over particles.position

view viewport(
  color=particles.color
  position=particles.position
  radius=particles.radius
) extern metal {
  source="shaders/particle.metal"
  entry="particle_pipeline"
}

flow simulation rate=120hz {
  move
  draw viewport after move
}
```

Validate and inspect it:

```sh
loom check four-particle-drift.loom
loom explain four-particle-drift.loom
loom four-particle-drift.loom
```

Safe experiment variations:

- Change only `world.acceleration`.
- Change initial velocity while preserving `m/s`.
- Compare `rate=60hz` and `rate=120hz`.
- Add particles by updating all four streams' `cap`, `len`, and `init`.
- Add a dimensionless damping constant and a statement such as
  `velocity[i] *= damping`.

Record the source graph hash for each variant. A changed hash proves the source
graph changed; it does not prove that the scientific hypothesis is correct.

## 13. Building complex experiments

For fields, growth, fracture, healing, component labeling, or interactive 3D
rendering, begin with:

```sh
cp examples/hello-crystal/crystal.loom my-experiment.loom
loom check my-experiment.loom
```

Then use this order:

1. Rename the module with a simple identifier.
2. State the hypothesis.
3. Change one constant or initializer.
4. Run `loom check`.
5. Use `loom explain` to confirm the bindings and pass order.
6. Run and observe the same number of ticks or the same interaction.
7. Save the source hash and observation.
8. Only then modify a kernel boundary or add a pass.

Examples of bounded Crystal questions:

- How does `growth.rate` change early visible growth?
- How does `growth.anisotropy_strength` change faceting?
- How does `healing.damage_rate` change the time a cut remains red?
- How does `healing.reassembly_rate` change seam closure?
- How does slice radius change removed volume?

Keep the one-million-cell field aligned unless deliberately running a smaller
performance experiment:

```text
field.width = 100
every aligned field/material/render stream cap = 1,000,000
every aligned field/material/render stream len = 1,000,000
seed index = 505050
```

For a cubic width `W`, the cell count is `W × W × W`. Do not change width
without changing all aligned counts and choosing an in-range seed.

## 14. Diagnostic repair loop

Run:

```sh
loom check program.loom
```

Read the JSON `status` first:

| Status | Meaning | Action |
| --- | --- | --- |
| `valid` | parsing and graph validation passed | inspect, then run |
| `source_invalid` | lexer, parser, lowering, native type, or unit failure | repair the reported span |
| `graph_invalid` | typed graph safety or scheduling failure | repair bindings, access, lengths, or order |
| `io_error` | source file could not be read | correct the path or permissions |

Diagnostic code families:

| Prefix | Area |
| --- | --- |
| `L` | lexical characters and strings |
| `P` | source grammar |
| `S` | lowering into the typed graph |
| `T` | native expression types, units, indexing, and effects |
| `M` | native Metal generation |

Mechanical repair algorithm:

```text
1. Read diagnostics[0].code, message, and span.
2. Inspect only the declaration containing that span.
3. Fix the first error without redesigning unrelated code.
4. Run loom check again.
5. Repeat until status is valid.
6. Run loom explain.
7. Confirm native_kernels/external_kernels and execution order.
8. Run the program.
```

Common repairs:

| Error | Likely cause | Repair |
| --- | --- | --- |
| unknown resource kind `all` | `all` placed before `stream` access incorrectly | use `in all stream<T>` |
| stream must be indexed | native expression used `position` | use `position[i]` |
| value cannot be indexed | native expression used `dt[i]` | use `dt` |
| does not declare read access | an `out` stream is read | change to `rw` or stop reading it |
| assignment type/unit mismatch | dimensional arithmetic is wrong | derive units and fix the expression |
| missing binding | pass omitted a parameter | add exactly one named binding |
| access violation | pass writes an `access=r` stream | change authority only if writing is intended |
| illegal alias | one stream bound to incompatible slots | use separate streams |
| unordered hazard | conflicting passes lack an order | add them to one arrow chain |
| invalid logical length | aligned resources disagree | make lengths compatible |
| unsupported packaged Metal source | invented `source` path | use a packaged source or extend/rebuild runtime |

Do not delete units, broaden every stream to `rw`, or mark every parameter
`all` merely to silence diagnostics. Those changes erase safety information.

## 15. Evidence required before completion

An AI-generated experiment is complete only when it provides:

```text
Question:
Independent variable:
Controlled inputs:
Observed stream or visual behavior:
Native Loom kernels:
External Metal kernels:
loom check result:
source_graph_hash:
artifact_fingerprint:
loom explain inspection:
run result:
limitations:
```

Minimum command sequence:

```sh
loom check PATH
loom explain PATH
loom PATH
```

For compiler or runtime changes in the Loom repository, also run the closest
focused Rust test and a real-Metal test. If performance is part of the claim,
use a release build, warm-up, repeated samples, fixed workload, and report the
device.

## 16. Compact prompt for a coding model

Use this prompt with this document:

```text
Read docs/agent-coding-reference.md completely.

Create one runnable Loom experiment that answers this question:
[QUESTION]

Start from the closest checked-in example. Use only executable Loom 0.1 syntax.
Do not invent conditionals, loops, intrinsics, random functions, neighbor
indexing, atomics, reductions, scans, or native render syntax. Keep unsupported
operations visibly extern metal and use only packaged Metal source paths unless
you are explicitly modifying and rebuilding the Loom runtime.

Change one behavior class at a time. Preserve types, physical units, resource
effects, complete pass bindings, compatible stream lengths, and explicit flow
order.

Finish only after:
1. loom check reports status valid;
2. loom explain confirms the intended kernels, bindings, and order;
3. loom PATH runs successfully;
4. you report the question, changed variables, native/external boundary,
   hashes, observed result, and limitations.
```

## 17. Authority order

When sources disagree, trust them in this order:

1. the current parser and validator,
2. `loom check` and `loom explain`,
3. checked-in runnable `.loom` examples,
4. this document,
5. conceptual design and roadmap documents.

The compiler is the final authority on accepted syntax. The Metal runtime is the
final authority on executable packaged sources.
