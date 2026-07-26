# Hello Crystal

`loom_core::hello_crystal_builder(cell_count)` builds a runnable, parser-independent
Loom graph for a mesoscopic 3D crystal. One element is a material volume, not an
atom.

The default demonstration uses `100³ = 1,000,000` cells and includes:

- solute and temperature diffusion,
- phase-field solidification from one seed,
- an orientation-dependent cubic growth law,
- latent-heat release and solute consumption,
- an accelerated impact at tick 240,
- stress-driven damage on a preferred cleavage plane,
- iterative connected-component labels,
- gravity and independent motion for detached material,
- GPU-reduced morphology, material, stress, and damage metrics,
- sparse surface extraction into a dedicated isometric renderer.

Run it on Metal:

```text
./scripts/run-hello-particle.sh crystal 1m
```

For a faster development run, use a smaller perfect cube:

```text
./scripts/run-hello-particle.sh crystal 262144
./scripts/run-hello-particle.sh crystal 32768 --bench headless --samples 300
```

Those counts are `64³` and `32³`. The first slice deliberately accelerates both
growth and impact so the complete causal sequence is visible in minutes. It is a
physically motivated demonstration, not yet a calibrated predictor for a specific
material. It also renders surface cells directly; feature-preserving dual
contouring and a full MPM stress transfer are the next fidelity steps.
