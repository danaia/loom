# Loom Package Format

A Loom source project is a directory rooted at its primary `.loom` file.
External source paths in the graph are relative to that directory and may not
be absolute or contain `..`.

```text
marble-water/
├── marble-water.loom
├── kernels/
│   └── marble_water.metal
├── shaders/
│   └── marble_water.metal
├── ui/
│   ├── loom-ui.json
│   ├── package.json
│   ├── src/
│   └── dist/
└── src/
    └── runtime.rs
```

`src/runtime.rs` is optional. When present, `loom build` compiles it as a
versioned project extension using only the stable C ABI supplied by the global
runtime.

```text
loom build marble-water.loom
loom marble-water.lmp
```

The resulting `.lmp` is a ZIP-compatible archive with a distinct extension. Its
root contains `loom-package.json`, the primary graph, every referenced external
source, the project extension source, a target-specific compiled extension, and
an optional project UI. The manifest records the format version, module, entry
graph, file inventory, extension ABI, target triple, and UI entry point.

When `ui/loom-ui.json` exists, `loom build` runs the UI package's `npm run build`
script and includes both its Vue source and generated assets. Running the `.lmp`
opens those assets in the installed generic Tauri panel beside the Metal viewer.
The UI is project-owned; the Tauri shell and authenticated local IPC bridge are
runtime-owned.

```json
{
  "framework": "vue3",
  "dist": "dist",
  "entry": "index.html",
  "title": "Marble Water — Controls",
  "width": 390,
  "height": 830
}
```

The global Loom runtime owns validation, scheduling, Metal execution, windowing,
and host statistics. The package owns all application-specific graphs, shaders,
input behavior, HUD behavior, and value overrides.

## Extract and edit

`.lmp` is ZIP-compatible. Extract it into a new source directory:

```text
mkdir marble-water
unzip marble-water.lmp -d marble-water
cd marble-water
```

Edit the primary `.loom`, files under `kernels/` or `shaders/`, or
`src/runtime.rs`, or the Vue project under `ui/`, then run
`loom build marble-water.loom` again. The extracted target-specific `runtime/`
directory and `ui/dist/` are generated artifacts and may be deleted.
