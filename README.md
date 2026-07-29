# Loom

Loom is an agent-native, low-level GPU language and execution model with support
for both Metal and CUDA systems. The Metal runtime is optimized for Apple Silicon,
and CUDA/RTX support is live for Linux workstation installs, CUDA-target graph
validation, explicit CUDA/OptiX backend declarations, and the `loom-cuda` runtime
path.

It lets AI agents author, validate, inspect, repair, and run GPU applications
from one `.loom` source.

Programs run as compiled GPU kernels instead of interpreted agent logic.
An agent can inspect diagnostics and generated backend code, correct failed
source, and continue refining the same program.

This creates a tight self-reflective development loop for fast autonomous
systems—including swarms, simulations, and procedural organisms that can detect
change, adapt, and repair their own state directly on the GPU.

Typed streams, bounded memory, explicit effects, kernel passes, validated
execution graphs, backend-specific kernels, and native rendering remain visible
to the agent. It's way powerful and open source. Enjoy!

## What that power can be used for

A Loom element can represent a particle, robot, cell, transaction, material
point, network node, order, or task. The GPU can update large populations of
these elements in parallel while Loom keeps their memory, rules, passes, and
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

Loom does not automatically solve these industries. It provides a low-level
way to build and test the large parallel systems they increasingly depend on.


## Why this is different

Most tools handle one part of the problem:

- an AI agent writes code
- a GPU framework runs kernels
- a graphics engine renders output
- a simulation framework coordinates many actors
- a validator checks whether the program is safe to run

Loom is designed to combine these parts in one language and runtime.

An AI agent can author a low-level GPU program, validate its memory and resource
access, inspect generated backend code, run millions of parallel elements, render
the result, and continue refining the same `.loom` source.

This combination is unusual. Loom is not a prompt wrapper, a shader toy, or a
finished simulation engine. It is an attempt to make agent-authored autonomous
systems a first-class GPU programming model.

You write a `.loom` program. Loom validates it, lowers to the selected backend,
connects declared Metal or CUDA kernels, schedules the GPU work, and uses the
available native runtime for execution.


## Get started

Loom supports backend-specific release packages:

- `loom-metal-darwin-arm64` for Apple Silicon Macs with Metal.
- `loom-cuda-linux-x86_64` for CUDA/RTX Linux workstations. CUDA support
  currently covers installation, validation, package handling, `target cuda`,
  `extern cuda`, and `extern optix`; full CUDA execution lands with the
  `loom-cuda` runtime backend.

Install Metal on Apple Silicon:

```sh
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/install.sh | LOOM_BACKEND=metal sh
```

Install CUDA on a Linux RTX workstation:

```sh
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/install.sh | LOOM_BACKEND=cuda sh
```

The installer creates a backend-specific command, `loom-metal` or `loom-cuda`,
and points `loom` at the backend you just installed. Set `LOOM_SET_DEFAULT=0` to
install only the backend-specific command and leave the existing `loom` default
alone.

Run a Metal particle:

```sh
loom-metal ~/.loom-metal/examples/hello-particle.loom
```

Run the interactive self-healing crystal:

```sh
loom-metal ~/.loom-metal/examples/crystal.loom
```

You do not need Rust or Cargo to use the installed release.

## Portable `.lmp` projects

A Loom project keeps its primary `.loom` file, referenced backend sources, and
optional `src/runtime.rs` extension in one directory. Build that directory into
a single distributable package:

```sh
loom build examples/marble-water/marble-water.loom
loom examples/marble-water/marble-water.lmp
```

The `.lmp` contains the graph, every referenced backend file, the Rust extension
source, and its compiled target library. Running the package requires only the
global `loom` runtime. Building a project with `src/runtime.rs` requires `rustc`.

`.lmp` is ZIP-compatible, so a package can be reopened for editing:

```sh
mkdir marble-water
unzip marble-water.lmp -d marble-water
cd marble-water
loom build marble-water.loom
```

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
→ generates or loads backend kernels
→ runs the program on the selected GPU runtime
```

## How Loom is different technically

Loom is not a Python wrapper around a finished engine. The `.loom` file owns the
GPU program.

- It exposes low-level GPU concepts such as streams, capacities, access modes,
  kernels, passes, and flows.
- Its compact syntax is designed for AI coding agents.
- Supported Loom kernels compile into inspectable backend source.
- Backend-specific work stays visible as `extern metal`, `extern cuda`, or
  `extern optix`.
- `loom check` rejects invalid programs before they run.
- `loom explain` shows the graph, execution plan, and generated backend code.

The goal is an agent-native authoring language for high-performance GPU systems:
Metal and CUDA/RTX, with many independent elements kept explicit and
measurable.

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

Loom checks the types and units, then generates the backend implementation for
the selected target.

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

A one-million-cell, 100 × 100 × 100 mesoscopic crystal grows on the GPU. Drag
across it to slice it, drag the black background to spin it, and scroll to zoom.
The cut glows red while damaged, then heals itself.

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

Use `loom` for the selected default backend, or call `loom-metal` and `loom-cuda`
explicitly when both are installed.

```sh
loom program.loom          # Run
loom project.lmp           # Run a self-contained package
loom build program.loom    # Build program.lmp
loom check program.loom    # Validate
loom check project.lmp     # Validate a package
loom explain program.loom  # Inspect the graph, plan, and backend code
loom new my-project        # Start from the Baseline project
loom update                # Install the latest release
loom --version             # Print the version
```

`loom program.loom` and `loom run program.loom` are equivalent.

## Ask an AI agent to build something

```text
Read docs/agent-coding-reference.md completely, then read the closest runnable
example.

Build a runnable .loom particle system with visible GPU motion.
Use native Loom for supported kernels and explicit `extern metal`, `extern cuda`,
or `extern optix` for backend-specific work. Do not invent syntax.

Finish only when these commands succeed:
loom check PATH_TO_PROGRAM
loom explain PATH_TO_PROGRAM
loom PATH_TO_PROGRAM

Document which kernels are native Loom and which are external Metal.
```

The [AI agent coding reference](docs/agent-coding-reference.md) gives smaller
models a literal executable grammar, a complete experiment template, current
compiler limits, diagnostic repairs, and an evidence checklist.

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
loom update        # selected default backend
loom-metal update  # Metal install
loom-cuda update   # CUDA install
```

```sh
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/uninstall.sh | sh
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/uninstall.sh | LOOM_BACKEND=metal sh
curl -fsSL https://raw.githubusercontent.com/danaia/loom/main/uninstall.sh | LOOM_BACKEND=cuda sh
```

## More

- [Visual handbook](docs/handbook/index.html)
- [Language design](docs/README.md)
- [Runnable examples](examples/README.md)
