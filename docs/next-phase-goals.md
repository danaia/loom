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
- Add multi-version buffers and asynchronous ticks.
- Cache Metal pipelines.
- Compare against direct Metal using the same MSL and data.

**Gate:** Sustain 120 Hz without a per-tick GPU wait.

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
- Bounded neighbor interaction.
- Simple cohesion, separation, alignment, and boundary avoidance.
- Compaction and indirect dispatch.
- GPU-rendered visualization.

**Gate:** Bounded neighbor work using spatial hashing.

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
- [ ] Diagnostics include stable codes and mechanical repairs where safe.
- [ ] Canonical hashes remain stable for equivalent graphs.
- [ ] No parser work is required.

### Correctness

- [ ] The graph validates before backend lowering.
- [ ] Scenarios cover expected physical behavior.
- [ ] Hazards and cross-tick lifetimes are proven.
- [ ] Render dropping cannot corrupt simulation state.
- [ ] Invalid graphs never receive executable fingerprints.

### Metal

- [ ] Resources and bindings come only from `ExecutionPlan`.
- [ ] Threadgroup sizes use compiled pipeline properties.
- [ ] Particle state stays GPU-resident.
- [ ] No unnecessary per-tick CPU/GPU wait.
- [ ] Metal failures produce structured diagnostics.

### Performance

- [ ] Record device, OS, artifact, shaders, pipelines, and workload.
- [ ] Measure GPU time, working set, allocations, copies, and blits.
- [ ] Compare identical MSL and data against direct Metal.
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
