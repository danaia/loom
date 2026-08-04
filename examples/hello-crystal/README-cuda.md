# Hello Crystal — CUDA

`crystal-cuda.pqo` is the Linux/NVIDIA counterpart of the interactive Metal
specimen. It evolves a 100³ crystal field on CUDA, then presents the crystal in
a native Vulkan 1.3 window on the same physical GPU. The Vulkan path contains
no browser or WebGL compositor. A separate Tauri control window sends growth,
anisotropy, temperature, damage, field/particle visibility, and particle-count
updates to the native Vulkan render loop.

Left-drag directly in the Vulkan window to orbit the crystal. Use the mouse
wheel or a trackpad scroll gesture to zoom. The Tauri panel also provides orbit,
zoom, and reset-view buttons for precise camera adjustments.

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
