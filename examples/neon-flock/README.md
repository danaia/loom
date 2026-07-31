# Neon Flock

Neon Flock asks whether a bounded GPU population can visibly cohere while
local separation preserves individual agents. The same graph maintains a
lagged position per agent, producing a subtle color trail behind each luminous
head.

## Run from the repository root

Build the repository CLI so its embedded Metal package includes this example's
external kernels:

```text
cargo build --package pqo-cli
export PATH="$PWD/target/debug:$PATH"
pqo check examples/neon-flock/neon-flock.pqo
pqo explain examples/neon-flock/neon-flock.pqo
pqo examples/neon-flock/neon-flock.pqo
```

## Run from this directory

Build the repository CLI and put that build first in `PATH`:

```text
cd examples/neon-flock
cargo build --manifest-path ../../Cargo.toml --package pqo-cli
export PATH="$(cd ../.. && pwd)/target/debug:$PATH"
which pqo
pqo check neon-flock.pqo
pqo explain neon-flock.pqo
pqo neon-flock.pqo
```

`which pqo` must print a path ending in `/dev/pqo/target/debug/pqo`.
An older installed `~/.pqo/bin/pqo` reports the same language version but
does not embed this checkout's new Metal sources; using it produces
`no packaged Metal source for kernels/neon_flock.metal`.

Close the Metal window to stop the program. `pqo run neon-flock.pqo` is the
explicit equivalent of `pqo neon-flock.pqo`.

## Kernel boundary

Every implementation boundary is explicit in `neon-flock.pqo`:

- **Native Pqo — `advance_agents`:** element-wise `f32x2` acceleration,
  damping, velocity update, and position integration.
- **Native Pqo — `evolve_trails`:** element-wise exponential lag of each
  agent's trail position.
- **External Metal — `flock_neighborhood`:** all-agent neighborhood reads,
  deterministic central seeding, conditional cohesion/alignment/separation,
  soft boundary response, deterministic wander, and steering limits. Source:
  `kernels/neon_flock.metal`.
- **External Metal render — `neon_view`:** oriented trail geometry,
  procedural color, luminous falloff, and additive composition. Source:
  `shaders/neon_flock.metal`.

Neighborhood access, conditionals, speed clamps, and render stages are outside
Pqo 0.1's current native compiler gates, so they deliberately remain visible
`extern metal` declarations. The `.pqo` file remains the source of truth for
population size, physical units, stream authority, constants, bindings, pass
order, and the native/external boundary.
