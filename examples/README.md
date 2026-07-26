# Runnable Loom examples

Every `.loom` entry point in this directory can be launched directly with the
Loom CLI.

From the repository root:

```text
loom examples/hello-particle/hello-particle.loom
loom examples/hello-crystal/crystal.loom
```

The Crystal example is interactive: drag across the crystal to slice it, drag
the black background to spin it, and scroll to zoom. The cut heals
automatically.

From inside an example directory:

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

`loom run FILE` is the explicit equivalent of `loom FILE`. Use `loom check FILE`
to validate without opening a window and `loom explain FILE` to inspect the
canonical graph, execution plan, and generated or packaged Metal.

Update the installed compiler and runtime with:

```text
loom update
```
