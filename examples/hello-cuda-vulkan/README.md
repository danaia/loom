# Hello CUDA + Vulkan

This target-restricted specimen proves CUDA artifact generation and Vulkan GLSL
to SPIR-V packaging:

```sh
pqo check hello-cuda-vulkan.pqo --target cuda-vulkan
pqo build hello-cuda-vulkan.pqo --target cuda-vulkan
```

The Vulkan runtime currently provides UUID, feature, external-memory, and
timeline-semaphore probes. Swapchain presentation is the next renderer gate.
