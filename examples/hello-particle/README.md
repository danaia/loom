# Hello Particle

Update Loom first:

```text
loom update
loom --version
```

## Run from this directory

```text
cd examples/hello-particle
loom hello-particle.loom
```

The explicit form does the same thing:

```text
loom run hello-particle.loom
```

## Run from the repository root

```text
loom examples/hello-particle/hello-particle.loom
```

## Run the installed copy

The curl installer places a copy that can be launched from any directory:

```text
loom ~/.loom/examples/hello-particle.loom
```

## Check and explain

These commands do not open the Metal window:

```text
loom check hello-particle.loom
loom explain hello-particle.loom
```

This example uses a native Loom integration kernel, an explicit Metal ground
contact kernel, and an explicit Metal particle view.
