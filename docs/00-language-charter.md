# Pqo Language Charter

Pqo is becoming an **agent-native, low-level physical-compute language**.

“Agent-native” describes its authoring and verification model, not its runtime. Agents
create and modify Pqo programs. Deterministic compilers, validators, verifiers, and
runtimes decide what is legal and what executes. No AI model lives inside a Pqo
program or participates in its simulation loop.

Today, Pqo is an early agent-native systems language with a low-level typed
execution model. It controls memory, effects, bindings, scheduling,
synchronization, validation, and orchestration. Its first native kernel-body
subset type-checks f32 scalar/vector arithmetic and generates Metal. Complex
kernel arithmetic may still use explicit external Metal while the native
language grows.

Its longer-term thesis is emergent computation: explicit stateful entities and
distributed fields cooperating through typed intents and authoritative
resolution. This does not make the particle a universal language primitive.
Streams, kernels, passes, schedules, contracts, and capabilities remain
fundamental; particles, cells, fields, organisms, and hierarchical aggregates
are schemas composed from them.

It should feel like arranging a precise experiment:

```text
declare the state
→ describe the transformations
→ connect the bindings
→ order the work
→ state what must remain true
→ run and inspect
```

The language is enjoyable when an agent can make a small change, understand its consequences locally, receive a useful explanation when it is wrong, and see the result quickly. “Fun” does not mean magical or implicit. In Pqo, clarity creates the freedom to experiment.

## Why Agent-Native

Pqo is native to agent workflows because an agent can deterministically:

- generate and inspect the canonical typed graph,
- compare versions using stable IDs and hashes,
- validate explicit state, units, access, bindings, order, and authority,
- apply structured `GraphEdit` repairs without scraping prose,
- and prove changes through contracts, scenarios, and benchmarks.

The innovation is not syntax that happens to look friendly to an AI model. It is a
machine-verifiable representation through which an agent can author, inspect,
repair, measure, and prove a program.

## Execution Boundary

```text
agent intention
→ Pqo typed semantic graph
→ validator and contracts
→ execution plan
→ Metal/native lowering
→ GPU execution
```

Pqo v0 is therefore both a typed compute/render graph and an executable physical
specification. Native Pqo arithmetic lowers to generated Metal behind
target-neutral kernel signatures; explicit Metal remains a bootstrap escape
hatch. This separation lets Pqo grow its kernel language without weakening the
validated execution policy.

## Constitution

These rules define Pqo. Changing one requires an explicit language decision record and a version boundary.

### 1. Agents author; deterministic systems decide

Agents may create programs, propose rewrites, choose implementations, and explore variants. The validator, compiler, verifier, and runtime determine whether a program is legal and whether its contracts pass.

No model call belongs in the trusted compilation path or simulation loop.

### 2. State is explicit

Persistent mutable state lives in named, typed streams. Pqo has no ambient property bags, hidden heap objects, or undeclared global mutation.

The particle is a useful domain entity represented by related streams. It is not the universal primitive for memory, execution, rendering, or identity.

### 3. Every transformation declares its reach

A kernel declares every value and stream it reads, writes, atomically accesses, renders, inspects, or passes to an external capability.

A kernel cannot reach a resource that is absent from its signature.

### 4. Invocation and implementation are separate

A `kernel` defines a computation and its effects. A `pass` invokes a kernel by binding its slots to concrete values and streams.

This separation makes kernels reusable and makes each invocation auditable.

### 5. Order is semantic

A `schedule` declares what runs and the dependencies between runs. It does not prescribe a Metal barrier, encoder boundary, fence, or command-buffer layout.

The backend chooses the mechanism. The validator proves that the chosen ordering satisfies every resource hazard.

### 6. Units are types

Physical units participate in type checking. Adding meters to seconds is illegal. Conversions are explicit and deterministic.

Units are not comments and cannot be erased before validation.

### 7. Memory has meaning

Capacity, layout, storage, buffering, mutability, residency, and lifetime are separate concepts. Pqo never overloads one word to mean another.

Logical capacity describes elements. Physical buffering describes concurrent versions.

### 8. Nothing expensive is hidden

Allocation, copying, readback, synchronization, dynamic dispatch, and external work must be visible in semantics or instrumentation.

Zero-work claims are scoped. Initialization, steady state, inspection, and shutdown are distinct phases.

### 9. Contracts are executable claims

A contract must be:

- statically verifiable by the compiler,
- dynamically measurable by the runtime,
- checked by a deterministic scenario,
- or explicitly identified as unsupported.

The compiler rejects unverifiable claims presented as guarantees.

### 10. Targets do not leak into physics

Apple Silicon and Metal are the first implementation target. Their capabilities shape lowering and performance contracts, not the meaning of position, velocity, time, collision, or dependency.

Backend-specific implementation blocks are allowed behind target-neutral signatures.

### 11. Reproducibility has an identity

Determinism claims include the device, OS, compilers, pipeline descriptors, artifacts, layouts, dispatch, schedule, and inputs they cover.

“Deterministic” without a declared tier and fingerprint is incomplete.

### 12. The graph is canonical

Pqo has one typed semantic graph.

- The builder API constructs it directly.
- `.pqo` is its canonical textual projection.
- `.pqob` is a validated, compiled artifact.

Parsing text is not required inside the compiler, optimizer, or runtime.

## Agent Experience

Pqo is designed for agents as first-class authors.

### One idea per construct

- `value` names immutable data.
- `stream` owns typed state.
- `kernel` defines computation.
- `pass` binds and invokes computation.
- `schedule` orders invocations.
- `contract` states required properties.
- `scenario` proves behavior from a known setup.
- `view` projects state without becoming simulation state.
- `capability` grants exceptional authority such as host mutation, inspection readback, or external integration.

An agent should not need to guess which construct owns a decision.

### Local reasoning

Reading a declaration and its direct references should reveal:

- its type and units,
- where its data comes from,
- who may mutate it,
- when it executes,
- and what validates it.

Resolution rules must not depend on distant declaration order or implicit imports.

### Progressive disclosure

The smallest valid world should be small. Layout overrides, buffering, backend variants, profiling, and optimization policy appear only when needed.

Defaults are allowed only when they are deterministic, inspectable, target-independent in meaning, and printed by `pqo explain`.

### Errors should teach

Every diagnostic has:

- a stable code,
- a primary source span,
- the violated rule in plain language,
- relevant declaration and binding spans,
- and at least one legal repair when a repair is unambiguous.

Diagnostics are available as canonical JSON so an agent can act without scraping prose.

### Fast, satisfying feedback

The core loop is:

```text
pqo check
pqo explain
pqo run
pqo inspect
pqo bench
pqo compare
```

The formatter is deterministic. A successful check reports the graph hash and a compact summary of state, work, dependencies, and contracts.

### Safe play

Experiments are named variants, not silent mutations of an accepted artifact. An agent can ask Pqo to compare variants against the same scenarios and contracts.

A variant is promoted only when it passes correctness and improves the declared objective reproducibly.

## What v0 Locks

Version 0 locks:

- the semantic nouns and their responsibilities,
- explicit state, transformation, binding, scheduling, proof, and projection patterns,
- identity and reference rules,
- the initial scalar, vector, struct, stream, and unit model,
- the initial effect and capability model,
- canonical graph ordering and hashing rules,
- and Hello Particle as the first conformance program.

Version 0 does not yet lock:

- every punctuation choice in the text projection,
- a general-purpose kernel-body language,
- user-defined macros,
- inference that can obscure a binding or effect,
- every future physical unit,
- or backend-specific optimization syntax.

## Design Test

A language feature belongs in Pqo v0 only if Hello Particle requires it or it is necessary to preserve one of the constitutional rules.

Everything else waits for a concrete expansion scenario.
