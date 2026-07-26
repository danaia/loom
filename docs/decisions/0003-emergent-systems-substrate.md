# Decision 0003 — Adopt the Emergent-Systems Substrate

- **Status:** Accepted
- **Date:** 2026-07-25

## Decision

Loom keeps its precise v0 description:

> Loom is an agent-native, low-level physical-compute systems DSL.

Its long-term thesis is:

> Loom is becoming a deterministic distributed-computation substrate where
> stateful entities and fields cooperate to produce emergent structure,
> behavior, adaptation, and visualization.

“Distributed” initially means local computation distributed across GPU elements.
It does not claim multi-machine execution.

Streams, kernels, passes, schedules, contracts, and capabilities remain the
language substrate. Particles, fields, cells, organisms, robots, and aggregates
are schemas built from those primitives rather than new language nouns.

## Runtime Rules

- Decision passes write typed perception, memory proposals, and intents.
- Protected committed state is writable only by passes holding its authority.
- Population count and aligned membership are mutated only through a membership
  capability.
- A dynamic population uses a mutable one-element `u32` stream as its active
  count; values remain immutable.
- Tick `T` reads committed state from its start. Deposits and daughters produced
  during `T` become behavior-visible at `T + 1`.
- Logical developmental decisions consume quantized inputs. Floating mechanics
  remain bounded numeric state.
- Field grids are ordinary streams. Their deposit, evolution, and commit passes
  are explicit and ordered.

## First Proofs

- `Hello Population` proves protected dynamic populations and GPU indirect
  dispatch.
- `Hello Field` proves packaged field kernels, reflective five-point diffusion,
  explicit deposit clearing, and committed next-tick state.
- `Hello Organism` couples both systems with quantized perception, typed intents,
  transition resolvers, energy accounting, canonical spatial bins, hierarchical
  prefix allocation, parallel compaction, indirect execution/rendering,
  disjoint homeostasis envelopes, and recorded environmental perturbations.

The backend-neutral reference rules remain the correctness oracle. The Metal
membership path now uses global stable-ID radix ordering, stable-ID bin
canonicalization, and prefix allocation. Developmental neighborhoods, morphology
reductions, causal field ablations, exact logical replay, and sustained
homeostasis are measured proofs. Regeneration and adaptive aggregation remain
follow-on gates.

## Consequences

- No LLM or hidden central organism controller enters the simulation loop.
- Runtime work and authority remain inspectable in the canonical graph.
- Visualization reads authoritative state rather than maintaining a parallel
  object model.
- Domain generality must be proven through materially different specimens, not
  asserted from the abstraction alone.
