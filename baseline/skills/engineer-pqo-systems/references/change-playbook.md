# Pqo change and team playbook

## Contents

- [Fast orientation](#fast-orientation)
- [Route A: application behavior](#route-a-application-behavior)
- [Route B: external Metal](#route-b-external-metal)
- [Route C: project runtime and UI](#route-c-project-runtime-and-ui)
- [Route D: compiler or runtime evolution](#route-d-compiler-or-runtime-evolution)
- [Team-of-agents protocol](#team-of-agents-protocol)
- [Validation matrix](#validation-matrix)
- [Performance and innovation discipline](#performance-and-innovation-discipline)

## Fast orientation

Use this read order:

1. Primary `.pqo` graph.
2. Referenced kernel and shader sources.
3. `src/runtime.rs` only when host events or value overrides matter.
4. `ui/src/bridge.ts` and the affected component only when controls matter.
5. `pqo check`, then `pqo explain`.
6. Closest tests and deeper architecture sources only for the layer being changed.

Search before browsing broadly:

```sh
rg -n 'kernel_name|stream.name|interaction.name|entry_name' .
rg --files examples baseline crates docs | sort
```

## Route A: application behavior

1. State one falsifiable behavior.
2. Copy the closest working project or preserve the current one.
3. Record baseline `source_graph_hash` and `artifact_fingerprint`.
4. Change one constant, initializer, stream schema, kernel, or pass class.
5. Preserve units and aligned stream lengths.
6. Run `pqo check`.
7. Inspect implementation class and order with `pqo explain`.
8. Run for a fixed tick/interaction sequence and observe the intended output.
9. Compare against the control.

Use a named variant for experiments. Avoid silently changing a previously
accepted artifact when a control comparison matters.

## Route B: external Metal

Freeze the Pqo signature before implementing Metal. Create an ABI table:

| Buffer | Pqo parameter | Access/reach | Pqo type/unit | MSL type |
| --- | --- | --- | --- | --- |
| 0 | first parameter | e.g. `rw` | e.g. `f32x3<m>` | `device packed_float3*` |

Then:

1. Match every `[[buffer(n)]]` to Pqo parameter order.
2. Use `const device` for read-only streams and `constant &` for values.
3. Bind Pqo f32x3 streams and values as `packed_float3`; the runtime encodes
   both as 12 packed bytes. Convert to `float3` only for arithmetic.
4. Use the global linear `uint` dispatch index.
5. Rely on in-range `gid` only when validation proves compatible lengths.
6. Keep per-invocation access local unless the Pqo slot declares `all`.
7. Make algorithmic bounds and overflow policy visible.
8. Run check/explain, compile on real Metal, and compare output.

For native compiler work, retain the handwritten Metal version as a differential
oracle until generated code earns replacement.

## Route C: project runtime and UI

The project extension is a stable, versioned C ABI. Preserve:

- `#[repr(C)]` layout and field order;
- exported symbol names;
- `ABI_VERSION`;
- fixed name and override capacities;
- null-pointer and bounds checks;
- project-local state only.

Host code should translate input into explicit value overrides. It should not
reach into GPU buffers or duplicate simulation state.

For each control, verify the whole contract:

```text
Vue event
→ bridge setControl(name, value)
→ runtime set_control name match
→ runtime frame override
→ Pqo const with same qualified name
→ pass value binding
→ kernel/Metal parameter
```

For telemetry, trace the reverse path. Keep the Tauri shell generic and project
logic in the packaged UI. After UI edits run `npm run build`; after integration
edits run `pqo build` and test the packaged UI.

## Route D: compiler or runtime evolution

Deliver one vertical semantic slice:

```text
documented source meaning
→ AST/parser with spans
→ canonical graph meaning
→ validation and invalid diagnostics
→ execution plan impact
→ Metal lowering/runtime enforcement
→ valid + invalid fixtures
→ generated/reference proof
→ docs and benchmark where relevant
```

### Source-only syntax change

At minimum inspect:

- `crates/pqo-syntax/src/lib.rs`;
- semantic nodes in `pqo-core`;
- source tests and generated Metal snapshots;
- `docs/agent-coding-reference.md`.

Do not add parser-only meaning that disappears before the canonical graph.

### Graph or authority change

At minimum inspect:

- `pqo-core` model, builder, canonical ordering, and typed IDs;
- all validator passes affected by the new node or edge;
- execution-plan serialization and fingerprinting;
- runtime enforcement;
- repair/diagnostic behavior.

Untrusted graphs must not reach runtime execution.

### Scheduling or lifetime change

Prove:

- hazards and dependency order;
- live versions across overlapping ticks;
- presentation leases;
- dropped-presentation cleanup;
- queue assumptions;
- fixed-time overload behavior.

Do not equate command submission with GPU completion.

### Native compiler gate

Require:

1. defined syntax, type, unit, effect, and failure semantics;
2. deterministic canonical lowering;
3. validator coverage;
4. inspectable generated Metal provenance;
5. differential result against handwritten Metal;
6. real Apple GPU execution;
7. performance delta with fixed workload;
8. explicit external fallback.

## Team-of-agents protocol

Divide work by stable interfaces, not arbitrary file counts:

- **Graph owner:** `.pqo` resources, signatures, bindings, and flow.
- **Kernel owner:** external Metal implementation against a frozen ABI table.
- **Integration owner:** `runtime.rs`, UI control schema, package behavior.
- **Compiler owner:** grammar-to-graph-to-plan vertical slice.
- **Evidence owner:** read-only checks, focused tests, explain inspection, run and
  benchmark records.

One agent may fill several roles. Avoid having two agents edit the same ABI or
canonical model simultaneously.

### Handoff contract

Each handoff includes:

```text
Question/change:
Owned files:
Frozen names and ABI:
Types, units, access, reach:
Expected pass order:
Commands already run:
Hashes/results:
Known risks or unresolved evidence:
```

The receiving agent verifies the frozen interface instead of reconstructing it
from prose.

### Safe parallelism

Good parallel work:

- inspect graph and tests while another agent inventories UI controls;
- implement Metal after the graph owner freezes and publishes the ABI;
- build an independent evidence plan without editing production files;
- review docs and invalid fixtures alongside implementation.

Unsafe parallel work:

- simultaneous edits to kernel parameter order and Metal buffer indices;
- simultaneous renames across UI/runtime/Pqo without one owner;
- independent changes to canonical IDs or serialization;
- benchmarking a build that is still changing.

## Validation matrix

| Change | Minimum evidence |
| --- | --- |
| `.pqo` constants/initializers | check, explain, run observation |
| stream/schema/binding/flow | check, explain plan/order, run |
| external compute Metal | check, ABI review, real pipeline compile/run |
| shader/view | check, view ABI review, visual run |
| `runtime.rs` | build package, input/control exercise, run |
| Vue UI | `npm run build`, packaged panel exercise |
| parser/lowering | focused syntax tests, invalid spans, workspace tests |
| validator/plan | focused valid+invalid tests, fingerprints, workspace tests |
| Metal runtime | focused tests, real-Metal test, run representative example |
| package loader/builder | build, check package, run package |
| performance claim | release, warm-up, repeated fixed workload, device record |

Run `cargo fmt --all -- --check` and the closest focused Rust test for repository
changes. Run `cargo test --workspace` when the change crosses crate boundaries.
On non-macOS, report the missing real-Metal proof rather than claiming it.

## Performance and innovation discipline

Optimize in this order:

1. algorithmic work per element;
2. active dispatch size;
3. memory traffic and representation;
4. synchronization and pass boundaries;
5. occupancy/threadgroup specialization;
6. micro-operations.

Keep state GPU-resident and structure-of-arrays. Use count-backed dynamic
populations, spatially bounded neighborhoods, hierarchical scan/compaction,
staged reductions, and ping-pong fields. Render from GPU state.

Innovation is encouraged at every layer, but promotion requires:

```text
new rule or algorithm
+ preserved explicit authority
+ validator-visible safety
+ same or declared numeric semantics
+ observable causal result
+ measured cost
```

Do not call a serialized full-population loop, CPU readback loop, or hidden
global mutation an evolutionary GPU system.
