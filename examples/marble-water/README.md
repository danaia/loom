# Marble Water

A keyboard-controlled Loom/Metal marble experiment with eight GPU-simulated hunters and an adjustable water-particle surface.

- WASD or arrow keys steer the yellow marble.
- Left-drag outside the HUD to move the yellow marble across the plane in X/Z.
- Scroll while holding the marble to raise or lower it, then release to drop it into the water.
- Use the separate Vue/Tauri panel to vary the active water grid from 3,072 to 30,704 particles.
- Grow the simulated plane from 1× to 3× in the panel.
- Amplify wakes and splashes from 1× to 6× in the panel.
- Reset every marble and calm the water from the panel.
- The panel reports measured presentation FPS and current Metal allocations in MiB.
- Red enemy marbles slowly chase the player.
- Every marble collides elastically with every other marble and transfers collision energy into the water.
- Passing crests and troughs lift every marble, while local surface gradients push their horizontal motion.
- Drag, gravity, landing detection, speed limits, surface constraints, and bounded movement run on Metal.
- Marbles stay on top of the enlarged water plane and continuously emit wakes from their horizontal motion.
- Up to 30,704 GPU-resident water particles solve a spacing-aware 2D shallow-water wave equation with an isotropic nine-point stencil.
- A dropped marble transfers momentum through a volume-balanced crater-and-rim impulse, producing a crest/trough pair that expands from the exact contact point.
- The outer particle band absorbs outgoing energy to prevent square boundary echoes, while moving marbles still produce gentler continuous wakes.
- Higher amplification increases impact energy and wave speed while reducing damping for larger, longer-lived ripples.
- Rendering reads the staged GPU state directly; no CPU-side particle model exists.

Build the self-contained Loom package:

```text
loom build examples/marble-water/marble-water.loom
loom check examples/marble-water/marble-water.lmp
loom examples/marble-water/marble-water.lmp
```

`marble-water.lmp` is the distributable project. It contains the primary Loom
graph, both Metal sources, `src/runtime.rs`, the Vue 3 panel source and built
assets, and the compiled Rust extension for the build machine's target. The
installed global `loom` runtime and its generic Tauri panel shell are its only
external dependencies.
