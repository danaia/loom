# Water molecule

The first molecular Pqo gate: one rigid three-site H₂O molecule with a
0.09572 nm O–H distance and a 104.52° H–O–H angle.

The molecule center and quaternion are authoritative GPU state. CUDA
reconstructs three atom positions, two bond endpoint pairs, partial charges,
and geometry diagnostics. The electron-density/orbital layer is deliberately
absent: ordinary molecular geometry must not depend on isolated atomic orbital
fields.

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

Drag to orbit and scroll to zoom. Oxygen is red, hydrogen is white, the two
covalent bonds are derived from the rigid template, and the charge glow and
dipole arrow communicate the model's polarity.

This is Gate 1 only. It does not yet integrate rigid-body motion, evaluate
intermolecular forces, classify hydrogen bonds, or simulate liquid water. The
current Linux renderer also selects a dedicated embedded shader for the water
contract; direct CUDA-to-Vulkan presentation-buffer binding remains a later
interop gate.
