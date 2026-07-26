# Loom Native Compiler Gates

Status: working roadmap  
North star: expand native Loom through small, measured kernel classes before
attempting to rewrite Hello Crystal.

Loom has crossed its first native kernel gate: `integrate` in Hello Particle is
authored in Loom, lowered into the canonical typed graph, generated as Metal,
compiled, executed on Apple GPU hardware, and rendered in a native application.
Ground contact and rendering remain explicit Metal escape hatches.

The next work is coverage, not spectacle. Each gate should add one coherent class
of GPU behavior, prove it against a handwritten Metal reference, and leave the
compiler more trustworthy than it found it.

## Gate discipline

A gate is complete only when it has all of the following:

1. **Source semantics** — the supported syntax, types, units, effects, and failure
   cases are documented.
2. **Canonical lowering** — source lowers deterministically into the existing
   typed graph; no backend-only meaning is hidden in the parser.
3. **Validation** — unsafe access, illegal aliasing, unsupported shapes, and
   authority violations fail before artifact identity is issued.
4. **Metal generation** — generated source has stable bindings and inspectable
   provenance under `loom://generated/...`.
5. **Differential proof** — the native Loom implementation and the packaged Metal
   reference receive the same initialized state and produce the same logical result
   within an explicit numeric tolerance.
6. **Hardware proof** — the generated pipeline compiles and executes on a real
   Apple GPU in CI or a recorded local gate.
7. **Performance evidence** — warm-up, sample count, workload size, GPU time,
   host overhead, and generated-versus-reference delta are recorded. Compiling is
   not evidence of speed.
8. **Honest fallback** — unsupported operations stay visibly `extern metal`;
   the compiler never silently changes implementation class.

Every gate should retain:

```text
same logical result
+ same bindings and resource effects
+ same determinism tier
+ same or explicitly bounded numeric behavior
+ measured performance delta
```

## Recommended sequence

### Gate 2 — Element-wise arithmetic and conditional updates

**Purpose:** complete the basic “one invocation owns one element” kernel class.

- Add comparisons, boolean expressions, `if`/`else`, scalar literals, component
  access, local `let` values, and selected math intrinsics.
- Keep writes restricted to the invocation’s own indexed element.
- Reject divergent resource effects and out-of-bounds index construction.
- Native specimen: replace `contact_ground` in Hello Particle.
- Differential oracle: current `kernels/ground_contact.metal`.
- Exit proof: Hello Particle compute is entirely native Loom; rendering remains
  the only external stage.

### Gate 3 — Field sampling and deposits

**Purpose:** let agents read a spatial field and contribute bounded signals.

- Add typed 2D/3D field resources, coordinate spaces, nearest/linear sampling,
  boundary modes, and explicit deposit operations.
- Separate read-only sampling from write/accumulate effects in the graph.
- Begin with one deterministic deposit strategy; do not imply deterministic
  floating-point atomics where the backend cannot provide them.
- Native specimen: particles sample gravity/attraction and deposit density into
  a small field.
- Exit proof: native and reference field snapshots agree at fixed checkpoints,
  including edges and empty cells.

### Gate 4 — Ping-pong stencil evolution

**Purpose:** express repeated field evolution without hidden in-place hazards.

- Add neighborhood offsets over a read field and an explicitly distinct write
  field.
- Represent ping-pong role swapping in the execution plan, not as source-level
  buffer-index arithmetic.
- Validate radius, boundary policy, dispatch extent, and read/write separation.
- Native specimen: diffusion or reaction-diffusion over a fixed grid.
- Exit proof: multi-tick parity, boundary behavior, and buffer-role swaps match
  the Metal oracle.

### Gate 5 — Atomics and bounded reductions

**Purpose:** support contested updates while making nondeterminism and bounds
visible.

- Add integer atomics first, then explicitly supported floating-point accumulation.
- Add bounded workgroup and whole-dispatch reductions with declared identities.
- Encode determinism tier and numeric tolerance in validation and artifacts.
- Native specimen: occupancy histogram plus min/max/count reduction.
- Exit proof: no lost updates, correct empty-input behavior, overflow diagnostics,
  and measured contention scaling.

### Gate 6 — Neighborhood access

**Purpose:** let each agent inspect a bounded local population rather than only
its own element.

- Add read-only indexed neighbor access through a declared spatial index or
  bounded adjacency range.
- Require maximum neighborhood work, valid index provenance, and explicit
  behavior for truncated neighborhoods.
- Preserve ownership: an invocation may observe neighbors but may only mutate
  its own state or use a declared contested-write primitive.
- Native specimen: separation/alignment/cohesion over a compact neighbor list.
- Exit proof: reference parity across empty, sparse, dense, and truncated cases;
  report work amplification and index-build cost separately.

### Gate 7 — Scans and compaction

**Purpose:** support dynamic populations without host-side list rebuilding.

- Introduce library-backed scan primitives before general user-authored barriers.
- Add predicate masks, exclusive/inclusive scan, scatter, compacted length, and
  capacity/overflow policy.
- Treat logical length updates as validated state transitions.
- Native specimen: filter dead particles and emit a compact active stream.
- Exit proof: stable ordering where promised, correct zero/full/overflow cases,
  and no steady-state CPU readback.

### Gate 8 — Component relaxation

**Purpose:** cover the iterative connectivity class used by fracture and organism
analysis.

- Add bounded iterative dispatch or an explicit convergence loop with a maximum
  iteration count.
- Model label reads/writes as ping-pong state and convergence as a reduction.
- Require termination policy, observation cadence, and determinism tier.
- Native specimen: connected-component label relaxation on a small occupied grid.
- Exit proof: labels match the reference up to canonical relabeling; disconnected,
  bridge, split, and iteration-limit fixtures are included.

### Gate 9 — Render preparation

**Purpose:** generate GPU-visible draw data from simulation state without making
render stages native yet.

- Add packing/casting, culling predicates, indirect draw arguments, and instance
  compaction.
- Validate render ABI shape, alignment, capacity, and synchronization with the
  consuming view.
- Native specimen: particle cull + instance preparation feeding the existing
  external particle renderer.
- Exit proof: draw count and visible instance data match the Metal reference with
  zero steady-state CPU copies.

### Gate 10 — Native vertex and fragment stages

**Purpose:** make a complete visible application authorable in Loom.

- Extend implementation kinds beyond compute while retaining the same typed
  resource, effect, and provenance model.
- Add stage inputs/outputs, interpolation qualifiers, render targets, depth/blend
  state, and a constrained intrinsic set.
- Keep render pipeline configuration explicit and validated.
- Native specimen: replace `shaders/particle.metal` with Loom-authored vertex and
  fragment stages.
- Exit proof: Hello Particle contains no external Metal, generated render
  pipelines are fingerprinted, and image comparison stays within a declared
  tolerance.

## Delivery shape for each gate

Each gate should land as a narrow vertical slice:

```text
grammar + semantic model
→ canonical graph additions
→ validation diagnostics and repairs
→ Metal lowering
→ generated-source snapshot
→ CPU/reference fixtures where useful
→ real-Metal differential test
→ benchmark record
→ handbook example
```

Keep at least one invalid fixture beside every valid fixture. Diagnostics should
identify the source construct, violated rule, and a repair an agent can apply
atomically when the repair is unambiguous.

## Immediate work queue

1. Freeze Gate 2’s supported expression and control-flow subset.
2. Convert `contact_ground` into the first conditional native Loom fixture while
   preserving the external implementation as the oracle.
3. Add component access and the minimum intrinsic set required by contact.
4. Add source-span diagnostics for illegal conditional writes and unit mistakes.
5. Run native/reference output comparison across restitution, friction, penetration,
   and no-contact cases.
6. Record generated/reference Metal timing at 1K, 100K, and 1M particles.
7. Promote the native implementation only after correctness and performance gates
   pass; keep the external declaration available as an explicit compatibility path.

Hello Crystal should remain unchanged until the smaller gates cover the kernel
classes it needs. At that point, migrate one pass at a time and keep every current
Metal pass as a differential oracle until the native replacement earns removal.
