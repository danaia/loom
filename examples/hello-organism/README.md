# Hello Organism

The executable typed graph is built by
`loom_core::hello_organism_builder(capacity)`. It intentionally remains
parser-independent while Loom v0 syntax is unfrozen.

The specimen declares:

- one mutable active-count stream,
- structure-of-arrays cell identity, developmental, health, memory, and physical state,
- quantized perception and typed intent streams,
- 256×256 activator, inhibitor, nutrient, and density fields,
- Q16.16 deposit streams,
- separate state, field, and membership mutation capabilities,
- explicit sample, decide, state-resolve, deposit, diffuse, commit, membership-resolve,
  and render phases.

Run it on Metal:

```text
./scripts/run-hello-particle.sh organism 16384
./scripts/run-hello-particle.sh organism 16384 --bench headless --samples 300
```

Population mutation is GPU-parallel: a stable LSD radix pipeline removes input
storage-order semantics, cells enter bounded spatial bins, each bin is
canonicalized by stable ID, births are qualified in two deterministic phases,
survivors and births receive prefix-scan destinations, and structure-of-arrays
state is compacted through staging buffers before the authoritative count commit.
