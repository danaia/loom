---
name: "Pqo project runtime"
description: "Rules for the project-local Rust extension ABI"
applyTo: "src/**/*.rs"
---

# Project extension rules

- Keep the extension project-local and preserve the versioned C ABI.
- Preserve `#[repr(C)]`, struct field order, exported symbol names, ABI version,
  pointer checks, and fixed capacity bounds.
- Translate host events and UI controls into explicit named f32 value overrides.
- Do not mutate GPU buffers or duplicate authoritative simulation state in Rust.
- Clamp untrusted input at the project boundary.
- Keep control and telemetry names identical across Rust, UI, and
  the primary `.pqo` file.
- Use standard Rust formatting and focused state methods; avoid new dependencies
  when the standard library suffices.
- Replace `PROJECT.pqo` with the primary filename, run `rustc` through
  `pqo build PROJECT.pqo`, and exercise each changed input path in the packaged
  application.
