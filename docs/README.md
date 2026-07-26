# Loom Language Design

## Start with the visual handbook

New to Loom? Open [`handbook/index.html`](handbook/index.html) for
**Loom: Programming Systems That Build Themselves** — a visual, beginner-friendly
introduction to intelligent particles, fields, GPU computing, Hello Organism,
Hello Crystal, agent-native contracts, and Loom's current limits.

The language documents are read in this order:

1. [`agent-coding-reference.md`](agent-coding-reference.md) — literal executable syntax, safe experiment recipes, hard current limits, and the check/repair protocol for coding models.
2. [`00-language-charter.md`](00-language-charter.md) — immutable principles and the intended agent experience.
3. [`01-semantic-model.md`](01-semantic-model.md) — primary nouns and composition patterns.
4. [`decisions/0001-language-shape.md`](decisions/0001-language-shape.md) — why semantics are locked before punctuation.
5. [`decisions/0002-agent-native-positioning.md`](decisions/0002-agent-native-positioning.md) — the agent-native trust boundary and current language classification.
6. [`decisions/0004-agent-native-source.md`](decisions/0004-agent-native-source.md) — the compact executable source grammar and its agent-oriented design rules.
7. [`../examples/hello-particle/hello-particle.agent.loom`](../examples/hello-particle/hello-particle.agent.loom) — first executable agent-native source specimen.
8. [`native-compiler-gates.md`](native-compiler-gates.md) — measured roadmap from element-wise conditionals through native render stages.
9. [`04-execution-scheduling.md`](04-execution-scheduling.md) — completion, overlap, observation, view, inspection, ABI, determinism, and overload rules.
10. [`gpu-authoring-handbook.md`](gpu-authoring-handbook.md) — human- and agent-facing guidance for GPU-resident state, scalable passes, Metal ABI alignment, and performance proof.
11. [`06-canonical-representation.md`](06-canonical-representation.md) — untrusted graphs, validation, atomic repairs, execution plans, and artifact identity.
12. [`decisions/0003-emergent-systems-substrate.md`](decisions/0003-emergent-systems-substrate.md) — emergent-computation positioning and authority rules.
13. [`07-emergent-systems.md`](07-emergent-systems.md) — implemented dynamic populations, fields, and Hello Organism boundary.
14. [`benchmarks/hello-organism-population-gate-m4-pro.md`](benchmarks/hello-organism-population-gate-m4-pro.md) — scalable population correctness load and declared-capacity timing boundary.
15. [`benchmarks/hello-organism-neighborhood-gate-m4-pro.md`](benchmarks/hello-organism-neighborhood-gate-m4-pro.md) — developmental neighborhoods, morphology reductions, and their timing boundary.
16. [`benchmarks/hello-organism-development-gate-m4-pro.md`](benchmarks/hello-organism-development-gate-m4-pro.md) — one-seed procedural development, causal ablations, exact replay, and populated reference timing.
17. [`benchmarks/hello-organism-homeostasis-gate-m4-pro.md`](benchmarks/hello-organism-homeostasis-gate-m4-pro.md) — sustained equilibrium, energy accounting, nutrient perturbation, and the clean Gate 4 timing.
18. [`benchmarks/hello-organism-regeneration-gate-m4-pro.md`](benchmarks/hello-organism-regeneration-gate-m4-pro.md) — committed-checkpoint lesion, local repair, sustained recovery, and causal ablations.
19. [`visuals/hello-organism-gate-5.svg`](visuals/hello-organism-gate-5.svg) — visual evidence summary of development, lesion, local regeneration, and causal ablations.
20. [`milestones/hello-organism-v0.1.md`](milestones/hello-organism-v0.1.md) — frozen Gate 1–5 proof manifest, hashes, event record, timing, and limitations.
21. [`../examples/hello-crystal/README.md`](../examples/hello-crystal/README.md) — runnable 3D mesoscopic crystal growth, cleavage, component, fragment, metric, and surface specimen.
22. [`benchmarks/hello-crystal-1m-m4-pro.md`](benchmarks/hello-crystal-1m-m4-pro.md) — one-million-cell M4 Pro execution smoke proof and its explicit evidence boundary.

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

## Agent-native source

The first executable `.loom` compiler parses native kernel arithmetic, checks
its types, units, and effects, generates packaged Metal, and lowers canonical
agent source into the same typed graph used by the Rust builders.

Install the complete Apple Silicon macOS distribution:

```text
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/install.sh | sh
```

Everything required to use Loom lives below `~/.loom`: the compiler and Metal
runtime, examples, handbook, version, and installation manifest. The installer
adds only a `~/.local/bin/loom` symlink outside that directory. Rust and Cargo are
not required on the host.

Update to the newest release from any directory:

```text
loom update
```

Run a program directly:

```text
loom ~/.loom/examples/hello-particle.loom
loom ~/.loom/examples/crystal.loom
```

A source path means run. The explicit development and inspection commands remain
available:

```text
loom run examples/hello-particle/hello-particle.agent.loom
loom check examples/hello-particle/hello-particle.agent.loom
loom explain examples/hello-particle/hello-particle.agent.loom
```

`check` and `explain` return structured JSON. `check` reports stable graph and
artifact hashes; `explain` includes the normalized graph, generated Metal, and
resolved execution plan. On macOS, `run` opens a native Metal window driven
directly by the parsed source graph.

Remove the command, runtime, examples, documentation, and manifest together:

```text
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/uninstall.sh | sh
```

The remover only deletes a directory carrying Loom’s recognized installation
manifest and only removes a command symlink that points into that directory.

## Native Metal Hello Particle

On macOS, launch the validated compute/render slice with:

```text
./scripts/run-hello-particle.sh
```

The runtime accepts a `ValidatedModuleGraph`, allocates private Metal buffers from
the execution plan, compiles its declared compute and render implementations,
executes `fall → bounce → viewport`, and enforces the plan's cross-tick completion
leases before single-buffer reuse.

Select the conformance experiment through the same launcher:

```text
./scripts/run-hello-particle.sh particle
./scripts/run-hello-particle.sh batch
./scripts/run-hello-particle.sh batch 10k
./scripts/run-hello-particle.sh population 16k --bench headless --samples 300
./scripts/run-hello-particle.sh field --bench headless --samples 300
./scripts/run-hello-particle.sh organism 16384
./scripts/run-hello-particle.sh organism 16384 --bench headless --samples 300
./scripts/run-hello-particle.sh crystal 1m
./scripts/run-hello-particle.sh crystal 32768 --bench headless --samples 300
```

`batch` defaults to 1,000 particles. Counts accept exact positive integers plus
`k` and `m` suffixes. Hello Batch uses the same kernels and language path while
testing stream capacity, logical length, plan-driven dispatch, private-buffer
allocation, and instanced rendering at scale. Initial state uses typed compact
`repeat`, `linear`, and `grid_2d` generators. Their counts, element types, shapes,
and parameters are validated before the Metal backend expands them into the one-time
upload; the semantic graph no longer grows with particle count.

Run bounded benchmarks without opening a window:

```text
./scripts/run-hello-particle.sh batch 10k --bench headless
./scripts/run-hello-particle.sh batch 10k --bench rendered
./scripts/run-hello-particle.sh batch 100k --bench rendered --warmup 120 --samples 600
./scripts/run-hello-particle.sh batch 100k --bench rendered \
  --runner direct-metal --warmup 120 --samples 600
./scripts/run-hello-particle.sh batch 100k --bench headless \
  --warmup-seconds 30 --duration-seconds 60
./scripts/run-hello-particle.sh batch 1m --bench rendered \
  --pace 120 --pace-lead-us 2000 --warmup-seconds 5 --duration-seconds 10
./scripts/run-hello-particle.sh batch 1m --bench presented \
  --pace 120 --warmup-seconds 5 --duration-seconds 10
./scripts/benchmark-hello-batch.sh 30 60
./scripts/benchmark-hello-batch-clean.sh 100k rendered 30 60 4
```

Benchmark commands build and run the optimized Rust release profile automatically.
Headless measures compute only. Rendered adds the plan-driven view into a private
960×720 target. Results report Metal command-buffer GPU timing, CPU encoding and
queue-admission time, end-to-end tick latency, p50/p95/p99, throughput, the
8.33 ms gate, GPU buffer bytes, initialization blits, scoped steady-state
copy/blit counts, peak resident set, and the runtime fingerprint. Heap-allocation
counting remains explicitly `null` until allocator instrumentation is added.

Hello Batch uses a validator-proven serial queue with four bounded in-flight command
buffers. Metal completion handlers collect timestamps asynchronously, and the host
drains results only after each warm-up or sampling phase. Results report this as
`synchronized_each_tick: false`.

`--runner loom` is the default plan-driven encoder. `--runner direct-metal` uses the
same initialized buffers, MSL, pipeline states, threadgroup sizing, dispatch, command
buffer grouping, and render target while replacing plan traversal and typed binding
lookup with fixed Metal encoding. This isolates steady-state orchestration overhead;
it is not an independently implemented resource loader.

Tick counts remain convenient for short regression runs. `--warmup-seconds` and
`--duration-seconds` select sustained wall-time phases; the result records both the
requested duration and the number of ticks actually executed. `--pace` admits a
fixed number of ticks per second and reports deadline misses. `--pace-lead-us`
explicitly admits work slightly ahead of its deadline; the lead must remain shorter
than one tick and is recorded in the result.

Rendered mode remains offscreen. Presented mode creates an attached `CAMetalLayer`
and uses `CAMetalDisplayLink` to admit render frames independently from the fixed
120 Hz simulation clock. It reports GPU deadline misses, actual presentation misses,
skipped presentations, and drawable starvation separately. Actual presentation time
comes from the drawable presented handler rather than command-buffer completion.
The sweep script runs 1K, 10K, 100K, and 1M in both modes and writes one JSON result
per workload under `benchmark-results/hello-batch` by default.

The clean comparison script refuses a dirty source tree and alternates Loom-first
and direct-Metal-first trials to reduce ordering bias. Its defaults are four
interleaved pairs with a 30-second warm-up and 60-second sample.

Recorded baselines:

- [`benchmarks/hello-batch-100k-m4-pro.md`](benchmarks/hello-batch-100k-m4-pro.md)
- [`benchmarks/hello-batch-100k-async-m4-pro.md`](benchmarks/hello-batch-100k-async-m4-pro.md)
- [`benchmarks/hello-batch-compact-paced-m4-pro.md`](benchmarks/hello-batch-compact-paced-m4-pro.md)
