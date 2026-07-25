# Loom — Agent-Managed Bare-Metal 3D Physics Engine

## North Star

- Create the lowest practical software layer for agent-authored 3D space and physics
- Treat particles as the fundamental state atoms of a physical world
- Compile Loom programs into native ARM64 host code and Metal GPU kernels
- Exploit Apple Silicon GPU compute, unified memory, SIMD execution, and low-overhead Metal dispatch
- Keep humans out of the hot authoring loop without removing human auditability
- Provide a Tauri-based window into the running engine
- Scale from one physically correct particle to billions of represented particles

## Meaning of Bare Metal

### Achievable on macOS

- Native ARM64 Mach-O executable
- Ahead-of-time compiled Metal compute and render pipelines
- Direct Metal command queues, command buffers, resources, fences, and events
- Explicit memory ownership, layout, alignment, residency, and synchronization
- Allocation-free steady-state simulation
- GPU-driven compute, culling, compaction, and rendering
- Minimal host runtime with no interpreter, garbage collector, or virtual machine

### Platform Boundary

- macOS retains control of the operating-system kernel
- Apple retains control of the GPU driver and undocumented GPU instruction set
- Loom targets GPU-kernel level through Metal Shading Language and compiled Metal libraries
- Loom targets native user-space ARM64 rather than a macOS kernel extension
- Bare-metal performance is a measurable outcome, not a claim of bypassing Metal or macOS

### Explicit Non-Claims

- No direct access to Apple GPU machine instructions
- No replacement of the macOS scheduler, display compositor, or GPU driver
- No guarantee that custom ARM64 emission is faster than LLVM-generated host code
- No assumption that unified memory removes synchronization or bandwidth costs

## Loom System Definition

### Loom Is

- A machine-first language for physical computation
- A canonical typed semantic graph with a compact binary serialization
- A deterministic compiler and validator
- A low-level 3D physics runtime
- A hardware profiler and kernel benchmark system
- An agent optimizer that proposes verified implementation variants

### Loom Is Not

- A general-purpose human programming language
- A conversational prompt interpreted at runtime
- A neural model in the trusted execution path
- A text-file-oriented source language
- A wrapper around an existing game engine
- A promise that AI can ignore hardware limits

### Trust Boundary

- Agent
  - Converts objectives into semantic graph changes
  - Proposes memory layouts, kernels, schedules, and optimizations
  - Generates multiple bounded candidates
- Deterministic compiler
  - Type-checks physical units
  - Validates effects, memory safety, capabilities, and contracts
  - Produces reproducible native artifacts
- Verifier
  - Runs correctness scenarios
  - Runs determinism checks
  - Measures performance and memory
  - Rejects invalid or slower candidates
- Runtime
  - Executes only validated artifacts
  - Does not call an AI model during a simulation step

## Core Machine Primitives

### Particle

- Fundamental physical state atom
- Stable semantic identity where identity is required
- Position
- Velocity
- Optional acceleration
- Physical attributes
- Lifecycle state

### Space

- Coordinate system
- Units
- Origin and bounds
- Precision policy
- Partitioning policy
- Boundary conditions

### Field

- Gravity
- Force
- Density
- Velocity
- Pressure
- Signed distance
- Probability or occupancy

### Kernel

- Pure transformation over particle streams or fields
- Explicit input and output buffers
- Explicit read and write effects
- Explicit execution domain
  - GPU compute
  - GPU render
  - CPU SIMD fallback
- Explicit scheduling and synchronization requirements

### Buffer

- Element schema
- Structure-of-arrays or array-of-structures layout
- Storage mode
- Residency policy
- Alignment
- Capacity
- Mutability
- Lifetime

### Schedule

- Fixed simulation timestep
- Dispatch order
- Dependencies
- Barriers
- Asynchronous overlap
- Update frequency by simulation level

### Contract

- Physical correctness
- Memory safety
- Determinism tier
- Maximum allocation count
- Maximum copy count
- Performance budget
- Working-set budget
- Numerical error budget

### Projection

- Render view
- Inspector view
- Telemetry view
- Debug-readable Loom projection
- Never the canonical program representation

## Hello Particle Vertical Slice

### Goal

- Prove the complete Loom path with one particle
- Validate architecture rather than claim a one-particle GPU speedup
- Use the exact memory and dispatch model that later expands to particle batches

### World

- Right-handed 3D coordinate system
- Meters, seconds, and kilograms
- Fixed 120 Hz simulation clock
- Constant gravity field
- Static ground-plane boundary
- Perspective camera

### Particle

- Initial position
  - X = 0 meters
  - Y = 1 meter
  - Z = 0 meters
- Initial velocity
  - X = 0 meters per second
  - Y = 0 meters per second
  - Z = 0 meters per second
- Radius = 4 millimeters
- Mass = 1 gram
- Restitution
- Friction
- Display color

### Physics Kernel

- One GPU thread updates one particle
- Semi-implicit Euler integration
- Sphere-plane contact
- Normal impulse
- Simple friction
- Fixed delta time
- No heap allocation
- No CPU readback in the normal frame loop

### Render Pipeline

- Compute kernel writes particle position
- Render stage reads the same current GPU state
- Vertex shader expands a point or instanced quad
- Fragment shader shades a simple sphere impostor
- Depth test places the particle in 3D
- Camera state is separate from simulation state

### Why Use the GPU for One Particle

- A CPU update would have lower latency for one particle
- The GPU version validates the scalable execution path
- Performance work begins only after dispatch overhead is amortized across large batches
- Loom must measure the CPU/GPU crossover rather than hard-code GPU use for every workload

## Canonical Loom Representation

### Storage

- Versioned binary semantic bundle
- Content-addressed declarations
- Stable semantic IDs
- Typed graph edges
- Reproducible serialization
- Optional debug projection for human inspection

### Agent Input

- Objective
- Hardware target
- Physical invariants
- Performance budgets
- Allowed capabilities
- Change scope

### Semantic Graph Nodes

- Target
- Space
- Particle schema
- Field
- Boundary
- Buffer
- Kernel
- Schedule
- Render projection
- Contract
- Scenario
- Benchmark
- Provenance

### Debug Projection

- Textual representation generated from the semantic graph
- Useful for logs, diffs, review, and bootstrapping
- Never executed directly in production
- Never treated as the source of truth

## Compiler Architecture

### Trusted Compilation Pipeline

- Canonical Loom semantic bundle
- Schema and version validation
- Unit and dimensional analysis
- Effect and capability validation
- Memory-layout validation
- Physics IR
- Spatial execution graph
- Hardware IR
- Metal compute and render generation
- ARM64 host generation through Rust and LLVM initially
- System linker and code signing
- Native application bundle

### Physics IR

- Coordinate spaces
- Physical units
- State streams
- Fields
- Neighbor relations
- Boundaries
- Integration method
- Precision policy
- Determinism policy

### Hardware IR

- Buffer layout
- Resource storage modes
- Pipeline stages
- Thread grid
- Threadgroup size
- SIMD-group operations
- Barriers and events
- Indirect dispatch
- Residency
- Precision lowering
- Kernel fusion

### Agent Optimization Loop

- Generate bounded candidate variants
- Compile every candidate deterministically
- Verify physical contracts
- Capture Metal GPU counters
- Measure occupancy, bandwidth, execution time, and thermal state
- Compare against the current accepted artifact
- Accept only a reproducible improvement
- Record hardware, operating system, compiler, and kernel provenance

### Bootstrap

- Rust implements the first semantic store, compiler driver, runtime host, and Tauri bridge
- Metal Shading Language implements the first GPU kernels
- Rust and LLVM produce host ARM64 code
- System tooling produces Mach-O binaries and signed app bundles
- Custom SSA IR follows after Loom semantics stabilize
- Direct ARM64 host emission remains optional and evidence-driven

## Apple Silicon Execution Architecture

### Control Plane

- Native Rust host
- Tauri commands
- Input events
- Fixed-step scheduling
- Pipeline creation
- Command-buffer submission
- Sparse telemetry

### GPU Data Plane

- Particle integration
- Field sampling
- Spatial indexing
- Collision candidates
- Constraint solving
- Visibility
- Compaction
- Render-command generation
- Rasterization

### Memory Policy

- Unified memory describes the shared physical memory architecture
- Storage mode is selected per resource from actual access patterns
- GPU-hot particle state uses GPU-private Metal buffers when the CPU does not need access
- CPU-authored controls and small shared data use shared Metal buffers
- Inspector snapshots use a small shared staging ring
- Temporary render targets use memoryless storage where applicable
- CPU and GPU never touch the same writable resource concurrently
- Synchronization is explicit even when no physical copy occurs

### Hot Particle State

- Structure-of-arrays layout
- Position stream
  - Prefer packed or quantized cell-local representation at scale
- Velocity stream
- Lifecycle and flags stream
- Optional physical-property streams
- No per-particle tick
- No per-particle 64-bit semantic ID unless the use case requires identity
- Global tick stored once per world
- Cold render and metadata streams separated from physics state

### Hello Particle Layout

- Position padded or packed according to measured Metal access behavior
- Velocity padded or packed according to measured Metal access behavior
- Mass, radius, restitution, and friction in a compact property block
- Color kept in render-only state
- Layout selected by benchmark rather than assumed from 16-byte alignment alone

### Synchronization

- Triple-buffered small CPU-authored control data
- GPU event or command-buffer dependency between compute and render
- No blocking CPU wait in the normal frame loop
- Inspector readback is asynchronous
- Simulation remains correct when the inspector is absent

## Tauri Window Into Loom

### Role

- Operator control surface
- Compiler status
- Runtime diagnostics
- Particle inspector
- Benchmark viewer
- Not part of the simulation hot path

### Native Viewport

- Tauri owns the application shell
- A native macOS view backed by CAMetalLayer owns the 3D viewport
- Rust and native macOS glue connect the Metal view to the Tauri window
- The WebView does not render the particle through JavaScript, WebGL, or canvas
- The WebView communicates through bounded asynchronous commands and telemetry

### Controls

- Run
- Pause
- Reset
- Advance one fixed tick
- Attach or detach inspector
- Start benchmark
- Select accepted kernel variant

### Diagnostics

- Current position and velocity
- World tick
- Physics and render rates
- Compute and render GPU time
- Working-set size
- Bytes read and written per step
- Allocation and copy counts
- Active and represented particle counts
- Active kernel hash
- Compiler and hardware profile

## Determinism

### Tier 1 — Replay on Identical Target

- Same chip family
- Same operating-system and Metal compiler versions
- Same compiled kernel
- Same initial buffers
- Same ordered inputs
- No data races
- Bitwise replay is the target

### Tier 2 — Replay Across Compatible Targets

- Same physical outcome within declared tolerances
- Bitwise identity is not assumed
- Floating-point contraction and execution-order differences are recorded

### Tier 3 — Optimized Statistical Simulation

- Aggregate invariants are preserved
- Individual particle identity may be discarded
- Intended for massive stochastic or field-derived populations

## Billion-Particle Reality Check

### Dense-State Cost

- One billion particles at 32 bytes of hot state require 32 GB before auxiliary data
- Reading and writing that state once requires at least 64 GB of memory traffic per step
- At 120 updates per second that lower bound is 7.68 TB per second
- A top M4 Max configuration provides up to 128 GB unified memory and 546 GB per second memory bandwidth
- Therefore one billion fully resident, fully updated particles at 120 Hz are not a credible single-M4 target
- Physics interactions, spatial indexing, rendering, and synchronization add further cost

### Rendering Cost

- A display has far fewer pixels than one billion particles
- Drawing every represented particle creates extreme overdraw with no visible benefit
- Compute must select, aggregate, or synthesize visible representatives before rendering
- Simulation count, represented count, active count, and visible count are separate metrics

### Credible Meaning of Billions

- Billions of particles represented in the world model
- A smaller active working set resident on one GPU
- Different regions updated at different rates
- Distant populations stored as fields, distributions, aggregates, or procedural generators
- Near-camera or interaction-critical particles expanded into explicit state
- Multi-device execution for dense global updates

## Scaling Architecture

### Level 0 — Explicit Active Particles

- Full position and velocity
- Highest update rate
- Local collisions
- Near camera or robot

### Level 1 — Quantized Particle Chunks

- Cell-local coordinates
- Reduced precision where error contracts allow it
- Shared chunk metadata
- Lower memory per particle

### Level 2 — Aggregated Particle Clusters

- Center of mass
- Velocity distribution
- Density
- Bounds
- Statistical material properties

### Level 3 — Continuous Fields

- Density field
- Velocity field
- Pressure field
- Signed-distance field
- Wave representation

### Level 4 — Procedural Population

- Seed
- Generator
- Boundary conditions
- Material rules
- Expanded only when observation or interaction requires particles

### Streaming and Residency

- Partition world space into chunks
- Maintain a bounded resident working set
- Use Metal heaps and residency controls where supported
- Stream or regenerate inactive chunks
- Prefer recomputation over storage when it is cheaper
- Respect the device-reported recommended maximum working-set size

### Update Scheduling

- 120 Hz for near and contact-critical particles
- Lower rates for distant or slow regions
- Event-driven wake-up for dormant regions
- Temporal interpolation for rendering
- Budgeted active-set selection each frame

### Interaction Scaling

- Never use all-pairs particle interaction
- Uniform grids for locally uniform scenes
- Spatial hashing for sparse scenes
- Morton ordering for locality
- Hierarchical grids or trees for mixed scales
- Neighbor lists for bounded-radius interactions
- Fields or multipole-style approximations for long-range effects

### GPU-Driven Rendering

- Compute visibility on the GPU
- Compact visible particles
- Select representation level
- Generate indirect draw commands
- Render only visible representatives
- Avoid CPU readback between simulation and rendering

## Scale Milestones

### Gate 0 — One Particle

- Complete Loom-to-Metal path
- Correct gravity and ground contact
- Tauri window and native Metal viewport
- Zero steady-state allocations
- No synchronous CPU readback

### Gate 1 — 1,024 Particles

- Batch dispatch
- Structure-of-arrays storage
- CPU versus GPU crossover benchmark
- Deterministic replay

### Gate 2 — One Million Particles

- GPU integration
- GPU culling and compaction
- Indirect rendering
- Bandwidth and occupancy profiling
- Stable real-time update on the selected M4 target

### Gate 3 — Ten to One Hundred Million Active Particles

- Quantized chunk-local state
- Spatial partitioning
- Multi-rate updates
- GPU-private hot buffers
- Explicit working-set enforcement

### Gate 4 — One Billion Represented Particles

- Hierarchical particles, clusters, fields, and procedural populations
- Bounded active and visible sets
- Streaming or regeneration
- Measured error against a smaller full-resolution reference
- No claim that all one billion particles receive full-rate dense updates

### Gate 5 — Distributed Dense Scale

- Hardware-neutral Loom execution graph
- Multiple GPU or node partitions
- Deterministic boundary exchange where required
- Distributed profiling and replay

## Implementation Roadmap

### Phase 0 — Hardware Probe

- Query Metal GPU family and feature support
- Query recommended maximum working-set size
- Record thread execution width and maximum threads per threadgroup
- Establish GPU counter capture
- Establish thermal and power-state reporting
- Save the target profile in Loom provenance

### Phase 1 — Native Hello Particle

- Build Tauri shell
- Embed native CAMetalLayer viewport
- Create Metal device, queue, compute pipeline, and render pipeline
- Allocate particle and control resources
- Dispatch one-thread physics kernel at fixed 120 Hz
- Render from GPU state
- Implement asynchronous inspector snapshot

### Phase 2 — Trusted Loom Core

- Define semantic graph schema
- Define binary bundle format
- Implement dimensional types
- Implement effects and capabilities
- Implement deterministic validator
- Generate the Hello Particle Metal and host artifacts
- Produce a debug-readable projection

### Phase 3 — Batch Engine

- Replace scalar particle record with structure-of-arrays streams
- Add dispatch sizing from runtime hardware properties
- Add GPU culling and compaction
- Add indirect rendering
- Benchmark 1,024 through one million particles

### Phase 4 — Spatial Physics

- Add chunked world space
- Add uniform-grid or hash-grid construction
- Add neighbor queries
- Add bounded particle interactions
- Add collision and constraint passes
- Preserve race freedom and declared determinism

### Phase 5 — Hierarchical Scale

- Add quantized cell-local particles
- Add particle-cluster representation
- Add field representation
- Add procedural expansion and collapse
- Add active-set and update-rate scheduler
- Demonstrate one billion represented particles with a bounded active set

### Phase 6 — Agent Optimizer

- Expose legal transformation space
- Generate kernel and layout candidates
- Run correctness and error contracts
- Collect Metal counters and timing
- Reject regressions
- Store accepted variant and complete provenance

### Phase 7 — Additional Hardware

- Preserve hardware-neutral Physics IR
- Add CUDA backend
- Add Vulkan compute backend
- Add CPU SIMD fallback
- Compare backend-specific precision and determinism

## Repository Architecture

### `loom-core`

- Semantic graph
- Binary bundle
- Units
- Effects
- Capabilities
- Contracts
- Provenance

### `loom-compiler`

- Physics IR
- Spatial execution graph
- Hardware IR
- Metal code generation
- Host code generation
- Artifact cache
- Validation

### `loom-agent`

- Objective planner
- Candidate generator
- Bounded transformation library
- Benchmark controller
- Acceptance policy

### `loom-runtime`

- Fixed-step scheduler
- Metal device and queues
- Resource manager
- World and chunk manager
- Replay
- Telemetry

### `loom-metal`

- Integration kernels
- Spatial-index kernels
- Collision kernels
- Compaction kernels
- Indirect-render generation
- Vertex and fragment shaders

### `loom-window`

- Tauri shell
- Native CAMetalLayer bridge
- Controls
- Inspector
- Benchmark views

### `examples/hello-particle`

- Canonical Loom bundle
- Debug projection
- Contracts
- Scenarios
- Expected traces
- Benchmark baseline

## Hello Particle Acceptance Contract

### Build

- `loom check hello-particle`
- `loom build hello-particle --target apple-m4`
- `loom run hello-particle`

### Visible Result

- Tauri application opens
- Native Metal viewport displays one particle in 3D
- Particle falls under gravity
- Particle collides with and bounces on the ground plane
- Camera movement changes only the projection

### Runtime Result

- Fixed 120 Hz simulation independent of display refresh
- Pause stops at an exact tick
- Step advances exactly one tick
- Reset restores the exact initial state
- No steady-state heap allocations
- No synchronous readback in the normal loop
- Compute output feeds rendering without a CPU round trip

### Verification Result

- Position and velocity remain finite
- Unit constraints pass
- Ground penetration remains within tolerance
- Identical-target replay meets the declared determinism tier
- GPU timing and bandwidth counters are captured
- Active artifact hash and provenance are visible

## Performance Principles

### Measure First

- Benchmark layouts instead of assuming alignment rules imply speed
- Benchmark shared versus private resource strategies
- Benchmark CPU versus GPU crossover
- Inspect bandwidth, occupancy, limiter, and shader timing counters
- Include thermal state in every accepted benchmark

### Optimize the Data Path

- Minimize bytes moved per active particle
- Separate hot and cold streams
- Fuse compatible kernels
- Avoid global atomics where possible
- Sort or partition for locality
- Keep compute and render on the GPU
- Make readback sparse and asynchronous

### Preserve Correctness

- Every optimization has an error contract
- Fast-math or reduced precision requires explicit authorization
- Kernel variants cannot weaken memory safety
- Performance gains must be repeatable
- The accepted artifact must remain replayable

## Deliberate Non-Goals for Hello Particle

- Human-friendly general-purpose syntax
- Runtime AI inference
- Direct Apple GPU ISA generation
- macOS kernel or driver extensions
- Custom ARM64 emitter
- Multiple particles
- General particle-to-particle collision
- Wave simulation
- Photorealistic rendering
- Editor or scene builder
- Package ecosystem
- Claims of one billion dense 120 Hz particles on a single M4

## Long-Term Applications

### Physical AI

- Agent world models
- Embodied planning
- Sensor-derived particle fields

### Robotics

- Occupancy and motion prediction
- Local digital twins
- Collision forecasting

### Simulation

- Fluids
- Granular materials
- Crowds
- Weather
- Wave-particle systems

### Rendering

- Procedural worlds
- Particle-native geometry
- GPU-driven visibility and representation

## Audit Sources

- [Apple Metal resource-storage guidance](https://developer.apple.com/documentation/metal/choosing-a-resource-storage-mode-for-apple-gpus)
- [Apple Metal unified-memory compute guidance](https://developer.apple.com/videos/play/tech-talks/10580/)
- [Apple GPU indirect-command-buffer guidance](https://developer.apple.com/documentation/metal/encoding-indirect-command-buffers-on-the-gpu)
- [Apple GPU performance-counter guidance](https://developer.apple.com/documentation/xcode/analyzing-apple-gpu-performance-using-counter-statistics)
- [Apple M4 Max unified-memory capacity and bandwidth](https://www.apple.com/ca/newsroom/2024/10/apple-introduces-m4-pro-and-m4-max/)
