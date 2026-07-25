# Hello Batch — 100K Asynchronous Gate

- Date: 2026-07-25
- Device: Apple M4 Pro
- macOS: 26.6
- Host profile: release
- Particles: 100,000
- Queue: one validator-proven serial Metal queue
- Maximum command buffers in flight: 4
- Per-tick host synchronization: no
- Render target: offscreen private 960×720 texture

## Short regression run

| Mode | Samples | GPU p95 | GPU p99 | CPU submission p95 | End-to-end p95 | Under 8.33 ms |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Headless | 40 | 0.0740 ms | 0.0768 ms | 0.0769 ms | 0.3758 ms | Yes |
| Rendered | 40 | 0.4985 ms | 0.5343 ms | 0.4186 ms | 1.9698 ms | Yes |

The CPU measurement includes command encoding, commit, and bounded queue-admission
backpressure. End-to-end latency begins before command-buffer creation and ends in
Metal’s completion handler.

## One-second sustained smoke test

The headless sustained path completed 31,756 ticks after a one-second wall-time
warm-up:

- GPU p95: 0.0240 ms
- GPU p99: 0.0244 ms
- CPU submission p95: 0.0607 ms
- End-to-end p95: 0.1744 ms
- Measured throughput: 31,745 submitted/completed ticks per second

This smoke test validates wall-time control and bounded asynchronous timestamp
collection. It is not the final 30–60-second thermal benchmark.

## Direct-Metal encoding control

Matched 240-tick offscreen-rendered runs:

| Runner | GPU mean | GPU p95 | CPU submission mean | CPU submission p95 |
| --- | ---: | ---: | ---: | ---: |
| Loom plan | 0.4153 ms | 0.4613 ms | 0.3252 ms | 0.4211 ms |
| Direct Metal encoding | 0.4134 ms | 0.4568 ms | 0.3238 ms | 0.4079 ms |

The mean CPU difference is about 0.0014 ms. At this workload the plan-driven path
is effectively at the fixed-encoding control’s measurement floor. Longer,
interleaved trials are still required before publishing a formal overhead ratio.

The control shares Loom’s validated setup and initialized resources, then bypasses
execution-plan traversal and typed binding lookup during each measured tick. This
holds GPU work constant and isolates command encoding; it is not yet a wholly
independent direct-Metal application.

## Scope

- Compute uses the plan-driven integration and ground-contact passes.
- Rendered mode adds the plan-driven particle view.
- Typed constant values are allocated once as persistent Metal buffers; the
  steady-state path no longer uses `setBytes`.
- GPU values come from actual completed Metal command-buffer timestamps.
- The result fingerprint includes the executable hash, Git revision and dirty state,
  Rust compiler, Metal SDK, device, OS, validated artifact, shaders, and pipelines.
- Drawable acquisition, presentation, compositor cost, direct-Metal parity, working
  set, allocation counts, and explicit copy/blit counters remain open.

## One-million-particle boundary

One-second sustained smoke tests after one second of wall-time warm-up:

| Mode | Completed ticks | GPU p95 | GPU p99 | End-to-end p95 | Under 8.33 ms |
| --- | ---: | ---: | ---: | ---: | --- |
| Headless | 2,604 | 0.3994 ms | 0.6585 ms | 2.1690 ms | Yes |
| Rendered | 285 | 5.8105 ms | 6.0288 ms | 19.1078 ms | Yes |

The rendered throughput test deliberately keeps four command buffers admitted and
therefore reports queueing latency above one 120 Hz tick even though individual GPU
work remains below 8.33 ms. A paced 120 Hz presented test is still required.

Constructing the current one-million-particle graph peaked around 3.0–3.4 GB because
initial stream contents are represented as expanded literal arrays. This is now a
measured language/runtime issue: Loom needs compact, explicit initialization before
the 1M path is considered production-clean.
