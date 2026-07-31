# Pqo Package Format

A Pqo source project is a directory rooted at its primary `.pqo` file.
External source paths in the graph are relative to that directory and may not
be absolute or contain `..`.

```text
marble-water/
├── marble-water.pqo
├── config/
│   └── window-layout.json
├── kernels/
│   └── marble_water.metal
├── shaders/
│   └── marble_water.metal
├── ui/
│   ├── pqo-ui.json
│   ├── package.json
│   ├── src/
│   └── dist/
└── src/
    └── runtime.rs
```

`src/runtime.rs` is optional. When present, `pqo build` compiles it as a
versioned project extension using only the stable C ABI supplied by the global
runtime.

```text
pqo build marble-water.pqo
pqo marble-water.lmp
```

The resulting `.lmp` is a ZIP-compatible archive with a distinct extension. Its
root contains `pqo-package.json`, the primary graph, every referenced external
source, the project extension source, a target-specific compiled extension, and
an optional project UI. The manifest records the format version, module, entry
graph, file inventory, extension ABI, target triple, and UI entry point.

When `ui/pqo-ui.json` exists, `pqo build` runs the UI package's `npm run build`
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

The global Pqo runtime owns validation, scheduling, Metal execution, windowing,
and host statistics. The package owns all application-specific graphs, shaders,
input behavior, HUD behavior, and value overrides.

`config/window-layout.json` is optional and keeps project-specific snapping,
detachment, and linked-movement policy outside both the Metal renderer and the
Vue UI. See [`window-layout.md`](window-layout.md).

## Extract and edit

`.lmp` is ZIP-compatible. Extract it into a new source directory:

```text
mkdir marble-water
unzip marble-water.lmp -d marble-water
cd marble-water
```

Edit the primary `.pqo`, files under `kernels/` or `shaders/`, or
`src/runtime.rs`, or the Vue project under `ui/`, then run
`pqo build marble-water.pqo` again. The extracted target-specific `runtime/`
directory and `ui/dist/` are generated artifacts and may be deleted.
