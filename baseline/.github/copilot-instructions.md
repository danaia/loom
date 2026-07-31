# Pqo project instructions

This workspace is a complete Pqo 0.1 GPU application, not a generic Rust or
Vue project. Find the primary top-level `.pqo` file and treat it as the
authoritative state, effect, binding, and execution-order graph. The Baseline
template calls it `baseline.pqo`; `pqo new` renames it to the project name.

- Read `skills/engineer-pqo-systems/SKILL.md` before changing behavior.
- State the intended behavior and affected rule before editing.
- Use only executable Pqo 0.1 syntax; do not copy future syntax from roadmap
  documents.
- Keep state in typed streams and keep every physical unit, access mode, binding,
  dispatch domain, and pass dependency explicit.
- Use native Pqo only for supported same-index f32 arithmetic. Declare advanced
  behavior as `extern metal`.
- Keep each file under `kernels/` mechanically aligned with its Pqo kernel
  parameter order; parameter order is `[[buffer(n)]]` order.
- Keep each file under `shaders/` aligned with its view read order and render
  stream types.
- Treat control names shared by `ui`, `src/runtime.rs`, and the primary `.pqo`
  file as a cross-layer API. Search all occurrences before renaming.
- Keep host input as explicit Pqo value overrides. Do not move authoritative
  simulation state to Rust or Vue.
- Change one behavior class at a time. Preserve the working baseline structure
  unless the requested behavior requires a deliberate architectural change.
- Replace `PROJECT.pqo` with the primary filename. After structural edits run
  `pqo check PROJECT.pqo`, then inspect `pqo explain PROJECT.pqo`.
- Run the application before claiming visual or interactive success.
- After UI or project-extension changes run `pqo build PROJECT.pqo` and test
  the packaged result. Run `npm run build` in `ui` after UI source changes.
- Never claim performance without a release build, fixed workload, warm-up,
  repeated samples, and device details.

Follow the file-scoped rules in `.github/instructions/` for Pqo, Metal, Rust,
and UI files.
