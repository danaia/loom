# Hello Particle

Run the source-tree example directly with the installed Loom CLI:

```text
cd examples/hello-particle
loom hello-particle.loom
```

Validate or inspect it without opening the Metal window:

```text
loom check hello-particle.loom
loom explain hello-particle.loom
```

The curl-installed copy is available from any directory:

```text
loom ~/.loom/examples/hello-particle.loom
```

This example uses a native Loom integration kernel, an explicit Metal ground
contact kernel, and an explicit Metal particle view.
