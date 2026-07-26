# Hello Crystal

`loom_core::hello_crystal_builder(cell_count)` builds a runnable, parser-independent
Loom graph for a mesoscopic 3D crystal. One element is a material volume, not an
atom.

The default demonstration uses `100³ = 1,000,000` cells and includes:

- solute and temperature diffusion,
- phase-field solidification from one seed,
- an orientation-dependent cubic growth law and rotated Wulff-like material envelope,
- latent-heat release and solute consumption,
- zero autonomous damage,
- mouse-drag slicing through explicit Loom interventions,
- iterative connected-component labels,
- gravity and independent motion for detached material,
- GPU-reduced morphology, material, slice, and damage metrics,
- normal-aware faceted surface extraction into a dedicated isometric renderer.

Run it on Metal:

```text
./scripts/run-hello-particle.sh crystal 1m
```

Hold the left mouse button and drag across the crystal. Every drag segment is
projected into the simulation, removes the intersected material, and lets the
connected-component and fragment passes decide what separates and falls. Nothing
damages the crystal until you do this.

For a faster development run, use a smaller perfect cube:

```text
./scripts/run-hello-particle.sh crystal 262144
./scripts/run-hello-particle.sh crystal 32768 --bench headless --samples 300
```

Those counts are `64³` and `32³`. Growth is accelerated so the morphology is
visible quickly. This remains a physically motivated demonstration rather than a
calibrated predictor for a specific material. It renders normal-aware surface
cells directly; feature-preserving dual contouring and a full MPM stress transfer
are the next fidelity steps.
