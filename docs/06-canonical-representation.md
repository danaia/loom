# Loom Graph Trust Boundary

Loom treats every deserialized, parsed, or agent-edited `ModuleGraph` as untrusted.

```text
Untrusted ModuleGraph
→ structural and reference validation
→ semantic validation
→ ValidatedModuleGraph
→ ExecutionPlan
→ backend lowering
```

## Untrusted graphs

`ModuleGraph` remains public and serializable so builders, future parsers, bundles, and agents can construct it. Its ID accessors return `Option`; malformed IDs never index vectors directly.

The first validator pass checks:

- canonical node IDs, declaration order, and duplicate names,
- every value, stream, kernel, slot, pass, view, schedule, contract, scenario, benchmark, and capability reference,
- ABI slot references,
- resource bindings and dispatch references,
- dependency endpoints,
- observation and predicate references.

If structural validation fails, no semantic pass runs.

Canonical serialization normalizes semantically unordered collections such as pass
bindings, view reads, dependencies, capability stream sets, contract clauses, and
scenario expectations. Reordered declarations are rejected structurally because
dense typed IDs use declaration position. Equivalent valid graphs therefore produce
the same canonical bytes and hash.

## Validated graphs

`ValidatedModuleGraph` has private fields and can only be created by the validator after every required pass succeeds. It owns:

- the normalized graph,
- its resolved `ExecutionPlan`,
- and an executable artifact fingerprint.

Backend lowering accepts `ValidatedModuleGraph`, not `ModuleGraph`.

The resolved `ExecutionPlan` contains the backend-facing facts Metal lowering needs:

- complete per-stream resource-version allocations,
- ordered pass and view nodes,
- concrete resource bindings and dispatch domains,
- kernel ABI and backend implementation identity,
- read/write access records,
- same-tick and before-next-tick completion requirements that must become ordering,
  barriers, events, fences, or admission waits.

## Two hashes

The normalized source-graph hash identifies any canonical graph, including an invalid one. It is useful for diagnostics, caching, and repair preconditions.

The executable artifact fingerprint exists only on `ValidatedModuleGraph`. It covers:

- validator schema version,
- normalized graph,
- effective simulation and presentation concurrency,
- complete resource-version allocations,
- resolved bindings, dispatch, ABI, accesses, and completion requirements,
- and topologically resolved execution plans.

An invalid graph never receives an artifact fingerprint.

## Atomic repair plans

A `RepairPlan` contains:

- the exact source-graph hash it applies to,
- a canonical ordered edit set,
- and expected old values for mutations.

Application clones the source, verifies the hash and every old value, applies all edits, and reruns the complete validator. The caller receives a `ValidatedModuleGraph` only if the entire plan succeeds.

Stale, partial, conflicting, or nonvalidating repairs return an error without changing the original graph.
