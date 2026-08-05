# Pqo baseline

A deliberately minimal, complete Pqo project: a selectable pool of
GPU-resident particles in zero gravity. Click and drag a particle to reposition
it directly, or click a particle and then click open space to assign its
destination. A small gold dot remains at that
destination until the selected particle arrives and stops. Choose an agent type
in the control panel, then hold Command and click space to add it at the cursor. New agents
default to `General`. Each agent must have a unique name; the shared roster is
also visible in the Pqo Agents window.

Clicking a particle publishes its GPU-selected ID to the control panel and opens
that particle's metadata card. Names, types, attached project skill paths, and
schema-defined custom fields are stored atomically in `agentDB/particles.json`.
The schema can mark controls as global, particle-specific, or read-only, and
new defaults are merged into older records. The Pqo Agents window receives the
selected record and can inspect any attached project skill before responding.

The project keeps the full application path ready for extension:

- a typed Pqo graph running at 120 Hz
- a Metal physics/projection kernel
- a Metal particle shader
- a native Rust input extension
- a Vue control panel
- FPS, GPU memory, GPU frame-time, budget, and pressure telemetry
- a portable `.lmp` build

Linux/NVIDIA projects can start from `baseline-cuda.pqo`. It represents one
neutral hydrogen atom as a point nucleus plus a normalized `1s` electron-density
field sampled over `100^3` GPU cells. A `25^3` hierarchy provides density-aware
culling, four detail levels, compact shared-presentation streams, and per-LOD
counters. Its CUDA/Vulkan target opens a dedicated volumetric hydrogen `1s`
probability-cloud view rather than the procedural crystal. It now also declares
an ideal B-DNA dodecamer hierarchy and opens a project control panel for moving
between hydrogen, nucleotide sites, base pairs, double-helix, and continuum
representations. See `../docs/DNA-SANDBOX.md` for that model's physical limits
and `AI_AGENT_README.md` for the scaling and evidence loop an AI coding agent
should follow.

From the repository root:

```sh
pqo check baseline/baseline.pqo
pqo build baseline/baseline.pqo
pqo baseline/baseline.lmp
```

CUDA headless validation and execution:

```sh
pqo check baseline/baseline-cuda.pqo --target cuda-headless
PQO_HEADLESS_TICKS=1 PQO_INSPECT_STREAM=metrics.total_probability \
  pqo run baseline/baseline-cuda.pqo --target cuda-headless
```

`Space drag` defaults to zero. The viewer wraps particle positions at the
edges to represent unbounded space without introducing collision forces.
