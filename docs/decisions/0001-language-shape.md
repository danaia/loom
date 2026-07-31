# Decision 0001 — Lock Semantics Before Surface Syntax

- **Status:** Accepted
- **Date:** 2026-07-25

## Problem

Pqo needs a language agents enjoy using, but a full parser and prematurely frozen syntax could consume the project before the execution model is tested.

At the same time, implementing Metal directly before agreeing on the agent's primary patterns would make the language a retrospective wrapper around runtime code.

## Decision

Pqo begins with:

1. a language charter,
2. a minimal canonical semantic graph,
3. the primary state/transformation/binding/schedule/contract/scenario/view patterns,
4. a typed builder API,
5. and Hello Particle as the first conformance program.

The first runtime milestone consumes that semantic graph and proves it end to end on Metal.

The `.pqo` block syntax in the Hello Particle specimen is a working projection, not yet a frozen grammar. Semantic meaning, identity, effects, and dependencies are locked before punctuation.

## Rejected Alternatives

### Build the complete text language first

Rejected because syntax, parser recovery, formatting, migrations, and complete diagnostics would delay evidence from the runtime.

### Build the Metal runtime before the language model

Rejected because the later language would inherit accidental runtime structure and the agent composition model would remain untested.

### Make particles the universal primitive

Rejected because particles do not cleanly model storage, kernels, render views, fields, or aggregated representations. Streams are the fundamental mutable-state primitive.

### Hide bindings and dependencies through inference

Rejected because local convenience would obscure authority, effects, synchronization, and reproducibility. Pqo may suggest missing declarations, but accepted programs contain explicit bindings and semantic dependency edges.

## Consequences

- Agents get a stable conceptual language immediately.
- The builder, text projection, optimizer, and runtime share one graph.
- Hello Particle can invalidate weak language patterns before the syntax is finalized.
- Parser work remains bounded to semantics proven by execution.
- Language evolution requires explicit decision records rather than incidental implementation drift.
