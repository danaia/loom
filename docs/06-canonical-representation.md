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

- canonical node IDs and duplicate names,
- every value, stream, kernel, slot, pass, view, schedule, contract, scenario, benchmark, and capability reference,
- ABI slot references,
- resource bindings and dispatch references,
- dependency endpoints,
- observation and predicate references.

If structural validation fails, no semantic pass runs.

## Validated graphs

`ValidatedModuleGraph` has private fields and can only be created by the validator after every required pass succeeds. It owns:

- the normalized graph,
- its resolved `ExecutionPlan`,
- and an executable artifact fingerprint.

Backend lowering accepts `ValidatedModuleGraph`, not `ModuleGraph`.

## Two hashes

The normalized source-graph hash identifies any canonical graph, including an invalid one. It is useful for diagnostics, caching, and repair preconditions.

The executable artifact fingerprint exists only on `ValidatedModuleGraph`. It covers:

- validator schema version,
- normalized graph,
- effective simulation and presentation concurrency,
- and topologically resolved execution plans.

An invalid graph never receives an artifact fingerprint.

## Atomic repair plans

A `RepairPlan` contains:

- the exact source-graph hash it applies to,
- a canonical ordered edit set,
- and expected old values for mutations.

Application clones the source, verifies the hash and every old value, applies all edits, and reruns the complete validator. The caller receives a `ValidatedModuleGraph` only if the entire plan succeeds.

Stale, partial, conflicting, or nonvalidating repairs return an error without changing the original graph.
