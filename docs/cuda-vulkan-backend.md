# CUDA/Vulkan backend

This document is the implementation status for the Linux/NVIDIA backend. It is
an evidence record, not a claim that all renderer gates are complete.

## Supported today

- Portable, Metal, CUDA/Vulkan, and CUDA-headless target policies.
- Target-specific validation and complete implementation-set selection.
- CUDA C++ generation for native element-wise Pqo kernels.
- Native `sm_120` cubin and `compute_120` PTX packaging with artifact metadata.
- CUDA Driver API execution using asynchronous allocation and instantiated CUDA
  Graphs, with device-resident dynamic counts.
- Adaptive VRAM budgets and development, benchmark, and production telemetry
  modes.
- Resource domains and backend-plan checks for private, shared-presentation, and
  bounded host-control resources.
- Explicit ABI layout generation for packed streams and structured view data.
- Logical simulation/projection plans and immutable published-version leases.
- Vulkan 1.3 capability probing, mandatory CUDA/Vulkan UUID matching, exportable
  device-local buffers, opaque-FD CUDA imports, and imported timeline semaphore
  signaling.
- A validated two-to-four-slot presentation ring model with monotonic timeline
  generations.
- A standalone native X11/Wayland Vulkan 1.3 window, CUDA-UUID device
  selection, swapchain, synchronization2 barriers, and dynamic-rendering frame
  submission. `pqo run --target cuda-vulkan` now uses this path and does not
  launch the Tauri/WebGL panel.
- Reusable cubic world-hierarchy planning plus CUDA leaf-cluster occupancy,
  visibility, LOD classification, and active-LOD counters for the crystal.

## Not complete yet

The live CUDA-backed crystal draw pipeline, CUDA-generated indirect draw path,
full presentation-ring scheduler, swapchain recovery, and end-to-end
device-loss recovery have not been implemented. The native swapchain currently
uses an embedded procedural crystal shader and is therefore a backend bring-up
surface, not yet the production shared-field renderer. CUDA headless execution
remains the production-usable Linux compute path in this revision.

The ABI module currently computes and records layouts; emitting Rust, CUDA, and
GLSL declarations from every schema is a later gate. The headless runtime also
supports explicit stream initializers only. Dynamic population algorithms,
CUB-backed semantic primitives, reproducible multi-candidate autotuning, and
large workload fixtures remain future gates.

## Build and run

CUDA 12.8 or newer is required for Blackwell `sm_120`. CUDA 13.x is preferred.
Vulkan is needed only for `cuda-vulkan` builds and interoperability checks.

```sh
cargo run -p pqo-cli -- check examples/hello-cuda/hello-cuda.pqo \
  --target cuda-headless
cargo run -p pqo-cli -- build examples/hello-cuda/hello-cuda.pqo \
  --target cuda-headless
PQO_HEADLESS_TICKS=120 cargo run -p pqo-cli -- \
  run examples/hello-cuda/hello-cuda.pqo --target cuda-headless
```

Build the CUDA plus SPIR-V package:

```sh
cargo run -p pqo-cli -- build \
  examples/hello-cuda-vulkan/hello-cuda-vulkan.pqo \
  --target cuda-vulkan
```

Exercise UUID matching, external memory import, CUDA memory access, and the
cross-API timeline signal:

```sh
cargo run -p pqo-vulkan --example probe
```

Benchmark mode uses preallocated CUDA events and delayed collection:

```sh
PQO_TELEMETRY=benchmark \
PQO_WARMUP_TICKS=120 \
PQO_HEADLESS_TICKS=1200 \
cargo run --release -p pqo-cli -- \
  run examples/hello-cuda/hello-cuda.pqo --target cuda-headless
```

The report includes p50, p95, p99, maximum tick duration, missed 8.333 ms
deadlines, memory observations, target artifact path, and semantic, artifact,
backend, and hardware fingerprints. Benchmark evidence is valid only with its
reported telemetry and competing-memory environment.

## Runtime controls

- `PQO_HEADLESS_TICKS`: logical ticks to launch.
- `PQO_WARMUP_TICKS`: unmeasured warm-up launches.
- `PQO_TELEMETRY`: `development`, `benchmark`, or `production`.
- `PQO_RESERVE_VRAM_GIB`: VRAM headroom reserved outside the Pqo budget.
- `PQO_SHUTDOWN_TIMEOUT_MS`: owned-stream drain deadline.
- `PQO_INSPECT_STREAM`: explicit post-run inspection of one stream.

No steady-state path performs population-scaled CPU traversal, allocation,
compilation, graph instantiation, full-state readback, or full-device waiting.
Inspection is an explicit post-run operation.

## Gate status

| Gate | Status |
| --- | --- |
| 1. Target and capability model | Complete |
| 2. Implementation selection and manifests | Complete |
| 3. Cross-language ABI | Layout contract complete; source emission pending |
| 4. CUDA artifact generation | Complete for native and external CUDA C++ |
| 5. Headless CUDA correctness | Complete for explicit-initializer element kernels |
| 6. Headless performance/logical plans | Initial graph runtime and telemetry complete |
| 7. Standalone Vulkan renderer | Native swapchain and procedural crystal pipeline complete; graph-driven pipelines pending |
| 8. External memory and UUID matching | Probe complete |
| 9. Timelines and state leases | Models/probe complete; integrated scheduler pending |
| 10. Indirect Hello Particle presentation | Pending |
| 11. Packaging/lifecycle/telemetry | Packaging and telemetry complete; recovery partial |
| 12. Dynamic populations and reproducible tuning | Pending |

The Metal tests remain part of the existing validation suite. The Metal runtime
itself must be compiled and exercised on macOS because it links Apple
frameworks unavailable on Ubuntu.
