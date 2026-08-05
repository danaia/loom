# Building high-performance 3D worlds from the Pqo Baseline

This directory contains two sibling starters:

- `baseline.pqo`: interactive native Metal application for macOS.
- `baseline-cuda.pqo`: one neutral-hydrogen CUDA baseline for Linux/NVIDIA,
  with a normalized `100^3` electron-density field, a `25^3` GPU hierarchy,
  and CUDA/Vulkan-ready presentation data.

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

The CUDA graph intentionally begins with one atom and preserves the architecture
needed to grow:

- structure-of-arrays atomic state in `atom.*`;
- a point nucleus plus the normalized hydrogen `1s` probability density
  `rho(r) = exp(-2r/a0) / (pi*a0^3)`;
- exactly one million cell-centred field samples in `field.electron_density`;
- block-local numerical reductions with only two global atomics per CUDA block;
- a 15,625-leaf hierarchy in which each thread classifies one `4^3` cluster;
- density-aware culling and four GPU-selected detail classes;
- compact `presentation.*` streams instead of rendering from simulation state;
- per-LOD GPU counters for later indirect draw generation;
- `shared_presentation` resource domains and two buffer versions for the
  future CUDA/Vulkan presentation ring;
- CUDA Graph execution, asynchronous device allocation, VRAM budgeting, and
  benchmark telemetry supplied by the Pqo CUDA runtime.

The checked-in Linux renderer recognizes this atom contract and displays a
dedicated volumetric Vulkan view of the same analytic hydrogen `1s` density; it
does not show the procedural crystal. The shader does not yet sample the
project's CUDA field through shared memory, so the numerical field and analytic
view are parallel evaluations of the same equation. Live shared-field volume
rendering and CUDA-generated indirect draws remain backend integration gates.

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

## What “atom” means

This is the nonrelativistic, spin-free, isolated hydrogen ground state with a
fixed point nucleus. It is not a classical electron orbit, a hard sphere, a
many-electron Hartree-Fock solution, or a relativistic/QED model. The
`presentation.radius` proxy is `4*a0`, which contains about 98.62% of ideal
hydrogen `1s` probability. The continuous density—not that radius—is the
physical representation.

The Bohr radius is the 2022 CODATA value from NIST. The executable field is a
single-precision midpoint discretization on a finite cube spanning `-8*a0` to
`+8*a0` on each axis, so it should converge toward the analytic state rather
than equal it bit-for-bit. The two `metrics.*` streams make truncation,
discretization, and reduction error observable.

The graph records mass and charge in addition to the density parameters. Charge
is expressed in elementary-charge units because executable Pqo 0.1 does not yet
have electric current or charge among its physical unit bases.

## Scaling beyond one atom

The checked-in `100^3` domain is one million computational cells sampling one
atom; it is not one million interacting atoms. Never allocate a private `100^3`
density field for every atom. For a `100^3` atomic lattice, turn the aligned
`atom.*` state into one-million-element SoA streams, deposit all atoms into one
shared spatial field, and use fixed-radius cell lists or Verlet lists. Evaluate
pair interactions only within a cutoff and apply Newton's third law through a
race-free gather or explicitly validated atomic scheme.

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

## Density hierarchy and LOD policy

Each hierarchy leaf covers `4^3` voxels and records maximum density and integrated
probability. Detail is selected from maximum density relative to the analytic
peak: LOD 0 is the dense core, LOD 1 and 2 are progressively weaker shells, and
LOD 3 is the diffuse tail. Clusters below the visibility threshold are culled.

For rendering, combine this physical-importance classification with projected
screen error and hysteresis. For simulation, conserve integrated cluster
probability when coarsening. Keep both decisions on the GPU and measure whether
classification saves more downstream work than it costs.

## Required evidence loop

From the repository root:

```sh
cargo run -p pqo-cli -- check baseline/baseline-cuda.pqo --target cuda-headless
cargo run -p pqo-cli -- explain baseline/baseline-cuda.pqo --target cuda-headless
PQO_HEADLESS_TICKS=1 PQO_INSPECT_STREAM=metrics.total_probability \
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

For this baseline, reject a change if the integrated probability materially
departs from `1` or if `radial_moment / total_probability` materially departs
from the analytic result `1.5*a0`. Re-run those invariants whenever field bounds,
resolution, sampling position, density math, or reduction strategy changes.
The graph currently recomputes the field every tick so future atom parameters
may evolve without host intervention; a truly static production scene should
reuse a completed field instead of paying that cost repeatedly.
