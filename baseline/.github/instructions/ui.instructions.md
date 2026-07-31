---
name: "Pqo project UI"
description: "Rules for Vue controls and telemetry"
applyTo: "ui/**/*.ts,ui/**/*.vue,ui/**/*.css,ui/**/*.html,ui/**/*.json"
---

# UI rules

- Use Vue 3 Composition API with `<script setup lang="ts">` and strict types.
- Keep `bridge.ts` as the typed boundary for Tauri commands.
- Treat the UI as a control and telemetry projection, never as simulation
  authority.
- Keep control names synchronized with `src/runtime.rs` and the primary `.pqo`
  file.
- Preserve the existing Ant Design Vue patterns and visual language unless the
  task explicitly changes the design system.
- Handle bridge failures without fabricating connected state or telemetry.
- Clean up timers and listeners during component unmount.
- Prefer small derived `computed` values and explicit async error handling.
- Run `npm run build`; do not hand-edit generated files in `ui/dist`.
- Replace `PROJECT.pqo` with the primary filename and run
  `pqo build PROJECT.pqo` when packaged UI behavior changes.
