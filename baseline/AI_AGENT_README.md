# Building high-performance 3D worlds from the Pqo Baseline

This directory contains two sibling starters:

- `baseline.pqo`: interactive native Metal application for macOS.
- `baseline-cuda.pqo`: one-particle CUDA compute baseline for Linux/NVIDIA,
  with CUDA/Vulkan-ready presentation data and GPU-selected LOD.

Start from the file for the target you will actually run. Do not merge their
backend-specific kernels into one graph until both implementations share a
frozen Pqo resource and kernel contract.

## Agent contract

Before editing, read `AGENTS.md`, then
`skills/engineer-pqo-systems/SKILL.md`. Treat the primary `.pqo` file as the
authority for state, resource access, bindings, dispatch, and pass order. Use
native Pqo only for same-index f32 arithmetic. Put branches, culling,
neighborhoods, scans, atomics, complex math, and rendering behind an explicit
target implementation.

Never move authoritative world state, visibility selection, or population-wide
loops into Rust, Vue, or a per-frame CPU readback. The UI may change controls;
it does not own the simulation.

## What the CUDA starter already establishes

The CUDA graph intentionally begins with one particle but preserves the
architecture needed to grow:

- structure-of-arrays authoritative state in `particle.*`;
- fixed-step 120 Hz native integration compiled to CUDA;
- a separate GPU projection/classification pass;
- distance culling and four GPU-selected detail classes;
- compact `presentation.*` streams instead of rendering from simulation state;
- per-LOD GPU counters for later indirect draw generation;
- `shared_presentation` resource domains and two buffer versions for the
  future CUDA/Vulkan presentation ring;
- CUDA Graph execution, asynchronous device allocation, VRAM budgeting, and
  benchmark telemetry supplied by the Pqo CUDA runtime.

The checked-in Linux renderer is still a Vulkan backend bring-up surface. It
does not yet draw these project-specific presentation streams. The CUDA
simulation and LOD output are real and inspectable today; live graph-driven
Vulkan rendering and CUDA-generated indirect draws remain backend integration
gates. Do not claim that a new world is visually connected until that path has
been implemented and observed.

## Safe world-building sequence

1. State one testable behavior and its controlled inputs.
2. Preserve the CUDA baseline as a runnable control; create a named variant for
   substantial experiments.
3. Add authoritative state as aligned, typed streams. Prefer SoA layouts and
   `domain=compute_private` for simulation-only data.
4. Add the smallest rule that changes that state. Keep element-wise arithmetic
   native; use `extern cuda` for advanced work.
5. Build compact renderer-facing streams in a dedicated projection pass. Keep
   them `domain=shared_presentation`; do not make them authoritative.
6. Order producers, consumers, compaction, LOD, and presentation explicitly in
   the flow.
7. Validate after every structural change, inspect the execution plan, then run
   a fixed tick count and inspect a named stream.

For a project created by `pqo new`, locate the CUDA source with
`ls *cuda.pqo`; its filename may retain the Baseline name until you rename the
variant.

## Scaling beyond one particle

When increasing the population, change `cap`, `len`, and `init` together for
every aligned `particle.*` and `presentation.*` stream. Keep the four-element
LOD counter unchanged. The CUDA runtime dispatches by logical stream length, so
an inactive capacity is not an optimization unless the graph uses a supported
device-resident dynamic count.

Do not stop at a full-population projection when the visible set becomes sparse.
The scalable progression is:

```text
authoritative SoA state
-> bounded spatial hierarchy
-> frustum/occlusion classification
-> prefix scan and visible compaction
-> LOD-specific instance lists
-> GPU-generated indirect draw commands
-> versioned CUDA/Vulkan presentation lease
```

Use fixed-size world cells or clusters, not an all-pairs neighborhood search.
For evolving fields, use ping-pong buffers so every neighbor reads the same
prior generation. For large populations, use count-backed active dispatch and
hierarchical scans; never serialize the population on the CPU.

## LOD policy

The starter uses squared camera distance, avoiding a square root per particle.
LOD 0 is full detail, LOD 1 medium, LOD 2 coarse, and LOD 3 an impostor. Objects
beyond `camera.cull_distance` are invisible and do not contribute to a counter.

For a sophisticated world, replace raw distance with projected screen error and
add hysteresis so instances do not flicker between levels. Keep the decision on
the GPU. Store only the metadata the renderer consumes, and measure whether an
extra classification pass saves more rendering work than it costs.

## Required evidence loop

From the repository root:

```sh
cargo run -p pqo-cli -- check baseline/baseline-cuda.pqo --target cuda-headless
cargo run -p pqo-cli -- explain baseline/baseline-cuda.pqo --target cuda-headless
PQO_HEADLESS_TICKS=120 PQO_INSPECT_STREAM=presentation.lod \
  cargo run -p pqo-cli -- run baseline/baseline-cuda.pqo --target cuda-headless
cargo run -p pqo-cli -- build baseline/baseline-cuda.pqo --target cuda-headless
```

For a performance claim, use a release build, fixed workload, warm-up, repeated
samples, and record the GPU and driver:

```sh
PQO_TELEMETRY=benchmark PQO_WARMUP_TICKS=120 PQO_HEADLESS_TICKS=1200 \
  cargo run --release -p pqo-cli -- \
  run baseline/baseline-cuda.pqo --target cuda-headless
```

Report the question, controlled inputs, changed streams and kernels, native vs.
external boundary, check status, graph hash, artifact fingerprint, explain
findings, run/build result, inspected output, device, and limitations. A valid
graph proves legality; it does not prove appearance, correctness, or speed.
