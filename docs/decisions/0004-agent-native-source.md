# Decision 0004: Agent-Native Canonical Source

## Status

Accepted for the first executable syntax slice.

## Problem

The original Hello Particle specimen repeats kernel slots, ABI order, pass
bindings, and schedule structure. Its intended semantics are sound, but that
repetition increases the number of tokens an agent must generate and the number
of places it must keep synchronized.

Pqo needs a canonical source representation optimized for reliable agent
generation, inspection, diagnosis, and repair. Optimizing for agents does not
mean minifying the source. The objective is to minimize total generation and
repair work.

## Decision

The first executable agent-native syntax uses these rules:

- `pqo 0.1`, `module`, and `target` form a fixed header.
- Kernel parameter order is the backend ABI order.
- Kernel parameter access is the effect signature.
- `stream<T,unit>` and `value<T,unit>` distinguish resource kinds.
- A pass contains named bindings and one explicit dispatch domain.
- A `flow` arrow chain creates completion dependencies.
- `draw <view> after <pass>` creates an explicit presentation dependency.
- External Metal views keep rendering source and stream bindings inspectable.
- Declaration order does not determine semantic identity.
- Native `each` kernels support indexed assignment and arithmetic expressions.
- Native expressions are type- and unit-checked before Metal generation.
- The parser lowers into the existing canonical `ModuleGraph`.
- The existing validator remains the trust boundary.
- Diagnostics use stable codes and exact source spans.
- CLI results are canonical JSON for direct agent consumption.

The first native compiler slice supports f32 scalars and vectors, indexed stream
access, `+`, `-`, `*`, `/`, and assignment. More complex kernels may use
`extern metal` as an explicit bootstrap escape hatch.

## Example

```pqo
kernel integrate(
  position: rw stream<f32x3,m>,
  velocity: rw stream<f32x3,m/s>,
  gravity: in value<f32x3,m/s^2>,
  dt: in value<f32,s>
) each i {
  velocity[i] += gravity * dt;
  position[i] += velocity[i] * dt;
}

pass fall = integrate(
  position=particles.position
  velocity=particles.velocity
  gravity=world.gravity
  dt=simulation.fixed_dt
) over particles.position

flow simulation rate=120hz {
  fall -> bounce
}
```

## Consequences

- Agents no longer declare ABI binding order separately.
- Effects and bindings remain explicit and locally inspectable.
- Native kernel signatures generate packed Metal buffer declarations and
  packaged Metal source deterministically.
- Existing Rust graph builders remain available for compiler bootstrapping but
  are no longer the only executable authoring path.
- The source subset cannot yet express contracts, capabilities, dynamic
  streams, branches, loops, atomics, threadgroup memory, or SIMD-group work.
- Generated source patches require a source-aware edit layer beyond the
  existing graph-only `RepairPlan`.
