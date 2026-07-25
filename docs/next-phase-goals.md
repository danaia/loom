# Next Phase: Hello Swarm

## Goal

Make Hello Swarm Loom’s second conformance specimen and first performance benchmark.

Build evidence that Loom is a scalable, precise, agent-native systems language.

## Language Improvement Loop

For every milestone:

1. Build it with the current typed graph.
2. Record repeated friction and missing proofs.
3. Improve the smallest semantic rule—not surface syntax.
4. Reject hidden state, bindings, synchronization, or authority.
5. Re-run correctness scenarios and direct-Metal benchmarks.
6. Keep the change only when it improves clarity, safety, or measured performance.

## 1. Hello Batch

- Scale 1K → 10K → 100K → 1M particles.
- Reuse fall and bounce kernels.
- Add multi-version buffers and asynchronous CPU submission.
- Cache Metal pipelines.
- Compare against an equivalent direct-Metal runner.

Simulation ticks remain sequential: tick `n + 1` consumes tick `n`. Asynchronous
submission means the CPU may submit without waiting after every tick while the GPU
preserves simulation dependencies and safely overlaps presentation or readback where
permitted. It does not mean dependent physics ticks execute in parallel.

Benchmark every particle count in two modes:

- headless simulation,
- simulation plus rendering at a fixed viewport resolution.

**Gate:** Record whether GPU p95 is below 8.33 ms at 1K, 10K, 100K, and 1M
particles without a per-tick CPU wait. One million particles at 120 Hz is a target,
not a guaranteed outcome.

Current status: asynchronous CPU submission is plan-proven and running with four
bounded command buffers. Timestamp collection is completion-handler based, and
wall-time warm-up/sampling plus p99/end-to-end latency are available. Compact typed
initializers reduced the 1M maximum resident set from roughly 3.38 GB to 92 MB.
A paced 10-second offscreen 1M run completed all 1,200 deadlines with an explicitly
reported 2 ms submission lead.

Presented benchmarking now acquires real `CAMetalLayer` drawables and calls
`presentDrawable`. Its GPU p95 remains below budget, but drawable/display cadence
still causes deadline misses. Hello Batch remains open until presentation is driven
by a display-synchronized admission policy and longer interleaved direct-Metal trials
are recorded from a clean tree.

## 2. Hello Field

- Add a simple GPU attraction or vector field.
- Reduce bounds, energy, and maximum velocity.
- Read counters asynchronously without full-buffer readback.

**Gate:** Multiple plan-driven passes and GPU reductions.

## 3. Hello Swarm

```text
hash
→ organize cells
→ calculate neighbor forces
→ integrate
→ resolve boundaries
→ compact
→ reduce metrics
→ render
```

- Spatial hashing.
- Cell sorting or prefix-sum/scatter.
- A declared maximum number of neighbors examined per particle.
- A declared maximum number of particles represented per cell.
- A deterministic dense-cell overflow policy.
- An asynchronous overflow counter and structured diagnostic.
- Simple cohesion, separation, alignment, and boundary avoidance.
- Compaction and indirect dispatch.
- GPU-rendered visualization.

**Gate:** Bounded neighbor work using spatial hashing, with measurable overflow
behavior and no silent truncation.

### Swarm determinism

Hello Swarm uses tolerance-based physical determinism. Parallel insertion,
neighbor ordering, and floating-point accumulation are not required to reproduce
bit-for-bit results. Its contracts declare numerical tolerances and invariant
metrics explicitly rather than inheriting Hello Particle’s exact tier.

### Swarm collection identity

The active swarm collection is semantically unordered. Particles retain stable IDs
for inspection and external references, but compaction is not required to preserve
storage order.

## Direct-Metal Comparison

Loom and the baseline must use identical:

- MSL kernels and input data,
- buffer layouts and storage modes,
- pipeline descriptors and specialization constants,
- threadgroup sizes and dispatch dimensions,
- command-buffer and encoder grouping,
- rendering resolution,
- warm-up duration and sampling duration.

Report GPU execution time and CPU orchestration time separately.

## Final Proofs

- Loom approaches direct Metal with small measured overhead.
- The runtime is not specialized around Hello Particle.
- Benchmarks are reproducible through runtime fingerprints.
- The kernel-model report identifies the minimum portable operations Loom needs.
- Agents can generate, diagnose, repair, benchmark, and compare each specimen.
- Every performance claim names its device, workload, artifact, and measurement.

## Proof Standard

Loom earns the claim through:

- **Correctness:** deterministic scenarios and executable contracts.
- **Performance:** direct-Metal comparisons using identical kernels and data.
- **Scale:** measured results from 1K through 1M particles.
- **Generality:** Particle, Batch, Field, and Swarm conformance specimens.
- **Agent usability:** structured diagnostics and atomic repairs for real failures.
- **Portability evidence:** a kernel-model report derived from proven workloads.

## Working Checklist

### Language

- [ ] State, units, access, bindings, and authority remain explicit.
- [ ] New semantics solve repeated workload friction.
- [x] Compact stream initialization is typed, deterministic, validated, and
      backend-neutral.
- [ ] Diagnostics include stable codes and mechanical repairs where safe.
- [ ] Canonical hashes remain stable for equivalent graphs.
- [ ] No parser work is required.

### Correctness

- [x] The graph validates before backend lowering.
- [ ] Scenarios cover expected physical behavior.
- [x] Hazards and cross-tick lifetimes are proven.
- [x] Render dropping cannot corrupt simulation state.
- [ ] Swarm contracts use tolerance-based physical determinism.
- [ ] Compaction preserves stable IDs, not storage order.
- [x] Invalid graphs never receive executable fingerprints.

### Metal

- [x] Resources and bindings come only from `ExecutionPlan`.
- [x] Threadgroup sizes use compiled pipeline properties.
- [x] Particle state stays GPU-resident.
- [x] No unnecessary per-tick CPU/GPU wait.
- [x] Sequential tick dependencies remain explicit during asynchronous submission.
- [ ] Dense-cell overflow is counted and diagnosed.
- [ ] Metal failures produce structured diagnostics.

### Performance

- [x] Record device, OS, artifact, host executable, source state, toolchain, shaders,
      pipelines, and workload.
- [ ] Measure GPU time, working set, allocations, copies, and blits. GPU time,
      peak resident set, buffer bytes, copies, and blits are reported; allocator
      instrumentation remains open.
- [x] Benchmark headless and fixed-resolution offscreen-rendered modes.
- [x] Hold all in-process direct-Metal encoding controls constant.
- [x] Measure GPU execution, CPU submission, and end-to-end latency separately.
- [x] Report p95/p99 against the 8.33 ms budget without assuming success.
- [ ] Test 1K, 10K, 100K, and 1M particles.
- [ ] Publish results that are reproducible from a runtime fingerprint.

### Agent Experience

- [ ] An agent can construct the specimen through the typed builder.
- [ ] An agent can explain every inferred execution decision.
- [ ] An agent can apply repairs atomically and revalidate.
- [ ] An agent can compare variants against identical contracts.
- [ ] Failures identify the semantic path and actionable cause.

## Keep Out

- Parser work.
- CUDA or other backends.
- A native Loom kernel language.
- Complex boid behavior.
- Tauri integration.
