# Decision 0002 — Define Loom as Agent-Native Physical Compute

- **Status:** Accepted
- **Date:** 2026-07-25

## Problem

Calling Loom “agent-based” can imply that an AI model participates in program
execution. Calling it only an agent-authored language misses the deterministic
structures that let agents inspect, repair, compare, benchmark, and prove programs.

Calling Loom a complete low-level programming language is also premature while
kernel arithmetic remains in external Metal implementations.

## Decision

The canonical description is:

> Loom is becoming an agent-native, low-level physical-compute language.

For v0, the precise classification is:

> Loom is an early agent-native systems DSL with a low-level typed execution model.

Agents author and modify Loom. Deterministic compilers, validators, verifiers, and
runtimes decide what executes. An AI model is never part of the trusted compilation
path or simulation loop.

Loom is agent-native because it provides:

- explicit state, effects, units, bindings, dependencies, and capabilities,
- a canonical typed graph with stable identity,
- deterministic serialization and fingerprints,
- structured diagnostics and atomic graph repairs,
- and executable contracts, scenarios, and benchmarks.

Loom is low-level because streams expose mutable state and memory semantics; kernels
declare computation and effects; passes bind resources and dispatch; schedules
declare ordering and overlap; views project state; and capabilities delimit external
authority.

## Current Boundary

Loom v0 controls memory, effects, scheduling, validation, and orchestration. Kernel
bodies are backend implementations:

```loom
implementation metal {
  source "kernels/euler_integrate.metal"
  entry "integrate_main"
}
```

A future native kernel language may lower arithmetic to Metal, SPIR-V, CUDA, CPU
SIMD, or other backends. That feature is explicitly outside the v0 freeze.

## Rejected Alternatives

### Describe Loom as agent-based

Rejected because it incorrectly suggests runtime AI participation and weakens the
deterministic trust boundary.

### Define agent-native as AI-friendly syntax

Rejected because surface syntax alone does not make generation safe, repairs
mechanical, or claims provable.

### Claim a complete low-level language in v0

Rejected because Loom does not yet express kernel arithmetic independently of a
backend language.

## Consequences

- Product and architecture documents use the canonical description consistently.
- The trusted execution path remains fully deterministic.
- Agent-facing features are judged by inspectability, repairability, and proof—not
  by textual novelty.
- A native kernel language requires its own later language decision and version
  boundary.
