# Water — from glass to molecule

An interactive multiscale water scene. Start with a real-looking glass of
room-temperature liquid water, disturb its surface with the mouse, then scroll
continuously down to one rigid three-site H₂O molecule with a 0.09572 nm O–H
distance and a 104.52° H–O–H angle.

At cup scale, water is represented as a continuum surface with the bulk
properties of water at 298.15 K: density 997 kg/m³, dynamic viscosity
0.890 mPa·s, surface tension 71.97 mN/m, and Earth gravity. Pointer impulses
produce damped capillary-gravity ripples, crown splashes, and ballistic droplets.

At molecular scale, the molecule center and quaternion are authoritative GPU state. CUDA
reconstructs three atom positions, two bond endpoint pairs, partial charges,
and geometry diagnostics. The electron-density/orbital layer is deliberately
absent: ordinary molecular geometry must not depend on isolated atomic orbital
fields.

The glass represents approximately 8 × 10²⁴ molecules as conserved metadata.
It does **not** instantiate them individually. Zoom is an adaptive change of
representation: continuum liquid → coarse molecular context → one explicitly
resolved molecule.

Run the headless scientific check:

```sh
pqo check examples/water-molecule/water-molecule.pqo --target cuda-headless
PQO_HEADLESS_TICKS=1 PQO_INSPECT_STREAM=metrics.geometry_error \
  pqo run examples/water-molecule/water-molecule.pqo --target cuda-headless
```

Open the native CUDA/Vulkan view:

```sh
cargo run -p pqo-cli -- run examples/water-molecule/water-molecule.pqo \
  --target cuda-vulkan
```

Controls:

- Drag on the water to create a local ripple and splash.
- Drag the background to orbit the scene.
- Scroll to cross continuously between glass and molecular scales.
- Use the Sphere Drop panel to choose 1–5 identical 3 cm spheres, set each
  sphere's mass from 2–120 g, and drop them into the glass. A sphere near
  14.1 g is neutrally buoyant; lighter spheres float and heavier spheres sink.

Oxygen is red, hydrogen is white, and the two covalent bonds are derived from
the rigid template.

The macroscopic interaction is a real-time capillary-wave presentation model,
not molecular dynamics. It uses physically grounded parameters but does not
solve Navier–Stokes or evaluate all intermolecular forces. The current Linux
renderer selects a dedicated embedded shader for the water contract; direct
CUDA-to-Vulkan presentation-buffer binding remains a later interop gate.

The visual target used for this implementation is in
[`design/water-glass-concept.png`](design/water-glass-concept.png).
