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
source, the project extension source, and a target-specific compiled extension.
The manifest records the format version, module, entry graph, file inventory,
extension ABI, and target triple.

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
`src/runtime.rs`, then run `loom build marble-water.loom` again. The extracted
target-specific `runtime/` directory is a generated artifact and may be deleted.
