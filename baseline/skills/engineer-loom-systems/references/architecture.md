# Loom architecture and change ownership

## Contents

- [The system in one graph](#the-system-in-one-graph)
- [Trust and evolution model](#trust-and-evolution-model)
- [Compiler and runtime anatomy](#compiler-and-runtime-anatomy)
- [Repository crate map](#repository-crate-map)
- [Project anatomy](#project-anatomy)
- [Change impact map](#change-impact-map)
- [Architectural review questions](#architectural-review-questions)

## The system in one graph

```text
agent intent
  ↓
.loom 0.1 source
  ↓ lex → parse → native expression checking/lowering
canonical ModuleGraph
  ↓ independent deterministic validation passes
ValidatedModuleGraph + ExecutionPlan + artifact identity
  ↓
Metal pipeline construction + plan-driven resource allocation/encoding
  ↓
Apple GPU execution → optional render view → project UI telemetry
```

A Loom application is not a script that sends ad hoc GPU commands. It is a
declarative, typed rule graph:

```text
immutable values + persistent streams
→ reusable kernels with exact effects
→ passes with total bindings and dispatch domains
→ an ordered fixed-rate flow
→ an optional external Metal view
```

This is the key evolutionary mechanism. An agent changes a rule, state schema,
binding, or dependency. Deterministic tooling rejects invalid evolution and
emits a new identity for valid evolution.

## Trust and evolution model

The trusted path contains no model call:

```text
agents propose
→ compiler normalizes
→ validator proves graph safety
→ runtime executes the validated plan
→ evidence decides promotion
```

The two primary identities have different meanings:

- `source_graph_hash`: canonical semantic graph identity.
- `artifact_fingerprint`: validated graph plus resolved execution-plan identity.

An invalid graph receives no valid artifact identity. A changed hash proves a
change, not correctness.

The source surface and the semantic graph are deliberately different in breadth:

- Executable `.loom 0.1` exposes the small, proven source subset.
- `loom-core::ModuleGraph` also represents richer contracts, scenarios,
  capabilities, dynamic lengths, scheduling policies, and builder fixtures.
- Roadmap documents describe future native syntax and must not be copied into a
  runnable `.loom` file before the parser implements it.

## Compiler and runtime anatomy

### 1. Source: `loom-syntax`

`crates/loom-syntax/src/lib.rs` owns:

- lexical tokens and source spans;
- the executable `.loom 0.1` parser;
- AST-to-`ModuleGraph` lowering;
- native f32 scalar/vector expression type and unit checking;
- native Metal source generation under `loom://generated/...`;
- structured `L`, `P`, `S`, `T`, and `M` diagnostics.

Keep parse recovery, spans, lowering, and generation deterministic. A new source
construct is incomplete until its invalid cases have stable diagnostics.

### 2. Semantics: `loom-core`

`crates/loom-core` is parser- and backend-independent:

- `model.rs`: canonical semantic nouns and enums.
- `ids.rs`: typed IDs preventing cross-kind confusion.
- `builder.rs`: direct graph construction and name/ID resolution.
- `canonical.rs`: stable ordering and canonical fingerprint input.
- conformance and domain fixtures: particle, emergent systems, organism,
  crystal, and worm graphs.

The graph owns meaning; Metal owns implementation. Do not encode target mechanics
as physical semantics in core nodes.

### 3. Proof and plan: `loom-validator`

`crates/loom-validator` runs ordered, independent passes over an untrusted graph:

1. structural references;
2. types, shapes, and units;
3. resource access;
4. capacity, length, and dispatch;
5. bindings and aliasing;
6. hazard construction;
7. schedule DAG validation;
8. buffer versions and in-flight lifetime;
9. backend ABI;
10. observation points;
11. determinism and overload;
12. validated artifact fingerprint.

Successful validation yields a `ValidatedModuleGraph` containing an immutable
graph, execution plan, and fingerprint. The runtime should consume this boundary,
not re-infer source intent.

### 4. Product boundary: `loom-cli`

`crates/loom-cli` owns user commands and stable JSON outcomes:

- a source/package path runs a program;
- `check` validates and reports summary plus hashes;
- `explain` prints normalized graph and execution plan;
- `build` creates a portable `.lmp`;
- `new` copies the Baseline starter;
- `update` manages an installed release.

Preserve machine-readable status values and exit behavior. Agents repair from
JSON, not terminal prose.

### 5. Execution: `loom-metal`

`crates/loom-metal` owns Apple GPU realization:

- validated resource allocation and initialization;
- Metal source and pipeline compilation;
- ABI-driven pass encoding and derived threadgroup sizing;
- fixed-rate scheduling, completion, reuse, and presentation lifetime;
- views, window input, scenarios, benchmarks, runtime fingerprints;
- optional project extension loading;
- panel and telemetry integration.

It may choose Metal mechanisms for a semantic dependency, but may not invent
authority or omit a completion requirement from the plan.

### 6. Window and panel shells

- `loom-windowing` owns shared viewer/panel/Agents window coordination.
- `loom-ui-panel` owns the generic Tauri host, authenticated local IPC, project
  asset serving, and agent/control bridge.

Application-specific controls stay in the project. Global shells must remain
generic.

## Repository crate map

```text
loom-core                 semantic center; no parser or Metal dependency
├── loom-syntax           text → graph and native kernel lowering
├── loom-validator        graph → validated graph + execution plan
├── loom-windowing        shared window layout primitives
├── loom-metal            validated plan → Metal execution
│   └── loom-windowing
├── loom-cli              command/package boundary
│   ├── loom-syntax
│   ├── loom-validator
│   ├── loom-windowing
│   └── loom-metal (macOS)
└── loom-ui-panel         generic project UI host
```

Avoid dependency cycles and avoid making `loom-core` aware of parser
punctuation, CLI output, UI concerns, or Metal objects.

## Project anatomy

The Baseline project is the smallest complete application architecture:

```text
baseline.loom             authoritative typed state/effect/order graph
kernels/*.metal           advanced compute behind declared Loom kernel ABIs
shaders/*.metal           render stages behind declared Loom view reads
src/runtime.rs            optional C-ABI input/control/value-override extension
ui/src                    Vue 3 controls and telemetry projection
ui/dist                   generated UI build included in packages
ui/loom-ui.json           panel manifest
config/window-layout.json optional viewer/panel/Agents layout policy
skills/                   agent procedures and references
.github/instructions/     VS Code file-scoped coding rules
.loom/build               generated local build output
*.lmp                     generated ZIP-compatible package
```

The primary `.loom` file defines the project root. External paths must be
relative, remain below that root, and not contain `..`.

Project ownership is intentionally split:

- The graph owns state, rules, effects, and order.
- Metal owns arithmetic the native subset cannot yet express.
- `runtime.rs` turns host events into explicit f32 value overrides.
- The UI requests named controls and displays named telemetry.
- The global runtime owns validation, scheduling, Metal, windowing, and host
  statistics.

### Cross-layer name contracts

Changing a control or override may require coordinated edits:

```text
UI setControl("interaction.name")
↔ runtime.rs set_control / write_frame
↔ const interaction.name in .loom
↔ pass binding to a kernel value slot
↔ Metal [[buffer(n)]] parameter
```

Treat this chain as an API. Search every occurrence before renaming.

## Change impact map

| Desired change | Primary owner | Required adjacent checks |
| --- | --- | --- |
| Tune a physical rule | `.loom` constant | unit, binding, run observation |
| Add persistent state | `.loom` stream | aligned lengths, access, initializer, memory |
| Add simple arithmetic | native Loom kernel | units, same-index ownership, generated Metal |
| Add branch/intrinsic/neighbor work | external compute `.metal` | Loom signature, ABI table, bounds |
| Add render data | `.loom` projection pass | shader view names, lengths, presentation order |
| Change appearance only | shader `.metal` | view buffer order and runtime render |
| Add mouse/control behavior | `src/runtime.rs` | Loom constants, UI control names, ABI version |
| Add panel control | `ui/src` | bridge call, runtime handler, built `ui/dist` |
| Add native syntax | `loom-syntax` | core meaning, diagnostics, validator, Metal snapshot |
| Add semantic authority | `loom-core` | builder, canonicalization, validator, plan, runtime |
| Add scheduling behavior | core + validator | plan, runtime enforcement, lifetime tests |
| Change package content | CLI package module | manifest/load symmetry and packaged run |

## Architectural review questions

- Can the behavior be understood from the resource, kernel, pass, and flow
  declarations without hidden state?
- Is each writable stream owned by the narrowest correct authority?
- Does each pass dispatch one invocation per natural work item?
- Are whole-resource access and aliasing justified by the algorithm?
- Do all consumers observe a completed, intended version?
- Does rendering project simulation state rather than becoming authoritative?
- Does host code emit explicit overrides instead of mutating GPU memory silently?
- Does a source feature lower into canonical meaning before backend code?
- Are invalid inputs rejected before artifact identity?
- Is every performance or determinism statement tied to recorded evidence?
