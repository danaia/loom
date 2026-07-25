# Hello Batch — 100K Baseline

- Date: 2026-07-25
- Device: Apple M4 Pro
- macOS: 26.6
- Host profile: release
- Particles: 100,000
- Warm-up: 60 ticks
- Samples: 240 ticks
- Render target: 960×720
- Artifact: `3a76d7960c58bdcd10e130607d59b12b58a56d3fa2450a10033c507127db1067`
- Runtime: `55e466cced4c936c13a44a720d54dee5a6bf3b0b083b4607c850e2c2968748db`

| Mode | GPU p50 | GPU p95 | CPU encoding p50 | CPU encoding p95 | Under 8.33 ms |
| --- | ---: | ---: | ---: | ---: | --- |
| Headless | 0.0173 ms | 0.0183 ms | 0.0051 ms | 0.0082 ms | Yes |
| Rendered | 0.3658 ms | 0.4373 ms | 0.0108 ms | 0.0320 ms | Yes |

## Scope

- Headless includes integration and ground contact.
- Rendered adds the plan-driven particle view into a private 960×720 target.
- GPU values use completed Metal command-buffer timestamps.
- CPU values measure encoding and commit, excluding the completion wait.
- Every sampled tick currently waits for completion; this is a measurement baseline,
  not completion of the asynchronous-submission gate.
- This is not yet a direct-Metal comparison.
