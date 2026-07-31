# Hello Crystal

[`crystal.pqo`](crystal.pqo) is a directly runnable Pqo-language program for a
one-million-cell (`100³`) interactive mesoscopic crystal. It grows on the GPU,
can be sliced with the mouse, and heals itself. One element is a material volume,
not an atom.

Update Pqo first:

```text
pqo update
pqo --version
```

## Run from this directory

```text
cd examples/hello-crystal
pqo crystal.pqo
```

The explicit form does the same thing:

```text
pqo run crystal.pqo
```

## Run from the repository root

```text
pqo examples/hello-crystal/crystal.pqo
```

## Run the installed copy

The curl installer places a copy that can be launched from any directory:

```text
pqo ~/.pqo/examples/crystal.pqo
```

## Controls

- Left-drag on the crystal to slice it.
- Left-drag on the black background to spin the crystal.
- Scroll or use a trackpad gesture to zoom.

Nothing damages the crystal until you slice it. A cut removes material and moves
the separated fragments. The exposed cut glows red while damage is active.
Damage then decays, fragments return toward their lattice positions, and the
red seam closes automatically.

## Check and explain

These commands do not open the Metal window:

```text
pqo check crystal.pqo
pqo explain crystal.pqo
```

If an older interactive shell still selects a previous Pqo command after
upgrading, run `rehash` in zsh or `hash -r` in bash.

## Configurable development runner

`pqo_core::hello_crystal_builder(cell_count)` remains the configurable,
parser-independent graph builder used for development and benchmark runs.

The language example and default builder demonstration use
`100³ = 1,000,000` cells and include:

- solute and temperature diffusion,
- phase-field solidification from one seed,
- an orientation-dependent cubic growth law and rotated Wulff-like material envelope,
- latent-heat release and solute consumption,
- zero autonomous damage,
- slicing, healing, orbit, and zoom through explicit Pqo interventions,
- iterative connected-component labels,
- gravity and independent motion for detached material,
- GPU-reduced morphology, material, slice, and damage metrics,
- normal-aware faceted surface extraction into a dedicated isometric renderer.

Run the configurable builder on Metal:

```text
./scripts/run-hello-particle.sh crystal 1m
```

Every stroke is projected into the simulation using the current camera transform.

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
