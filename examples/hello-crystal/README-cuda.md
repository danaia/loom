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

Drag across the crystal to make a cleavage cut; drag the background to orbit,
and scroll to zoom. The specimen grows toward the selected growth target, and
cuts visibly heal after a short interval. The panel controls growth,
anisotropy, temperature, visible damage, and whether to render the continuous
field, its particle-cell representation, or both. The particle-count slider
controls display density (10,000 to 1,000,000); the CUDA simulation remains a
fixed 100³ field. The current desktop renderer is
Tauri/WebGL; zero-copy Vulkan presentation remains the next backend step.
