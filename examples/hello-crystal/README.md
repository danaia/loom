# Hello Crystal

[`crystal.loom`](crystal.loom) is a directly runnable Loom-language program for a
32³ mesoscopic crystal. One element is a material volume, not an atom.

Update Loom first:

```text
loom update
loom --version
```

## Run from this directory

```text
cd examples/hello-crystal
loom crystal.loom
```

The explicit form does the same thing:

```text
loom run crystal.loom
```

## Run from the repository root

```text
loom examples/hello-crystal/crystal.loom
```

## Run the installed copy

The curl installer places a copy that can be launched from any directory:

```text
loom ~/.loom/examples/crystal.loom
```

## Check and explain

These commands do not open the Metal window:

```text
loom check crystal.loom
loom explain crystal.loom
```

If an older interactive shell still selects a previous Loom command after
upgrading, run `rehash` in zsh or `hash -r` in bash.

## Configurable development runner

`loom_core::hello_crystal_builder(cell_count)` remains the configurable,
parser-independent graph builder used for larger development and benchmark runs.

The default demonstration uses `100³ = 1,000,000` cells and includes:

- solute and temperature diffusion,
- phase-field solidification from one seed,
- an orientation-dependent cubic growth law and rotated Wulff-like material envelope,
- latent-heat release and solute consumption,
- zero autonomous damage,
- slicing, healing, orbit, and zoom through explicit Loom interventions,
- iterative connected-component labels,
- gravity and independent motion for detached material,
- GPU-reduced morphology, material, slice, and damage metrics,
- normal-aware faceted surface extraction into a dedicated isometric renderer.

Run the configurable builder on Metal:

```text
./scripts/run-hello-particle.sh crystal 1m
```

Controls:

- Left-drag on the crystal to slice it.
- Left-drag on the black background to orbit it.
- Scroll or use a trackpad gesture to zoom in and out.

Every stroke is projected into the simulation using the current camera transform.
Nothing damages the crystal until you slice it. Once cut, the crystal immediately
begins self-healing: damage decays while displaced fragments are pulled back
toward their lattice positions, and the seam closes automatically.

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
