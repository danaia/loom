# Runnable Loom examples

Every `.loom` entry point in this directory can be checked or launched directly
with the Loom CLI.

```text
cd examples/hello-particle
loom hello-particle.loom

cd ../hello-crystal
loom crystal.loom
```

The installed distribution places the same programs at:

```text
loom ~/.loom/examples/hello-particle.loom
loom ~/.loom/examples/crystal.loom
```

Use `loom check FILE` to validate without opening a window and `loom explain
FILE` to inspect the canonical graph, execution plan, and generated or packaged
Metal.
