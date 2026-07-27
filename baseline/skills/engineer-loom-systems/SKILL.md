---
name: engineer-loom-systems
description: Build, modify, debug, review, optimize, or extend Loom 0.1 GPU systems and the Loom compiler/runtime. Use for .loom graphs, external .metal kernels and shaders, Loom project runtime.rs extensions, project UIs, .lmp packaging, compiler crates, validators, execution plans, agent-authored experiments, and evidence-backed GPU performance work.
---

# Engineer Loom Systems

Treat Loom as a rule graph that controls how GPU state may evolve. Let agents
propose changes; let the parser, validator, execution plan, Metal compiler, and
runtime decide what is legal and what actually happened.

## Start with the task boundary

1. Find the primary `.loom` file and treat its directory as the project root.
2. State one testable question or one concrete behavior change.
3. Classify the work before editing:
   - **Loom application:** state, types, units, kernels, bindings, flow, or view.
   - **External Metal:** behavior beyond native Loom's current arithmetic subset.
   - **Project integration:** input, value overrides, telemetry, Vue UI, or package.
   - **Language evolution:** parser, graph, validator, lowering, runtime, or CLI.
4. Read only the matching reference:
   - Application syntax or diagnostics: [executable-language.md](references/executable-language.md)
   - Architecture or change ownership: [architecture.md](references/architecture.md)
   - Implementation, team, and proof route: [change-playbook.md](references/change-playbook.md)
5. Start from the nearest checked-in working example. Preserve its known ABI and
   execution structure until the new behavior requires a deliberate change.

For repository work, read `docs/agent-coding-reference.md` completely before
authoring executable Loom. Treat the current parser and validator as the final
authority, then `loom check`, runnable examples, documentation, and roadmap
concepts in that order.

## Use the capability ladder

Choose the simplest layer that fully expresses the required behavior:

1. **Constants, initializers, bindings, and flow:** use `.loom`.
2. **Element-wise f32 arithmetic:** use a native Loom `each` kernel.
3. **Branches, neighbors, atomics, reductions, scans, textures, complex math, or
   rendering:** declare the effects in Loom and implement `extern metal`.
4. **Host input and project-local controls:** use `src/runtime.rs` through the
   versioned project ABI; expose values as explicit Loom constants.
5. **Project UI:** use `ui/` only as a control and telemetry projection.
6. **A genuinely new language ability:** evolve grammar, canonical semantics,
   validation, lowering, runtime, tests, and docs as one narrow vertical slice.

Do not hide an unsupported operation, simulate GPU work on the CPU, or invent
future Loom syntax. Explicit `extern metal` is a valid and innovative boundary.

## Preserve Loom's constitutional invariants

- Keep persistent state in named, typed streams.
- Declare every kernel resource, access mode, indexing reach, and physical unit.
- Bind every kernel parameter exactly once in each pass.
- Order every conflicting producer and consumer in the flow.
- Keep capacity, logical length, buffering, storage, and authority distinct.
- Keep host intervention and rendering separate from authoritative simulation
  state.
- Preserve the exact Loom-to-Metal buffer ABI and stream element layout.
- Keep target-neutral semantics in the graph; keep Metal mechanisms in the
  backend.
- Prefer deterministic canonical data and stable diagnostics for agent repair.
- Never weaken types, units, access, `all`, or ordering merely to silence an
  error.

## Work in an evidence loop

### 1. Discover

- Inspect the primary `.loom` file, all referenced Metal sources, the project
  extension, and UI control names.
- Search for the closest behavior and tests before creating a new pattern.
- Record the current `loom check` hashes before a semantic change.

### 2. Frame

Write down:

```text
Question or requested behavior:
Independent change:
Controlled inputs:
Affected streams and units:
Pass order:
Native/external boundary:
Expected observation:
```

For performance work, also declare workload, device, build profile, warm-up,
sample count, and the metric that could falsify the hypothesis.

### 3. Change

- Change one behavior class at a time.
- Run `loom check` after structural edits.
- Run the nearest focused test after compiler or runtime edits.
- Keep external Metal and Loom signatures side by side while reviewing ABI.
- Preserve a working Metal implementation as a differential oracle when adding
  native language support.

### 4. Inspect

Run:

```sh
loom check path/to/project.loom
loom explain path/to/project.loom
```

Or use:

```sh
python3 path/to/engineer-loom-systems/scripts/inspect_loom.py \
  path/to/project.loom
```

Read JSON `status` before anything else. In `loom explain`, confirm the normalized
graph, kernel implementation class, binding order, dispatch, completion
dependencies, pass order, and presentation point.

### 5. Execute and prove

- Run the application before claiming it works.
- Observe the requested behavior rather than inferring it from compilation.
- Build the `.lmp` when package contents, UI, Metal, or `runtime.rs` changed.
- Use release builds and repeated representative samples for performance claims.
- Report limitations and untested hardware paths honestly.

## Report completion

Include:

```text
Question/change:
Files and behavior changed:
Controlled inputs:
Native Loom kernels:
External Metal kernels:
loom check status:
source_graph_hash:
artifact_fingerprint:
loom explain findings:
focused tests:
run/build result:
observed result:
limitations:
```

Compilation proves legality. A graph hash proves graph identity changed. Neither
proves visual correctness, scientific validity, or speed.
