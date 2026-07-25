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

The current membership resolver is deliberately serial and deterministic. It is
the correctness oracle for the later stable-ID GPU sort and prefix allocator.
