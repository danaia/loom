# Executable Loom 0.1

Use this as a working map, then defer to `docs/agent-coding-reference.md`, the
current parser, and `loom check` for literal source authority.

## Contents

- [Fixed shape](#fixed-shape)
- [Typed state](#typed-state)
- [Native kernel boundary](#native-kernel-boundary)
- [External Metal boundary](#external-metal-boundary)
- [Pass and flow](#pass-and-flow)
- [View](#view)
- [Native versus external decision](#native-versus-external-decision)
- [Diagnostic repair](#diagnostic-repair)
- [Common valid patterns](#common-valid-patterns)

## Fixed shape

```loom
loom 0.1
module simple_snake_case_name
target metal
```

A module contains `const`, `stream`, `kernel`, `pass`, `view`, and `flow`
declarations and must contain at least one flow.

## Typed state

Accepted scalar/vector spellings include:

```text
bool i32 u32 f16 f32
i32x2..i32x4 u32x2..u32x4 f16x2..f16x4 f32x2..f32x4
```

Native kernel generation currently supports f32 scalars and f32 vectors only.
Physical units use `m`, `kg`, `s`, multiplication, division, and integer powers:

```loom
const world.gravity: f32x3<m/s^2> = [0, -9.81, 0]

stream particle.position: f32x3<m> {
  cap=1 len=1 buffers=1 access=rw storage=device
  init=[[0, 0, 0]]
}
```

Write all stream structural properties. Keep `len <= cap`; keep aligned streams
at compatible lengths; use the narrowest access and `device` unless real host
sharing is required.

## Native kernel boundary

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

Supported now:

- `f32` and `f32x2..f32x4`;
- value reads by name;
- current element stream reads/writes as `name[i]`;
- `=`, `+=`, `-=`, `*=`, `/=`;
- `+`, `-`, `*`, `/`, parentheses, negative numeric literals;
- sequential assignments.

Unsupported in native bodies:

- conditionals, comparisons, booleans, locals, loops;
- component access, constructors, calls, and math intrinsics;
- neighbor indexing, textures, sampling, atomics, reductions;
- scans, compaction, threadgroup/SIMD operations;
- native vertex or fragment stages.

Do not approximate these operations with invented syntax. Use `extern metal`.

## External Metal boundary

```loom
kernel contact(
  position: rw stream<f32x3,m>,
  radius: in stream<f32,m>,
  ground: in value<f32,m>
) extern metal {
  source="kernels/contact.metal"
  entry="contact_main"
}
```

Use `all` only for intentional whole-resource access:

```loom
neighbors: in all stream<u32>
counter: atomic all stream<u32>
```

The Loom parameter order is the Metal buffer order. Typical mappings:

```text
in stream<f32>      → const device float*
rw stream<f32>      → device float*
in stream<f32x3>    → const device packed_float3*
in value<f32>       → constant float&
in value<f32x3>     → constant packed_float3&
atomic all u32      → device atomic_uint*
dispatch index      → uint gid [[thread_position_in_grid]]
```

The runtime encodes Loom f32x3 streams and values as 12 packed bytes. Use
`packed_float3` at either ABI boundary and convert to `float3` for arithmetic.
Never insert an undeclared Metal buffer.

## Pass and flow

```loom
pass move = integrate(
  position=particle.position
  velocity=particle.velocity
  acceleration=world.gravity
  dt=simulation.fixed_dt
) over particle.position

flow simulation rate=120hz {
  move
}
```

Every parameter requires exactly one named binding. The `over` stream supplies
the logical dispatch length. `simulation.fixed_dt` is an explicit typed value,
not an ambient global.

Order hazards explicitly:

```loom
flow simulation rate=120hz {
  simulate -> project
  draw viewport after project
}
```

## View

Views are external Metal:

```loom
view viewport(
  color=render.color
  position=render.position
  radius=render.radius
) extern metal {
  source="shaders/particle.metal"
  entry="particle_pipeline"
}
```

The runtime derives vertex and fragment entries from the declared pipeline entry
using the established project convention. Preserve a working view ABI when
extending an example.

## Native versus external decision

```text
Same-index f32 arithmetic only?
├─ yes → native `each`
└─ no
   ├─ GPU compute with declared buffers? → `extern metal` kernel
   ├─ rendering?                          → `extern metal` view
   ├─ host input/control?                 → project runtime extension
   └─ new reusable language semantics?    → compiler vertical slice
```

External Metal is part of the current language design. It preserves accuracy and
enables advanced GPU systems while the native compiler grows through measured
gates.

## Diagnostic repair

Run `loom check` and read `status` first:

| Status | Meaning |
| --- | --- |
| `valid` | source and graph passed |
| `source_invalid` | lex, parse, lower, native type/unit, or generation error |
| `graph_invalid` | graph safety, binding, access, length, order, or ABI error |
| `io_error` | project path could not be loaded |
| `package_error` | `.lmp` construction failed |

Repair mechanically:

1. Read the first diagnostic code, message, and span.
2. Inspect the containing declaration and its direct references.
3. Fix that cause without redesigning unrelated code.
4. Re-run `loom check`.
5. Once valid, run `loom explain` and inspect bindings and order.
6. Run and observe.

Never respond to a diagnostic by deleting physical units, broadening every
stream to `rw`, marking every stream `all`, or adding arbitrary order edges.

## Common valid patterns

### Damping

```loom
const material.damping: f32 = 0.99

kernel damp(
  velocity: rw stream<f32x3,m/s>,
  damping: in value<f32>
) each i {
  velocity[i] *= damping;
}
```

### Separate simulation and render projection

```text
authoritative simulation streams
→ optional GPU projection/culling pass
→ transient render streams
→ view
```

Use this when camera-space values, colors, normals, or visible instances are
derived. Do not copy the render list through the CPU.

### Ping-pong neighborhood evolution

```text
read field.current
→ write field.next
→ completion boundary
→ commit/swap roles
```

Implement the current arithmetic externally. Never update a field in place when
neighbors must observe the same prior generation.
