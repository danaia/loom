# Loom

Loom is an agent-native language for building GPU applications on Apple
Silicon.

You write a `.loom` program. Loom validates it, generates Metal for supported
kernels, connects declared Metal kernels, schedules the GPU work, and opens a
native window.

## Get started

Loom currently requires an Apple Silicon Mac.

Install:

```sh
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/install.sh | sh
```

Run a particle:

```sh
loom ~/.loom/examples/hello-particle.loom
```

Run a crystal:

```sh
loom ~/.loom/examples/crystal.loom
```

You do not need Rust or Cargo to use the installed release.

## What Loom does

A Loom program defines:

- GPU data, types, units, and memory limits
- which kernels may read or change that data
- the order of compute passes
- rendering
- the update rate

Loom then:

```text
parses the .loom source
→ checks types, units, memory, and access
→ builds a validated execution graph
→ generates or loads Metal kernels
→ runs the program on the Apple GPU
```

## Why Loom is different

Loom is not a Python wrapper around a finished engine. The `.loom` file owns the
GPU program.

- It exposes low-level GPU concepts such as streams, capacities, access modes,
  kernels, passes, and flows.
- Its compact syntax is designed for AI coding agents.
- Supported Loom kernels compile into inspectable Metal source.
- Unsupported work stays visible as `extern metal`.
- `loom check` rejects invalid programs before they run.
- `loom explain` shows the graph, execution plan, and generated Metal.

The goal is a CUDA-like authoring language for Metal, designed for agents and
GPU systems with many independent elements.

## Why it is useful for organisms

A GPU can update thousands or millions of elements at the same time. An element
can represent a particle, cell, voxel, material point, or agent.

Loom gives those elements persistent state, local rules, shared fields, bounded
memory, ordered compute passes, and rendering.

This can be used to build:

- particles and swarms
- cellular systems and procedural organisms
- reaction-diffusion fields
- crystals and materials
- physics experiments
- large interactive simulations

The program defines the rules. The GPU calculates the resulting structure over
time.

## A real Loom kernel

This kernel updates particle velocity and position:

```loom
kernel integrate(
  position: rw stream<f32x3,m>,
  velocity: rw stream<f32x3,m/s>,
  gravity: in value<f32x3,m/s^2>,
  dt: in value<f32,s>
) each i {
  velocity[i] += gravity * dt;
  position[i] += velocity[i] * dt;
}
```

Loom checks the types and units, then generates the Metal implementation.

The program schedules it at 120 Hz:

```loom
pass fall = integrate(
  position=particles.position
  velocity=particles.velocity
  gravity=world.gravity
  dt=simulation.fixed_dt
) over particles.position

flow simulation rate=120hz {
  fall -> bounce
  draw viewport after bounce
}
```

Read the complete source:
[`examples/hello-particle/hello-particle.loom`](examples/hello-particle/hello-particle.loom).

## Examples

### Hello Particle

One particle falls, hits the ground, and renders in a Metal window.

```sh
loom examples/hello-particle/hello-particle.loom
```

Its integration kernel is native Loom. Ground contact and rendering are explicit
Metal kernels.

### Hello Crystal

A 32 × 32 × 32 mesoscopic crystal grows, can be cut, and repairs damage.

```sh
loom examples/hello-crystal/crystal.loom
```

Loom owns the typed GPU graph. Advanced field, neighborhood, component, and
render kernels currently use explicit Metal.

### Hello Organism

The organism research example tests bounded populations, fields, local
perception, development, damage, and regeneration.

It is currently a compiler-development example:

```sh
./scripts/run-hello-particle.sh organism 16384
```

## Commands

```sh
loom program.loom          # Run
loom check program.loom    # Validate
loom explain program.loom  # Inspect the graph and generated Metal
loom update                # Install the latest release
loom --version             # Print the version
```

`loom program.loom` and `loom run program.loom` are equivalent.

## Ask an AI agent to build something

```text
Read the Loom grammar and Hello Particle example.

Build a runnable .loom particle system with visible GPU motion.
Use native Loom for supported kernels and explicit extern metal for unsupported
work. Do not invent syntax.

Finish only when these commands succeed:
loom check PATH_TO_PROGRAM
loom explain PATH_TO_PROGRAM
loom PATH_TO_PROGRAM

Document which kernels are native Loom and which are external Metal.
```

## Current scope

Loom is early software. Native Loom currently supports the first element-wise
arithmetic kernel class.

Conditionals, fields, stencils, atomics, reductions, neighborhood access, scans,
compaction, component relaxation, and native render stages are being added as
measured compiler gates. Until a gate is complete, that work remains explicit
Metal.

See the [native compiler roadmap](docs/native-compiler-gates.md).

## Update or remove

```sh
loom update
```

```sh
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/uninstall.sh | sh
```

## More

- [Visual handbook](docs/handbook/index.html)
- [Language design](docs/README.md)
- [Runnable examples](examples/README.md)
