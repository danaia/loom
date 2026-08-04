# Hello CUDA

Portable native Pqo compute with no presentation view. Build the Blackwell
artifacts on Ubuntu with:

```sh
pqo check hello-cuda.pqo --target cuda-headless
pqo build hello-cuda.pqo --target cuda-headless
```

The package contains an `sm_120` cubin and a `compute_120` PTX fallback.
