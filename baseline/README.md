# Loom baseline

A deliberately minimal, complete Loom project: a selectable pool of
GPU-resident particles in zero gravity. Click and drag a particle to reposition
it directly, or click a particle and then click open space to assign its
destination. A small gold dot remains at that
destination until the selected particle arrives and stops. Choose an agent type
in the control panel, then hold Command and click space to add it at the cursor. New agents
default to `General`. Each agent must have a unique name; the shared roster is
also visible in the Loom Agents window.

Clicking a particle publishes its GPU-selected ID to the control panel and opens
that particle's metadata card. Names, types, attached project skill paths, and
schema-defined custom fields are stored atomically in `agentDB/particles.json`.
The schema can mark controls as global, particle-specific, or read-only, and
new defaults are merged into older records. The Loom Agents window receives the
selected record and can inspect any attached project skill before responding.

The project keeps the full application path ready for extension:

- a typed Loom graph running at 120 Hz
- a Metal physics/projection kernel
- a Metal particle shader
- a CUDA baseline graph with CUDA kernels and an OptiX view declaration
- a native Rust input extension
- a Vue control panel
- FPS, GPU memory, GPU frame-time, budget, and pressure telemetry
- a portable `.lmp` build

From the repository root:

```sh
loom check baseline/baseline.loom
loom build baseline/baseline.loom
loom baseline/baseline.lmp
```

On a Linux RTX workstation, validate the CUDA baseline:

```sh
loom-cuda check baseline/baseline.cuda.loom
loom-cuda explain baseline/baseline.cuda.loom
```

`baseline.cuda.loom` is the CUDA-native version of the baseline graph. Full
interactive CUDA execution will use that entry once the `loom-cuda` runtime
backend lands.

`Space drag` defaults to zero. The viewer wraps particle positions at the
edges to represent unbounded space without introducing collision forces.
