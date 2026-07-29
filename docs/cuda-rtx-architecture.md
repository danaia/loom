# Loom CUDA and RTX Architecture

This document defines the CUDA path for Loom's dense particle, volume, geometry,
lighting, and shadow workloads. It is a design target for the future `loom-cuda`
backend, and an authoring guide for agents preparing CUDA-ready Loom graphs.

The current executable runtime remains Metal-first. CUDA support must preserve
the same trust boundary:

```text
Untrusted ModuleGraph
→ validation
→ ValidatedModuleGraph
→ ExecutionPlan
→ loom-cuda lowering
→ CUDA graphs, streams, memory pools, kernels, and RTX rendering
```

CUDA is not a separate semantic language. It is a backend that must prove the
same Loom effects, dependencies, capacities, lifetimes, observations, and artifact
identity as Metal.

## Target Machine

The first CUDA optimization target is a single high-end Ada/Blackwell-class NVIDIA
system such as an RTX 5090 workstation:

- discrete GPU memory, not Apple-style unified memory,
- high device bandwidth but expensive host/device synchronization,
- SIMT execution in 32-lane warps,
- hardware ray tracing cores usable through OptiX or equivalent RTX pipelines,
- asynchronous copy, CUDA streams, events, graphs, and memory pools,
- profiling through Nsight Systems and Nsight Compute.

Every benchmark must record the exact GPU model, VRAM size, driver, CUDA toolkit,
compiler flags, kernel hashes, launch shape, occupancy, memory throughput, copy
traffic, and presentation mode.

## Backend Contract

`Target::Cuda` means simulation kernels require a CUDA implementation. Views may
use either CUDA or OptiX implementations:

```text
target cuda
kernel implementation: cuda
view implementation:   cuda or optix
```

The CUDA backend must lower schedule edges to CUDA events, stream order, explicit
host waits, graph dependencies, or other measured completion mechanisms. A
dependency is satisfied by completion, not by launch submission.

The backend must not:

- read or write a resource absent from a kernel signature,
- perform hidden per-element CPU work,
- silently resize device buffers during steady state,
- use host readback as part of normal simulation or rendering,
- claim cross-device bit identity unless the determinism tier explicitly supports
  it.

## Memory Model

CUDA Loom should treat VRAM as the hot authoritative state.

Use these classes internally:

| Loom intent | CUDA placement |
| --- | --- |
| hot simulation stream | device allocation from a backend memory pool |
| transient scan/sort/bin buffer | reusable arena allocation with validated live range |
| small constants | constant memory or compact device parameter block |
| dynamic counts and indirect arguments | device memory updated by kernels |
| inspection snapshot | asynchronous device-to-pinned-host copy |
| CPU-authored controls | pinned host staging plus async host-to-device copy |

Default to structure-of-arrays for particles, voxels, cells, rays, and mesh
attributes. Fuse into array-of-structs only when a measured kernel consumes the
fields together and improves memory transactions.

Use 32-bit indices until a workload proves it needs larger domains. A billion
explicit elements can still be indexed by `u32`; tens of billions require
partitioned domains, tiled passes, or hierarchical representation rather than one
flat dispatch.

## Dense Particles

The primary particle loop should stay entirely device-resident:

```text
emit or activate
→ hash/bin
→ sort or scatter
→ bounded neighborhood
→ integrate
→ collide
→ compact visible/active
→ render or splat
```

Rules for billion-scale claims:

- **active particles** are resident, dispatchable particles updated this tick,
- **represented particles** may be procedural clusters, bricks, distributions, or
  lower-rate far-field state,
- **visible particles** are the compacted subset sent to raster, splat, or ray
  pipelines.

Do not promise tens of billions of fully resident, full-rate particles on one GPU.
Instead, Loom should make the representation tier explicit:

```text
near field: explicit particles, full-rate
mid field: clustered particles or surfels, lower-rate
far field: volumes, impostors, signed-distance fields, or procedural reservoirs
```

Use bounded spatial data structures: uniform grids, Morton-coded tiles, radix sort,
prefix sums, cell ranges, dense-cell overflow counters, and deterministic overflow
policies where required. Never use production all-pairs interaction.

## Volumes, Clouds, and Fire

Clouds and fire should be represented as sparse bricks plus particles, not as one
monolithic full-resolution grid.

Recommended data plane:

```text
emitters and fuel particles
→ sparse brick activation
→ velocity, density, temperature, fuel fields
→ advection
→ combustion or phase update
→ pressure/projection or approximate divergence control
→ lighting volume preparation
→ raymarch or RTX-assisted render
```

Keep bricks in page-sized 3D tiles, with per-brick active masks and compacted work
queues. Only active bricks dispatch. Far-field cloud detail should become
procedural noise parameters or cached low-frequency volumes rather than resident
fine voxels.

Fire should separate:

- hot scalar fields: density, temperature, fuel, soot,
- velocity fields: staggered or collocated according to solver choice,
- render fields: emission, extinction, scattering coefficients,
- particles: sparks, embers, droplets, high-frequency detail carriers.

## Polys and Geometry

The CUDA backend should support GPU-generated geometry without round-tripping
through the CPU:

```text
particle/field state
→ surface extraction or splat generation
→ compaction
→ device-side draw/ray arguments
→ raster, mesh shader, or OptiX acceleration structure update
```

For dense surfaces, prefer tiles, surfels, meshlets, marching-cubes bricks, or
displacement patches over one giant mutable mesh. If ray tracing is enabled, keep
acceleration structure builds or refits in the render phase budget and report them
separately from simulation.

## Lighting and Shadows

Loom should treat lighting as part of the GPU schedule, not as a postscript.

Use a hybrid path:

- raster or splat for very dense near-pixel particles,
- raymarching for clouds, smoke, fire, and participating media,
- OptiX/RTX for hard geometry, shadow rays, transmittance probes, and selected
  global-illumination samples,
- temporal accumulation and denoising as explicit streams with declared history.

For volumes, shadowing should use cached light-space transmittance, deep opacity
maps, froxel grids, or raymarched cone approximations before brute-force per-pixel
multi-light raymarching. Measure light count, step count, temporal history, and
denoising cost separately.

## Kernel Design

CUDA kernels should be authored around warp-coherent work:

- consecutive threads read consecutive stream elements,
- branch divergence is isolated by sorting or splitting passes,
- hot loops have bounded iteration counts,
- atomics are aggregated per warp or block before global writes,
- shared memory caches cell ranges, brick tiles, or neighbor windows,
- reductions use warp/block primitives before global accumulation,
- persistent kernels are used only when they beat CUDA graph replay in measurement.

Threadgroup choices are target-specific. Loom's ABI may remain `BackendDerived`
until profiling proves a fixed block shape is better. The backend should report
chosen block size, register count, occupancy, limiter, and achieved bandwidth.

## Scheduling

The CUDA runtime should prefer prebuilt CUDA graphs for steady-state schedules:

```text
validated ExecutionPlan
→ capture or instantiate CUDA graph
→ update scalar parameters and dynamic counts
→ launch graph per tick or frame
```

Use multiple CUDA streams for independent phases only when the execution plan
proves no hazards. Simulation, render preparation, readback inspection, and asset
upload should have distinct streams with explicit event dependencies.

Host code should submit ahead, not wait per tick. Readback is a capability event,
not a frame-loop habit.

## Benchmarks

Every CUDA milestone needs paired evidence:

- Loom CUDA path,
- direct CUDA or direct CUDA+OptiX baseline,
- identical buffers, kernels, launch shapes, render resolution, and inputs.

Report at least:

- active, represented, and visible element counts,
- VRAM working set and allocation count,
- GPU time by pass,
- CPU submission time,
- device-host copy bytes,
- achieved bandwidth,
- occupancy and limiting factor,
- sort/scan/bin overflow counters,
- render time, shadow time, denoise time, and acceleration-structure time.

Use the same honesty standard as the Metal proofs: a beautiful result is not a
performance claim until the artifact, device, workload, and measurement are named.

## First Implementation Milestones

1. Add a `loom-cuda` crate that accepts `ValidatedModuleGraph` and rejects graphs
   whose target is not CUDA.
2. Build a headless CUDA runner for Hello Batch with device-private streams,
   async upload, CUDA events, and no presentation.
3. Add CUDA graph replay for the fixed tick schedule.
4. Port spatial binning, radix sort, compaction, and overflow diagnostics.
5. Add a CUDA/OptiX rendered particle or splat view.
6. Add sparse-brick volume storage and a cloud/fire specimen.
7. Add RTX shadow/transmittance passes with explicit temporal history streams.
8. Publish Loom-vs-direct-CUDA benchmarks on the named RTX 5090 system.

