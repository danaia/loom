---
name: "Loom Metal ABI"
description: "Rules for external Loom compute and render Metal"
applyTo: "**/*.metal"
---

# Metal rules

- Implement only resources and effects declared by the corresponding Loom kernel
  or view.
- Match Loom parameter order exactly to `[[buffer(n)]]`; never insert an ambient
  buffer.
- Map read streams to `const device`, writable streams to `device`, and values to
  `constant &`.
- Bind Loom `f32x3` streams and constant values as `packed_float3`; the runtime
  encodes both as 12 packed bytes. Convert to `float3` for arithmetic.
- Use `uint index [[thread_position_in_grid]]` for compute dispatch.
- Access only `index` for per-invocation slots. Reach neighbors or global state
  only when the Loom parameter declares `all`.
- Make capacity, overflow, boundary, and synchronization assumptions visible.
- Keep simulation and render projection on the GPU.
- Preserve entry names referenced by the primary `.loom` file, or update both
  sides in the same change.
- Replace `PROJECT.loom` with the primary filename, verify the ABI with
  `loom explain PROJECT.loom`, and compile/run on real Metal.
