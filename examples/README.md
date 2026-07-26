# Runnable Loom examples

Every `.loom` entry point in this directory can be checked or launched directly
with the Loom CLI.

```text
loom examples/hello-particle/hello-particle.loom
loom examples/hello-crystal/crystal.loom
```

The installed distribution places the same programs at:

```text
loom ~/.loom/examples/hello-particle.loom
loom ~/.loom/examples/crystal.loom
```

Use `loom check FILE` to validate without opening a window and `loom explain
FILE` to inspect the canonical graph, execution plan, and generated or packaged
Metal.
