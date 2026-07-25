# Loom Language Design

The language documents are read in this order:

1. [`00-language-charter.md`](00-language-charter.md) — immutable principles and the intended agent experience.
2. [`01-semantic-model.md`](01-semantic-model.md) — primary nouns and composition patterns.
3. [`decisions/0001-language-shape.md`](decisions/0001-language-shape.md) — why semantics are locked before punctuation.
4. [`decisions/0002-agent-native-positioning.md`](decisions/0002-agent-native-positioning.md) — the agent-native trust boundary and current language classification.
5. [`../examples/hello-particle/hello-particle.loom`](../examples/hello-particle/hello-particle.loom) — first conformance specimen.
6. [`04-execution-scheduling.md`](04-execution-scheduling.md) — completion, overlap, observation, view, inspection, ABI, determinism, and overload rules.
7. [`06-canonical-representation.md`](06-canonical-representation.md) — untrusted graphs, validation, atomic repairs, execution plans, and artifact identity.

Planned specifications:

- `02-types-units-effects.md`
- `03-memory-model.md`
- `05-contracts-scenarios.md`
- `07-compiler-pipeline.md`
- `08-metal-backend.md`
- `09-hello-particle.md`

Each major language decision belongs in `decisions/` with its problem, chosen rule, rejected alternatives, and consequences.

## Typed graph milestone

The parser-independent implementation lives in:

- `crates/loom-core` — graph nodes, stable typed IDs, builder, canonical serialization, and Hello Particle fixture.
- `crates/loom-validator` — structural and semantic validation, structured diagnostics, atomic repair plans, ordering, lifetime analysis, validated execution plans, and artifact identity.

Run it with:

```text
cargo test --workspace
cargo run -p loom-validator --example hello_particle
```

The example intentionally constructs the unsafe one-buffer/four-overlapping-ticks variant. It proves that the invalid source receives no artifact identity, applies its two repairs atomically, revalidates, and then prints the validated artifact fingerprint.

## Native Metal Hello Particle

On macOS, launch the validated compute/render slice with:

```text
./scripts/run-hello-particle.sh
```

The runtime accepts a `ValidatedModuleGraph`, allocates private Metal buffers from
the execution plan, compiles its declared compute and render implementations,
executes `fall → bounce → viewport`, and enforces the plan's cross-tick completion
leases before single-buffer reuse.
