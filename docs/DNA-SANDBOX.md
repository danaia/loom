# Quantum-to-DNA sandbox

Status: executable multiscale visualization baseline  
Primary graph: `baseline/baseline-cuda.pqo`  
Presentation target: CUDA/Vulkan

## Scientific contract

The sandbox connects deliberately different model boundaries:

```text
analytic isolated hydrogen 1s
    → nucleotide interaction sites
    → complementary base pairs
    → ideal B-DNA double helix
    → elastic centerline
```

The hydrogen endpoint evaluates the normalized nonrelativistic `1s`
probability density documented in `BASELINE-ATOM.md`. The DNA endpoint is an
ideal equilibrium B-DNA hierarchy. It does **not** assert that the hydrogen
wavefunction can be copied into carbon, nitrogen, oxygen, phosphorus, bonded
orbitals, or a complete DNA electronic wavefunction.

The 12-base-pair reference uses the Drew-Dickerson sequence
`CGCGAATTCGCG`, associated with the crystallographic B-DNA reference
[PDB 1BNA](https://www.rcsb.org/structure/1BNA). The executable currently uses
idealized geometry rather than importing its experimental atomic coordinates.

## Geometry

For base-pair index `i`, the equilibrium phase and axial coordinate are:

```math
\theta_i = i\frac{2\pi}{10.5},
\qquad
z_i = \left(i-\frac{N-1}{2}\right)(0.34\ \mathrm{nm}).
```

Two antiparallel backbone sites lie on opposing sides of a one-nanometre
radius helix. Interior sites encode complementary A–T and C–G pairs. The graph
records a 50 nm reference persistence length for a future elastic-rod energy
model.

The bend control deforms the centerline smoothly. The thermal control excites
bounded deterministic display modes, and central separation opens paired sites
locally. These are causal geometric perturbations, not a thermostatted
molecular-dynamics integrator and not calibrated free energies.

## Representation levels

| Level | Name | Meaning |
| --- | --- | --- |
| 0 | Quantum | Analytic hydrogen `1s` probability density |
| 1 | Molecular | Three-site nucleotide abstraction |
| 2 | Structural | Complementary base-pair sites and backbones |
| 3 | Mesoscale | B-DNA double helix with pairing rungs |
| 4 | Continuum | Tube around an elastic DNA centerline |

Changing the level changes the model contract. It is not merely hiding
polygons. Future simulation LOD should depend on chemical importance and model
error in addition to camera distance.

## Controls

- `sandbox.scale`: integer representation level from 0 through 4.
- `sandbox.base_pairs`: 2–24 visible pairs. Twelve is the reference; longer
  exploratory strands repeat the sequence.
- `sandbox.thermal`: bounded display-mode amplitude.
- `sandbox.bend`: signed centerline bend.
- `sandbox.separation`: localized central strand opening.
- `sandbox.motion`: freezes or advances the display modes.
- `sandbox.show_bases`: toggles base sites and pairing rungs.
- `sandbox.smart_lod` and `sandbox.lod_bias`: bound fragment ray-march steps.

The Vue panel emits these names through the Linux control bridge. Vulkan
clamps every input before copying it into the fragment push-constant block.
The UI is never authoritative simulation state.

## Performance design

The hydrogen density remains a dirty-versioned 100³ CUDA field. DNA is not
represented as one volumetric field per atom. The B-DNA view evaluates a
bounded maximum of 24 base pairs and 112 ray steps, with adaptive steps when
smart LOD is active.

The next performance and fidelity gate is GPU-resident structural state shared
directly between CUDA and Vulkan. It should add:

1. imported PDB/mmCIF atom coordinates and topology;
2. validated bonded, angular, torsional, electrostatic, and van der Waals terms;
3. constrained integration and neighbor lists for atomistic fragments;
4. a nucleotide rigid-body model for longer real-time strands;
5. local QM/MM or learned reactive regions where electronic change matters;
6. CUDA/Vulkan external-memory and semaphore synchronization;
7. energy, temperature, bond-error, twist, rise, and pairing telemetry.

Coarse-grained DNA is an established strategy; oxDNA uses rigid nucleotide
bodies with interaction sites rather than independent atomic density volumes.
See the [oxDNA project](https://dna.physics.ox.ac.uk/) and its publications.
Local quantum treatment surrounded by molecular mechanics follows the QM/MM
boundary summarized in [PubMed 37100031](https://pubmed.ncbi.nlm.nih.gov/37100031/).

## Honest limitations

- The visual DNA is idealized rather than the atomic coordinates of 1BNA.
- No molecular-mechanics forces or time integrator run yet.
- Thermal and opening controls are not calibrated thermodynamic variables.
- The shader mirrors graph parameters but does not consume CUDA structural
  buffers directly.
- The only electronic state currently solved is isolated hydrogen `1s`.
- “Quantum-to-DNA” describes an explicit hierarchy of model boundaries, not an
  exact many-electron calculation of DNA.
