# Pqo project instructions

This workspace is a complete Pqo 0.1 GPU application, not a generic Rust or
Vue project. Find the primary top-level `.pqo` file and treat it as the
authoritative graph for state, effects, bindings, and execution order. The
Baseline template calls it `baseline.pqo`; `pqo new` renames it to the
project name.

Before changing behavior, read
[`skills/engineer-pqo-systems/SKILL.md`](skills/engineer-pqo-systems/SKILL.md).
It contains the architecture references, change playbook, inspection helper,
and the required evidence loop. The parser and validator are the final
authority for executable Pqo syntax and semantics.

## General rules

- State the intended behavior and affected rule before editing.
- Use only executable Pqo 0.1 syntax; do not copy future syntax from roadmap
  documents.
- Change one behavior class at a time. Preserve the working Baseline structure
  unless the task requires a deliberate architectural change.
- Keep state in typed streams. Make every physical unit, access mode, binding,
  dispatch domain, and pass dependency explicit.
- Use native Pqo only for supported same-index f32 arithmetic. Declare
  branches, neighbors, atomics, reductions, scans, textures, complex math, and
  rendering as `extern metal`.
- Treat control names shared by `ui`, `src/runtime.rs`, and the primary `.pqo`
  file as a cross-layer API. Search all occurrences before renaming them.
- Keep host input as explicit Pqo value overrides. Do not move authoritative
  simulation state to Rust or Vue.
- Replace `PROJECT.pqo` below with the primary Pqo filename. After structural
  edits, run `pqo check PROJECT.pqo` and inspect `pqo explain PROJECT.pqo`.
- Run the application before claiming visual or interactive success. Do not
  claim performance without a release build, fixed workload, warm-up, repeated
  samples, and device details.

## Pqo source (`*.pqo`)

- Begin with `pqo 0.1`, a snake_case module name, and `target metal`.
- Write all stream properties: `cap`, `len`, `buffers`, `access`, `storage`,
  and `init` when initial data is required. Keep `len <= cap` and update every
  aligned stream together.
- Treat units as types. Give kernels the narrowest correct access; use `all`
  only for intentional whole-resource algorithms.
- Bind every parameter exactly once and keep the dispatch stream length
  compatible with every per-invocation stream.
- Order all conflicting passes; draw only after the pass that completes every
  view input.

## Metal (`*.metal`)

- Implement only resources and effects declared by the corresponding Pqo
  kernel or view. Match Pqo parameter order exactly to `[[buffer(n)]]`; never
  insert an ambient buffer.
- Map read streams to `const device`, writable streams to `device`, and values
  to `constant &`. Bind Pqo `f32x3` streams and values as `packed_float3`.
- Use `uint index [[thread_position_in_grid]]` for compute dispatch. Access
  only `index` for per-invocation slots; use neighbors/global state only when
  the Pqo parameter declares `all`.
- Keep capacity, overflow, boundary, and synchronization assumptions visible.
  Keep simulation and render projection on the GPU.
- Preserve entry names referenced by the primary `.pqo` file, or update both
  sides in the same change. Verify ABI with `pqo explain PROJECT.pqo` and
  compile/run on real Metal.

## Project runtime (`src/**/*.rs`)

- Preserve the versioned C ABI: `#[repr(C)]`, field order, exported symbols,
  ABI version, pointer checks, and fixed capacity bounds.
- Translate host events and UI controls into explicit named f32 value overrides.
  Do not mutate GPU buffers or duplicate authoritative simulation state in Rust.
- Clamp untrusted input at the project boundary. Keep control and telemetry
  names identical across Rust, UI, and the primary `.pqo` file.
- Use standard Rust formatting and focused state methods; avoid dependencies
  when the standard library suffices. Run `pqo build PROJECT.pqo` and
  exercise each changed input path in the packaged application.

## UI (`ui/**`)

- Use Vue 3 Composition API with `<script setup lang="ts">` and strict types.
  Keep `bridge.ts` as the typed boundary for Tauri commands.
- The UI is a control and telemetry projection, never simulation authority.
  Preserve the existing Ant Design Vue patterns and visual language unless the
  task explicitly changes the design system.
- Handle bridge failures without fabricating connected state or telemetry;
  clean up timers and listeners during component unmount.
- Prefer small derived `computed` values and explicit async error handling. Run
  `npm run build`; never hand-edit generated files in `ui/dist`. Run
  `pqo build PROJECT.pqo` when packaged UI behavior changes.

## Completion evidence

Report the question/change, files and behavior changed, controlled inputs,
native and external kernels, `pqo check` status, graph and artifact hashes,
`pqo explain` findings, focused tests, run/build result, observed result, and
limitations. Compilation proves legality, not visual correctness, scientific
validity, or performance.
