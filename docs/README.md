# Loom Language Design

The language documents are read in this order:

1. [`00-language-charter.md`](00-language-charter.md) — immutable principles and the intended agent experience.
2. [`01-semantic-model.md`](01-semantic-model.md) — primary nouns and composition patterns.
3. [`decisions/0001-language-shape.md`](decisions/0001-language-shape.md) — why semantics are locked before punctuation.
4. [`../examples/hello-particle/hello-particle.loom`](../examples/hello-particle/hello-particle.loom) — first conformance specimen.
5. [`04-execution-scheduling.md`](04-execution-scheduling.md) — completion, overlap, observation, view, inspection, ABI, determinism, and overload rules.

Planned specifications:

- `02-types-units-effects.md`
- `03-memory-model.md`
- `05-contracts-scenarios.md`
- `06-canonical-representation.md`
- `07-compiler-pipeline.md`
- `08-metal-backend.md`
- `09-hello-particle.md`

Each major language decision belongs in `decisions/` with its problem, chosen rule, rejected alternatives, and consequences.

## Typed graph milestone

The parser-independent implementation lives in:

- `crates/loom-core` — graph nodes, stable typed IDs, builder, canonical serialization, and Hello Particle fixture.
- `crates/loom-validator` — deterministic validation passes, structured diagnostics, graph edits, ordering, concurrency analysis, and fingerprint reports.

Run it with:

```text
cargo test --workspace
cargo run -p loom-validator --example hello_particle
```

The example intentionally constructs the unsafe one-buffer/four-overlapping-ticks variant so its mechanical repair diagnostics are visible.
