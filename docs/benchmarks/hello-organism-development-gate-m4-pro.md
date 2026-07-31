# Hello Organism Development Gate — M4 Pro

Date: 2026-07-25

This is the archived Gate 3 proof record. Gate 4 adds metabolic regulation,
ledger, and audit passes and shifts the current deterministic fate split to 16
boundary and 22 interior cells. See
`hello-organism-homeostasis-gate-m4-pro.md` for the current correctness boundary;
the timing below describes the earlier Gate 3 schedule.

## Correctness load

The Metal test `one_organizer_constructs_a_connected_differentiated_body` starts
with one organizer at the origin and no daughter positions or target geometry.
At tick 3,200 it reports:

```text
population:                 39
components:                  1
organizers:                  1
boundary:                   17
interior:                   21
component unresolved:        0
neighbor overflow:           0
physical overflow:           0
perception truncation:       0
deposit saturation:          0
```

Area, perimeter, compactness, and radial accounting are all nonzero or complete
as applicable. Daughter placement searches the exact eight-sector exposure mask
in a stable-ID/age-derived order, then authoritatively rejects invalid overlap,
lost parent contact, region escape, capacity exhaustion, and lower-ID candidate
conflicts.

`developmental_fields_are_causal_and_logical_replay_is_exact` runs the complete
reference twice and compares integer morphology, identity, lineage,
developmental state, quantized perception, and per-lineage trajectory hashes.
It then records:

```text
reference:             39 cells
activator disabled:     5 cells, no interior tissue
inhibitor disabled:    55 cells
reference envelope: 24–48 cells
```

## Populated reference timing

```text
cargo run --release -q -p pqo-metal --bin hello-particle-view -- \
  organism 1024 --bench headless --warmup 3200 --samples 100
```

Device: Apple M4 Pro. Host profile: release. Source tree: dirty development
artifact.

```text
GPU mean: 1.282 ms
GPU p95:  1.476 ms
GPU p99:  1.557 ms
GPU max:  1.559 ms
stream buffers: 7,926,464 bytes
```

The warm-up develops the one-seed reference before sampling. This is a
39-cell-class populated reference at 1,024 declared capacity; it is not evidence
for 1,024 or 16,384 fully populated cells. Component relaxation and all
morphology reductions still run every tick. Cadenced observation and populated
256/1,024/4,096/16,384 scaling remain open performance work.
