---
name: "Executable Pqo 0.1"
description: "Rules for typed Pqo graphs and native kernels"
applyTo: "**/*.pqo"
---

# Pqo source rules

- Begin with `pqo 0.1`, a snake_case module name, and `target metal`.
- Write all stream properties: `cap`, `len`, `buffers`, `access`, `storage`, and
  `init` when initial data is required.
- Keep `len <= cap` and update every aligned stream together.
- Treat units as types. Derive expression units before changing arithmetic.
- Give kernels the narrowest correct access. Use `all` only for intentional
  whole-resource algorithms.
- Bind every parameter exactly once and keep the dispatch stream length
  compatible with every per-invocation stream.
- Use native `each` bodies only for same-index f32 scalar/vector arithmetic with
  assignments and `+`, `-`, `*`, `/`.
- Use `extern metal` for conditions, intrinsics, component access, neighbor
  indexing, atomics, reductions, scans, compaction, textures, and rendering.
- Order all conflicting passes; draw only after the pass that completes every
  view input.
- Run `pqo check` after structural changes and `pqo explain` before asserting
  binding, generated Metal, or execution order.
