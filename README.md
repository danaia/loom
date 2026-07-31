# Pqo

Install Pqo on an Apple Silicon Mac:

```sh
curl -fsSL https://raw.githubusercontent.com/danaia/pqo/main/install.sh | sh
```

Or build the current checkout and install it globally for your user with the
same verified installer flow:

```sh
./scripts/install-global.sh
```

Both commands install under `~/.pqo` and expose `pqo` through
`~/.local/bin/pqo`. Override `PQO_HOME` or `PQO_BIN_DIR` when a different
user-local location is needed.

## The cool part

Pqo is an agent-native, low-level GPU language for Apple Silicon. It lets AI
agents author, validate, inspect, repair, and run Metal applications from one
`.pqo` source.

In other words: you describe a world once, and Pqo turns it into validated,
native GPU code that can evolve millions of particles, cells, robots, or agents
in parallel. The source stays compact and inspectable, the generated Metal stays
visible, and an AI agent can diagnose a failure, repair the program, and keep it
running—all from the same `.pqo` file.

This creates a tight self-reflective development loop for fast autonomous
systems—including swarms, simulations, and procedural organisms that can detect
change, adapt, and repair their own state directly on the GPU.

Typed streams, bounded memory, explicit effects, kernel passes, validated
execution graphs, Metal generation, and native rendering remain visible to the
agent. It's way powerful and open source. Enjoy!

## What that power can be used for

A Pqo element can represent a particle, robot, cell, transaction, material
point, network node, order, or task. The GPU can update large populations of
these elements in parallel while Pqo keeps their memory, rules, passes, and
effects explicit.

That creates a useful foundation for:

- **Robotics:** sensor processing, multi-robot coordination, path search,
  collision avoidance, and digital twins
- **Finance:** market simulation, portfolio stress testing, risk scenarios,
  fraud-pattern analysis, and large transaction models
- **Cryptocurrency networks:** transaction-graph analysis, network simulation,
  consensus experiments, and adversarial testing
- **Biology and medicine:** cell populations, tissue development, reaction
  fields, injury, and regeneration research
- **Materials and manufacturing:** crystal growth, fracture, repair, granular
  systems, and process simulation
- **Graphics and games:** particles, crowds, procedural worlds, physics, and
  native GPU rendering
- **Infrastructure and logistics:** traffic, routing, scheduling, supply
  networks, and resource allocation
- **Autonomous software:** many local decision-makers operating inside bounded,
  validated rules

Pqo does not automatically solve these industries. It provides a low-level
way to build and test the large parallel systems they increasingly depend on.


## Why this is different

Most tools handle one part of the problem:

- an AI agent writes code
- a GPU framework runs kernels
- a graphics engine renders output
- a simulation framework coordinates many actors
- a validator checks whether the program is safe to run

Pqo is designed to combine these parts in one language and runtime.

An AI agent can author a low-level GPU program, validate its memory and resource
access, inspect the generated Metal, run millions of parallel elements, render
the result, and continue refining the same `.pqo` source.

This combination is unusual. Pqo is not a prompt wrapper, a shader toy, or a
finished simulation engine. It is an attempt to make agent-authored autonomous
systems a first-class GPU programming model.

You write a `.pqo` program. Pqo validates it, generates Metal for supported
kernels, connects declared Metal kernels, schedules the GPU work, and opens a
native window.


## Get started

Run a particle:

```sh
pqo ~/.pqo/examples/hello-particle.pqo
```

Run the interactive self-healing crystal:

```sh
pqo ~/.pqo/examples/crystal.pqo
```

You do not need Rust or Cargo to use the installed release.

## Portable `.lmp` projects

A Pqo project keeps its primary `.pqo` file, referenced Metal sources, and
optional `src/runtime.rs` extension in one directory. Build that directory into
a single distributable package:

```sh
pqo build examples/marble-water/marble-water.pqo
pqo examples/marble-water/marble-water.lmp
```

The `.lmp` contains the graph, every referenced Metal file, the Rust extension
source, and its compiled target library. Running the package requires only the
global `pqo` runtime. Building a project with `src/runtime.rs` requires `rustc`.

`.lmp` is ZIP-compatible, so a package can be reopened for editing:

```sh
mkdir marble-water
unzip marble-water.lmp -d marble-water
cd marble-water
pqo build marble-water.pqo
```

## What Pqo does

A Pqo program defines:

- GPU data, types, units, and memory limits
- which kernels may read or change that data
- the order of compute passes
- rendering
- the update rate

Pqo then:

```text
parses the .pqo source
→ checks types, units, memory, and access
→ builds a validated execution graph
→ generates or loads Metal kernels
→ runs the program on the Apple GPU
```

## How Pqo is different technically

Pqo is not a Python wrapper around a finished engine. The `.pqo` file owns the
GPU program.

- It exposes low-level GPU concepts such as streams, capacities, access modes,
  kernels, passes, and flows.
- Its compact syntax is designed for AI coding agents.
- Supported Pqo kernels compile into inspectable Metal source.
- Unsupported work stays visible as `extern metal`.
- `pqo check` rejects invalid programs before they run.
- `pqo explain` shows the graph, execution plan, and generated Metal.

The goal is a CUDA-like authoring language for Metal, designed for agents and
GPU systems with many independent elements.

## Why it is useful for organisms

A GPU can update thousands or millions of elements at the same time. An element
can represent a particle, cell, voxel, material point, or agent.

Pqo gives those elements persistent state, local rules, shared fields, bounded
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

## A real Pqo kernel

This kernel updates particle velocity and position:

```pqo
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

Pqo checks the types and units, then generates the Metal implementation.

The program schedules it at 120 Hz:

```pqo
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
[`examples/hello-particle/hello-particle.pqo`](examples/hello-particle/hello-particle.pqo).

## Examples

### Hello Particle

One particle falls, hits the ground, and renders in a Metal window.

```sh
pqo examples/hello-particle/hello-particle.pqo
```

Its integration kernel is native Pqo. Ground contact and rendering are explicit
Metal kernels.

### Hello Crystal

A one-million-cell, 100 × 100 × 100 mesoscopic crystal grows on the GPU. Drag
across it to slice it, drag the black background to spin it, and scroll to zoom.
The cut glows red while damaged, then heals itself.

```sh
pqo examples/hello-crystal/crystal.pqo
```

Pqo owns the typed GPU graph. Advanced field, neighborhood, component, and
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
pqo program.pqo          # Run
pqo project.lmp           # Run a self-contained package
pqo build program.pqo    # Build program.lmp
pqo check program.pqo    # Validate
pqo check project.lmp     # Validate a package
pqo explain program.pqo  # Inspect the graph and generated Metal
pqo new my-project        # Start from the Baseline project
pqo update                # Install the latest release
pqo --version             # Print the version
```

`pqo program.pqo` and `pqo run program.pqo` are equivalent.

## Ask an AI agent to build something

```text
Read docs/agent-coding-reference.md completely, then read the closest runnable
example.

Build a runnable .pqo particle system with visible GPU motion.
Use native Pqo for supported kernels and explicit extern metal for unsupported
work. Do not invent syntax.

Finish only when these commands succeed:
pqo check PATH_TO_PROGRAM
pqo explain PATH_TO_PROGRAM
pqo PATH_TO_PROGRAM

Document which kernels are native Pqo and which are external Metal.
```

The [AI agent coding reference](docs/agent-coding-reference.md) gives smaller
models a literal executable grammar, a complete experiment template, current
compiler limits, diagnostic repairs, and an evidence checklist.

## Current scope

Pqo is early software. Native Pqo currently supports the first element-wise
arithmetic kernel class.

Conditionals, fields, stencils, atomics, reductions, neighborhood access, scans,
compaction, component relaxation, and native render stages are being added as
measured compiler gates. Until a gate is complete, that work remains explicit
Metal.

See the [native compiler roadmap](docs/native-compiler-gates.md).

## Update or remove

```sh
pqo update
```

```sh
curl -fsSL https://raw.githubusercontent.com/danaia/pqo/main/uninstall.sh | sh
```

## More

- [Visual handbook](docs/handbook/index.html)
- [Language design](docs/README.md)
- [Runnable examples](examples/README.md)
