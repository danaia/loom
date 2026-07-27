# Loom project instructions

This workspace is a complete Loom 0.1 GPU application, not a generic Rust or
Vue project. Find the primary top-level `.loom` file and treat it as the
authoritative state, effect, binding, and execution-order graph. The Baseline
template calls it `baseline.loom`; `loom new` renames it to the project name.

- Read `skills/engineer-loom-systems/SKILL.md` before changing behavior.
- State the intended behavior and affected rule before editing.
- Use only executable Loom 0.1 syntax; do not copy future syntax from roadmap
  documents.
- Keep state in typed streams and keep every physical unit, access mode, binding,
  dispatch domain, and pass dependency explicit.
- Use native Loom only for supported same-index f32 arithmetic. Declare advanced
  behavior as `extern metal`.
- Keep each file under `kernels/` mechanically aligned with its Loom kernel
  parameter order; parameter order is `[[buffer(n)]]` order.
- Keep each file under `shaders/` aligned with its view read order and render
  stream types.
- Treat control names shared by `ui`, `src/runtime.rs`, and the primary `.loom`
  file as a cross-layer API. Search all occurrences before renaming.
- Keep host input as explicit Loom value overrides. Do not move authoritative
  simulation state to Rust or Vue.
- Change one behavior class at a time. Preserve the working baseline structure
  unless the requested behavior requires a deliberate architectural change.
- Replace `PROJECT.loom` with the primary filename. After structural edits run
  `loom check PROJECT.loom`, then inspect `loom explain PROJECT.loom`.
- Run the application before claiming visual or interactive success.
- After UI or project-extension changes run `loom build PROJECT.loom` and test
  the packaged result. Run `npm run build` in `ui` after UI source changes.
- Never claim performance without a release build, fixed workload, warm-up,
  repeated samples, and device details.

Follow the file-scoped rules in `.github/instructions/` for Loom, Metal, Rust,
and UI files.
