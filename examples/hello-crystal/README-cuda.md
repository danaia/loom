# Hello Crystal — CUDA

`crystal-cuda.pqo` is the Linux/NVIDIA counterpart of the interactive Metal
specimen. It evolves a 100³ crystal field on CUDA, then presents the crystal in
a native Vulkan 1.3 window on the same physical GPU. The Vulkan path contains
no browser or WebGL compositor. A separate Tauri control window sends growth,
anisotropy, temperature, damage, field/particle visibility, and particle-count
updates to the native Vulkan render loop.

The Crystal instances stress-test slider expands the specimen into an exact
1–1,000-instance evenly spaced square grid. Vulkan evaluates compact grid
blocks in the scene shader rather than issuing a separate draw call per copy.

Left-drag directly in the Vulkan window to orbit the crystal. Use the mouse
wheel or a trackpad scroll gesture to zoom. The Tauri panel also provides orbit,
zoom, and reset-view buttons for precise camera adjustments.

The CUDA simulation also builds a GPU-resident spatial hierarchy every tick:

- 15,625 leaf clusters covering 4³ cells each,
- occupancy and visibility metadata in shared-presentation streams,
- GPU-selected detail classes and active counts,
- six reusable hierarchy levels in the engine plan,
- automatic distance LOD with a user-adjustable detail bias.

The hierarchy represents one million cells with under 256 KiB of planning
metadata. The same planner bounds the hierarchy metadata for a cubic
one-billion-element world to under 36 MiB when using 8³ leaf clusters.

Run it on Linux with CUDA 12.8+ and an NVIDIA Blackwell-compatible driver:

```sh
pqo crystal-cuda.pqo
```

Inspect the crystallized field after execution:

```sh
PQO_HEADLESS_TICKS=240 PQO_INSPECT_STREAM=field.phase \
  pqo run crystal-cuda.pqo --target cuda-headless
```

The current native window is the first Vulkan swapchain and crystal-pipeline
vertical slice. CUDA/Vulkan external-memory and timeline-semaphore exchange is
already proven by `pqo-vulkan`; wiring the live field into the draw pipeline,
restoring direct cut/orbit gestures on the Vulkan window, and GPU-generated
indirect particle drawing are the next integration gates. The CUDA simulation
remains a fixed 100³ field.
