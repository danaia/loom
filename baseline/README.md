# Loom baseline

A deliberately minimal, complete Loom project: one GPU-resident particle in
zero gravity. Drag anywhere in the viewer to pull the particle through space,
scroll while dragging to change its depth, then release it to preserve its
inertia.

The project keeps the full application path ready for extension:

- a typed Loom graph running at 120 Hz
- a Metal physics/projection kernel
- a Metal particle shader
- a native Rust input extension
- a Vue control panel
- FPS, GPU memory, GPU frame-time, budget, and pressure telemetry
- a portable `.lmp` build

From the repository root:

```sh
loom check baseline/baseline.loom
loom build baseline/baseline.loom
loom baseline/baseline.lmp
```

`Space drag` defaults to zero, so the released particle follows ideal
zero-gravity inertial motion. The viewer wraps its position at the edges to
represent unbounded space without introducing collision forces.
