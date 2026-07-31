# Pqo Execution and Scheduling Semantics

This document closes the version 0 execution rules required by Hello Particle.

## Completion Dependencies

`after` always means completion:

```pqo
run bounce after fall
```

`bounce` may not observe or mutate resources until `fall` has completed all effects visible through their bindings. Submission order alone does not satisfy the edge.

The graph stores execution and presentation dependencies separately:

```text
ExecutionDependency: pass → pass
PresentationDependency: pass → view
```

A backend may lower completion to ordered commands, encoder boundaries, barriers, fences, events, or other proven mechanisms.

Presentation nodes are terminal. A simulation pass cannot depend on a view, so dropping presentation never removes an execution dependency.

## Capacity, Length, and Dispatch

Every stream has:

- `capacity` — maximum allocated logical elements,
- `length` — active logical elements,
- `buffering` — physical resource versions.

Length is either:

- a fixed integer no greater than capacity, or
- a readable, mutable, one-element dimensionless `u32` stream.

`dispatch over particles.position` dispatches over the active length, not capacity. Every stream indexed by that dispatch must have a validator-proven compatible logical domain.

For dynamic lengths, the Metal backend constructs bounded indirect dispatch and
draw arguments on the GPU. The declared stream capacity is the hard dispatch
ceiling even if a faulty kernel writes a larger count.

## Tick Overlap and Resource Reuse

`in_flight.simulation` describes submitted ticks that may remain incomplete. It does not by itself prove that mutable resources can be reused.

Every schedule selects one overlap policy:

### Require resource versions

```pqo
tick_overlap require_resource_versions
```

Each stream written by the schedule needs at least as many physical versions as overlapping simulation ticks. Insufficient buffering is an error with a `SetStreamBuffering` graph edit.

### Serialize conflicting ticks

```pqo
tick_overlap serialize_conflicting_ticks
```

Multiple ticks may be pending, but conflicting ticks execute one at a time. The validator reports effective simulation concurrency as one.

Hello Particle uses this conservative policy while its streams have `buffering 1`.

### Queue-ordered reuse

```pqo
tick_overlap queue_ordered_reuse
queue proof single_serial_queue_completion
```

Single-version reuse is legal only when the backend supplies a proof that conflicting simulation commands complete in a safe serial order. Declaring queue reuse without the proof is an error.

## Presentation Lifetime

Tick serialization does not prove that a previous render has finished reading a mutable stream. Every schedule therefore selects a separate presentation-lifetime policy.

### Require resource versions

```pqo
presentation_lifetime require_resource_versions
```

Every mutable stream read by a view needs enough physical versions for the declared
render frames in flight and the view's historical lag.

### Block until presentation completes

```pqo
presentation_lifetime block_next_tick_until_views_complete
```

The next conflicting simulation tick waits for the previous view's GPU reads to complete. Hello Particle uses this policy with `buffering 1`; its effective render concurrency is one even though two frames may be pending at the host boundary.

### Queue-ordered presentation reuse

```pqo
presentation_lifetime queue_ordered_reuse
queue proof single_serial_queue_completion
```

This is legal only when the backend proves that view reads complete before the next conflicting writes on one serial queue.

### Complete live ranges

Simulation overlap, presentation concurrency, and historical lag are not validated
as independent minima. The validator derives one live range per stream.

For resource-versioned presentation, simulation and presentation leases overlap:

```text
required versions
= simulation live versions
+ presentation/history live versions
- one shared produced version
```

For blocking or proven queue-ordered reuse, the two phases do not overlap and the
larger live range wins. For example, lag `1` with two render frames in flight needs
three versions—not two—even when simulation ticks are serialized.

## Cross-Tick Completion

The execution plan carries both:

- `WithinTick { before, after }` for pass and presentation DAG edges,
- `BeforeNextTick { after, streams }` for resource reuse across tick boundaries.

The latter is emitted for serialized conflicting simulation writes and for views
that must finish before a later tick overwrites their streams. A dropped view never
submits a resource lease, so `drop_presentation_only` releases that unsubmitted
lease immediately; any submitted view still satisfies its `BeforeNextTick`
requirement before reuse.

## Kernel ABI

Every kernel implementation declares:

- total slot binding order,
- dispatch-index representation,
- threadgroup behavior,
- aliasing rules,
- backend source,
- and entry point.

Version 0 uses a global linear `u32` dispatch index.

Threadgroup behavior is either:

- `backend_derived`, using compiled-pipeline limits, or
- a nonzero fixed `(x, y, z)` shape validated for the target.

Aliasing is forbidden unless the ABI names an allowed slot pair. Two writable slots cannot receive the same stream accidentally.

## Fixed Schedule Resources

A fixed schedule declares an immutable built-in resource:

```text
simulation.fixed_dt: value<f32, s>
```

It is a typed graph node with a stable `ValueId`. Kernels cannot access it implicitly; a pass must bind it to a slot.

## Contract and Scenario Observation

Every measured or behavioral clause names an observation point:

- `AfterPassCompletion(pass)`
- `AfterEveryPassCompletion(schedule)`
- `AfterTickExecution(schedule)`
- `AfterGpuCompletion(schedule)`

An observation must belong to the contract or scenario's schedule.

`AfterTickExecution` means all simulation passes for the tick have completed in schedule semantics. `AfterGpuCompletion` additionally means the backend completion signal for that tick has fired and host-visible measurements may be consumed.

## View State

Every view chooses one state relationship:

- `CurrentCompletedTick`
- `PreviousStableTick { lag >= 1 }`
- `Interpolated { older_lag > newer_lag }`

Hello Particle's viewport consumes `CurrentCompletedTick`. Its presentation dependency completes after `bounce`, so it never renders the partially integrated pre-contact state.

## Inspection Snapshots

An inspection capability names its streams, asynchronous delivery, and snapshot rule.

Hello Particle uses:

```text
NextGpuCompletedTickAfterRequest
```

A request returns an immutable snapshot labeled with the first tick whose GPU completion occurs after the request is accepted. It never means “whatever bytes happen to be available.”

Other explicit rules are `LatestGpuCompletedTickAtRequest` and `ExactCompletedTick`.

## Determinism Scope

Tier 1 requires `ExactExecutionFingerprint`, covering:

- source and compiled binary,
- exact device and GPU identity,
- OS and compiler identities,
- pipeline descriptors and hashes,
- layouts and buffering,
- dispatch dimensions,
- schedule and overload policy,
- and ordered inputs.

Tier 1 does not claim cross-GPU exactness. Cross-device execution requires a tolerance-based tier.

## Overload Clocks

With `discard excess_wall_time`:

- the simulation tick counter advances only for executed fixed ticks,
- simulation time is `executed_ticks × fixed_dt`,
- scenario duration advances in simulation ticks,
- discarded wall-time decisions are recorded in replay input,
- and rendering may drop only terminal presentation work.

The simulation never invents ticks, skips dependency edges, or stretches `fixed_dt`. A replay consumes the recorded overload decisions rather than re-deriving them from host timing.
