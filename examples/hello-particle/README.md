# Hello Particle

Update Pqo first:

```text
pqo update
pqo --version
```

## Run from this directory

```text
cd examples/hello-particle
pqo hello-particle.pqo
```

The explicit form does the same thing:

```text
pqo run hello-particle.pqo
```

## Run from the repository root

```text
pqo examples/hello-particle/hello-particle.pqo
```

## Run the installed copy

The curl installer places a copy that can be launched from any directory:

```text
pqo ~/.pqo/examples/hello-particle.pqo
```

## Check and explain

These commands do not open the Metal window:

```text
pqo check hello-particle.pqo
pqo explain hello-particle.pqo
```

This example uses a native Pqo integration kernel, an explicit Metal ground
contact kernel, and an explicit Metal particle view.
