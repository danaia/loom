# Loom Core Semantic Model

This document defines the primary patterns agents use to construct Loom programs. It describes semantics, not final punctuation.

## The Composition Loop

Every Loom program follows the same loop:

```text
module
  ├── declare values and streams
  ├── define kernels
  ├── bind kernels into passes
  ├── connect passes in schedules
  ├── constrain behavior with contracts
  ├── prove behavior with scenarios
  └── expose state through views and capabilities
```

This repeated shape is intentional. Agents learn one way to build a one-particle experiment and reuse it at larger scales.

## Core Nouns

| Noun | Responsibility | Does not own |
| --- | --- | --- |
| `module` | Versioned program and namespace boundary | Runtime state |
| `value` | Immutable typed data or derived constant | Mutable storage |
| `stream` | Typed, indexed state with logical capacity | Kernel behavior |
| `kernel` | Reusable parallel computation and effect signature | Concrete resources |
| `pass` | Kernel invocation, bindings, and dispatch domain | Global ordering |
| `schedule` | Invocation order, dependencies, timing, and overload policy | Kernel implementation |
| `contract` | Required static or measured properties | Test initialization |
| `scenario` | Deterministic setup, actions, and expectations | Production scheduling policy |
| `view` | Render, inspector, or telemetry projection | Authoritative simulation state |
| `capability` | Exceptional host, inspection, or external authority | Ordinary declared dataflow |

Domain concepts such as spaces, boundaries, materials, and cameras are typed values or groups of values until a concrete use case proves that they need a new semantic noun.

## Pattern 1 — Declare State

A stream is one logical field over an indexed population:

```loom
stream particles.position: vec3<f32> unit m {
  capacity 1
  storage device_private
  access device_read_write
}
```

Related stream names may share a namespace, but this does not create an implicit object:

```text
particles.position
particles.velocity
particles.radius
```

Each stream declares:

- element type,
- physical unit when applicable,
- logical capacity,
- allowed access,
- optional initial data,
- and optional physical hints.

The initial layout model is structure-of-arrays by construction. Each stream is independently addressable. A later storage planner may pack compatible streams into one allocation without changing their semantic identity.

### Capacity and buffering

Capacity counts logical elements:

```loom
capacity 1024
```

Buffering counts physical versions used for safe overlap:

```loom
buffering 3
```

They are never interchangeable.

### Dynamic populations and write authority

A dynamic population shares a readable, mutable, one-element dimensionless
`u32` count stream. Mutable counts are streams because values remain immutable.
Dispatch and rendering over a count-backed stream lower to bounded indirect GPU
execution.

A stream may require a named capability for writes. Any pass binding a writable
slot to that stream must grant the capability explicitly. Membership capabilities
name the count and aligned streams whose active set may change.

Stream slots also declare indexing reach. `PerInvocation` means the invocation
accesses its corresponding element. `WholeResource` permits declared random or
aggregate access to a differently sized resource such as a field grid, counter,
or reduction. Reach is never inferred from a length mismatch.

### Values

Values are immutable and may be scalar, vector, matrix, struct, handle, or compile-time expression:

```loom
value world.gravity: vec3<f32> unit m/s^2 = [0.0, -9.81, 0.0]
value ground.height: f32 unit m = 0.0
```

Values do not imply global kernel access. They must still be bound into a pass.

## Pattern 2 — Define Transformation

A kernel declares typed slots and their effects:

```loom
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

The access mode is semantic:

- `read`
- `write`
- `read_write`
- `atomic`
- `render`
- `inspect`
- `external`

The implementation cannot legally access anything outside the declared slots and granted capabilities.

Version 0 may use external Metal implementations. A portable Loom kernel-body language is a separate decision and is not required to lock the primary composition patterns.

## Pattern 3 — Bind an Invocation

A pass supplies every kernel slot and chooses a dispatch domain:

```loom
pass fall uses integrate {
  bind position = particles.position
  bind velocity = particles.velocity
  bind gravity = world.gravity
  bind dt = simulation.fixed_dt

  dispatch over particles.position
}
```

Bindings are named and total:

- every required slot has exactly one binding,
- a binding must match type, unit, access, and capacity rules,
- an undeclared binding is an error,
- and a kernel cannot fall back to a global name.

`dispatch over stream_name` means one logical invocation per active element of that stream. Backends derive legal physical grid and threadgroup sizes from the compiled pipeline and target.

## Pattern 4 — Connect a Schedule

A schedule composes passes and views:

```loom
schedule simulation fixed 120 Hz {
  run fall
  run bounce after fall
  show viewport after bounce
}
```

`after` creates a semantic dependency edge. The validator combines these edges with pass effects to prove that:

- every read observes a defined write,
- conflicting accesses are ordered,
- no dependency cycle exists,
- render and inspection see a permitted version,
- and the backend can realize the schedule.

The Metal backend may realize an edge with command order, an encoder boundary, a memory barrier, a fence, or an event. That choice is not part of target-neutral Loom semantics.

A fixed schedule publishes immutable, typed schedule values such as `simulation.fixed_dt`. These values are part of the graph, appear in `loom explain`, and still require an explicit pass binding. They are not ambient kernel globals.

### Timing and overload

Timing policy belongs to the schedule:

```loom
schedule simulation fixed 120 Hz {
  catch_up at_most 4 ticks
  tick_overlap serialize_conflicting_ticks
  presentation_lifetime block_next_tick_until_views_complete
  overload {
    preserve fixed_dt
    drop render
    discard excess_wall_time
  }
}
```

The policy states exactly which truth is preserved under load. Loom never silently stretches a fixed timestep.

## Pattern 5 — State a Contract

Contracts name claims and their scope:

```loom
contract realtime on simulation {
  steady_state after initialization {
    heap_allocations_per_tick == 0
    application_copies_per_tick == 0
    application_blits_per_tick == 0
  }

  gpu_time_per_tick <= 8.33 ms
}
```

Every clause is classified as:

- `static` — compiler-verifiable,
- `measured` — runtime-instrumented,
- `scenario` — checked over deterministic execution,
- or `unsupported` — rejected if required.

Initialization and explicitly requested inspection are not silently included in steady-state claims.

## Pattern 6 — Prove with a Scenario

A scenario defines reproducible evidence:

```loom
scenario drop_and_bounce {
  reset to initial
  run simulation for 5 s

  expect always for_each i over particles.position {
    particles.position[i].y - particles.radius[i]
      >= ground.height - 0.0001 m
  }
  expect finite particles.position particles.velocity
}
```

A scenario owns:

- initial overrides,
- ordered inputs,
- run duration or tick count,
- observations,
- tolerances,
- and expectations.

`for_each i over stream` is a bounded quantifier over that stream's active indices. Every stream indexed by `i` must have a validator-proven compatible domain.

Scenario comparisons have defined operators and typed operands. Natural-language expectations are documentation only and cannot satisfy a contract.

## Pattern 7 — Project a View

A view reads authoritative state without becoming that state:

```loom
view viewport render {
  read position = particles.position
  read radius = particles.radius
  read color = particles.color

  implementation metal {
    vertex "shaders/particle_vertex.metal" entry "particle_vertex"
    fragment "shaders/particle_fragment.metal" entry "particle_fragment"
  }
}
```

Render, inspector, telemetry, and debug projections share the same rule: they declare what they observe and when their observation is valid.

## Pattern 8 — Grant a Capability

Operations that cross the ordinary declared dataflow require a named capability:

```loom
capability inspect_particle_state {
  allow inspect particles.position particles.velocity
  delivery asynchronous
}
```

Capabilities are narrow, auditable, and bindable. Ordinary kernel access is authorized by typed slots, pass bindings, and stream access; ordinary rendering is authorized by a view's declared reads. Capabilities cover exceptional operations such as copying private state to a host inspector, accepting host mutation, or invoking external work. Possessing an inspector view does not grant mutation. A target or host integration cannot invent authority not present in the graph.

Initial capability kinds are:

- `inspect`
- `host_mutate`
- `external`

## Identity and References

Every declaration has a stable semantic ID derived from:

- module identity,
- declaration kind,
- canonical qualified name,
- schema version,
- and semantic content where content-addressing applies.

References use qualified names in source and resolved IDs in the graph. Declaration order does not change identity.

Renaming is a semantic change unless an explicit migration preserves identity.

## Canonical Graph Rules

The semantic graph has:

- versioned node schemas,
- typed edges,
- canonical field ordering,
- canonical declaration ordering,
- deterministic numeric encoding,
- content hashes,
- provenance records,
- and explicit unknown-field behavior.

Unknown required fields are errors. Unknown optional fields may be retained only when the schema declares round-trip preservation safe.

`.loom` printing is deterministic. `.loomb` serialization is deterministic for identical hermetic inputs and compiler identity.

## Version 0 Type Surface

### Scalars

- `bool`
- `i32`
- `u32`
- `f16`
- `f32`

### Aggregates and handles

- fixed vectors of 2, 3, or 4 scalar lanes,
- fixed matrices of 2, 3, or 4 dimensions,
- deterministic-layout structs,
- stream handles,
- texture handles,
- view handles.

### Units

- length,
- time,
- mass,
- velocity,
- acceleration,
- frequency,
- and dimensionless values.

Derived units are normalized before comparison. Conversions must name their scale.

### Rejections

Version 0 rejects:

- undeclared mutation,
- missing or duplicate bindings,
- incompatible units,
- ambiguous struct layout,
- dependency cycles,
- unordered data hazards,
- illegal concurrent CPU/GPU writes,
- unsupported target features,
- capability escalation,
- and contracts the selected build cannot verify or measure.

## Primary Agent Rhythm

An agent should be able to approach any Loom task with the same questions:

1. What state and immutable values exist?
2. What transformations are legal?
3. How are concrete resources bound?
4. In what order does work occur?
5. What must always or measurably remain true?
6. Which scenario proves it?
7. Which view makes the result observable?

If a proposed feature cannot answer those questions clearly, its semantic shape is not ready.
