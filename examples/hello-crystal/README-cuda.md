# Hello Crystal — CUDA

`crystal-cuda.pqo` is the Linux/NVIDIA counterpart of the interactive Metal
specimen. It evolves a 100³ crystal field on CUDA, then opens a Rust/Tauri
desktop environment with an interactive WebGL crystal and live controls.

Run it on Linux with CUDA 12.8+ and an NVIDIA Blackwell-compatible driver:

```sh
pqo crystal-cuda.pqo
```

Inspect the crystallized field after execution:

```sh
PQO_HEADLESS_TICKS=240 PQO_INSPECT_STREAM=field.phase \
  pqo run crystal-cuda.pqo --target cuda-headless
```

Drag the crystal to orbit and scroll to zoom. The panel controls growth,
anisotropy, temperature, and visible damage. The current desktop renderer is
Tauri/WebGL; zero-copy Vulkan presentation remains the next backend step.
