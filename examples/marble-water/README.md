# Marble Water

A keyboard-controlled Loom/Metal marble experiment with eight GPU-simulated hunters and an adjustable water-particle surface.

- WASD or arrow keys steer the yellow marble.
- Left-drag outside the HUD to move the yellow marble across the plane in X/Z.
- Scroll while holding the marble to raise or lower it, then release to drop it into the water.
- Drag the cyan HUD slider to vary the active water grid from 3,072 to 30,704 particles.
- Drag the purple HUD slider to grow the simulated plane from 1× to 3×.
- Click the HUD reset button to restore every marble and calm the water.
- The HUD reports measured presentation FPS and current Metal allocations in MiB.
- Red enemy marbles slowly chase the player.
- Drag, gravity, landing detection, speed limits, surface constraints, and bounded movement run on Metal.
- Marbles stay on top of the enlarged water plane and continuously emit wakes from their horizontal motion.
- Up to 30,704 spring-coupled water particles consume those wakes and propagate damped waves to their four neighbors.
- Rendering reads the staged GPU state directly; no CPU-side particle model exists.

From the repository root:

```text
loom check examples/marble-water/marble-water.loom
loom explain examples/marble-water/marble-water.loom
loom examples/marble-water/marble-water.loom
```
