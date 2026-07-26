# Neon Flock

Neon Flock asks whether a bounded GPU population can visibly cohere while
local separation preserves individual agents. The same graph maintains a
lagged position per agent, producing a subtle color trail behind each luminous
head.

## Run from the repository root

Build the repository CLI so its embedded Metal package includes this example's
external kernels:

```text
cargo build --package loom-cli
export PATH="$PWD/target/debug:$PATH"
loom check examples/neon-flock/neon-flock.loom
loom explain examples/neon-flock/neon-flock.loom
loom examples/neon-flock/neon-flock.loom
```

## Run from this directory

Build the repository CLI and put that build first in `PATH`:

```text
cd examples/neon-flock
cargo build --manifest-path ../../Cargo.toml --package loom-cli
export PATH="$(cd ../.. && pwd)/target/debug:$PATH"
which loom
loom check neon-flock.loom
loom explain neon-flock.loom
loom neon-flock.loom
```

`which loom` must print a path ending in `/dev/loom/target/debug/loom`.
An older installed `~/.loom/bin/loom` reports the same language version but
does not embed this checkout's new Metal sources; using it produces
`no packaged Metal source for kernels/neon_flock.metal`.

Close the Metal window to stop the program. `loom run neon-flock.loom` is the
explicit equivalent of `loom neon-flock.loom`.

## Kernel boundary

Every implementation boundary is explicit in `neon-flock.loom`:

- **Native Loom — `advance_agents`:** element-wise `f32x2` acceleration,
  damping, velocity update, and position integration.
- **Native Loom — `evolve_trails`:** element-wise exponential lag of each
  agent's trail position.
- **External Metal — `flock_neighborhood`:** all-agent neighborhood reads,
  deterministic central seeding, conditional cohesion/alignment/separation,
  soft boundary response, deterministic wander, and steering limits. Source:
  `kernels/neon_flock.metal`.
- **External Metal render — `neon_view`:** oriented trail geometry,
  procedural color, luminous falloff, and additive composition. Source:
  `shaders/neon_flock.metal`.

Neighborhood access, conditionals, speed clamps, and render stages are outside
Loom 0.1's current native compiler gates, so they deliberately remain visible
`extern metal` declarations. The `.loom` file remains the source of truth for
population size, physical units, stream authority, constants, bindings, pass
order, and the native/external boundary.
