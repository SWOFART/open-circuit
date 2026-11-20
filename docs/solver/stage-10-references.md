# Stage 10 reference fixtures

The [Stage 10 contract](../contracts/stage-10-three-phase-equipment.md) defines
the models, signs, and acceptance tolerances. The generator writes 26 cases
under `tests/golden/stage-10`. It never reads Engine results. The checked-in
manifest records the case inputs, analysis and options, observation times,
vector order, and SHA-256 of every netlist.

| Cases | Analytical or numerical reference | ngspice reference |
| --- | --- | --- |
| Contactor at 50 and 60 Hz | Exact R–L current with a voltage change, midpoint integration of the exponential detector, and threshold hysteresis | Series R–L coil, behavioral squaring, RC detector and hysteretic switch |
| Breaker trip and reset | Prospective current and an independent trip/reset sequence | A threshold detector and charge-retaining latch with finite 50 µs response, sampled between operator events |
| Thermal overload | Maximum squared phase current, exponential detector and piecewise thermal dose on a 10 µs grid | Three branch-current probes, behavioral maximum, RC detector and dose integration |
| Blocked rotor and run-up | Amplitude-invariant stator and rotor current equations integrated by RK4 at 2 µs | Behavioral flux integrators with explicit shaft torque and friction. The blocked fixture fixes shaft speed at zero |
| Transformer and loads at 50 and 60 Hz | Complex impedances, mutual-inductance two-port and floating-neutral KCL | AC analysis with coupled inductors and series resistors. No-load voltage probes present a 1 GΩ load |
| Direct-on-line and star–delta starters | Coil pickup and dropout from the R–L current and detector, then the motor equations with the resulting source and shunt connections | Behavioral flux integration driven by the equivalent winding circuits for each contact state |

The sign is voltage at the first winding terminal minus the second and
current entering the first. The starter fixtures each include a schema-2
schematic, orthogonal signal wires, separately supplied Engine pin geometry,
operator actions, and the IDs of the motor and auxiliary devices. Pin
geometry in these fixtures is for the Engine acceptance tests. The desktop
editor uses its separate symbol geometry.

The Engine test compiles each wired starter from scratch before comparing
its transient against both oracles. For the long trace comparison it places
the resolved pins of each conductor at one point and checks that the
initial result equals the full drawing's result. Both oracles provide the
three source line currents, including winding shunt current and the
delta line-current differences. The tests check their signs, KCL, and
sampled RMS within 2%. The tests also check that the
main coil is fed through the overload NC path and that a delta request
while the star contactor is picked cannot engage delta. The later
transition includes an open interval before delta picks. Every motor
winding and control coil has a resistive return path. The swapped-phase
fixtures swap the L2 and L3 connections and verify reversed rotation.
Operator requests are submitted one public step after their intended
electrical boundary because the Engine recomputes the newest interval.

## Regeneration

The Node.js version is pinned by the repository to 24.19.0. The SPICE
reference uses the console executable from the ngspice 47 Windows archive
`ngspice-47_64.7z`. Its SHA-256 is
`59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a`.
The generator verifies the ngspice version before it runs any netlist.

```text
node scripts/stage10-references.mjs --write --ngspice <path-to-ngspice-47-console>
node scripts/stage10-references.mjs --check --ngspice <path-to-ngspice-47-console>
node scripts/stage10-references.mjs --check
cargo test -p open-circuit-engine --test stage_10_starter_references
```

The offline check regenerates the analytical reference and verifies all
committed netlists, the manifest, their simulator observations, and the
starter drawings. Supplying ngspice reruns the netlists and compares the
new samples with the committed simulator data. Transient observations
are interpolated at the manifest times. For the 50 and 60 Hz AC analyses,
the RMS phasors are evaluated at those same observation times. Regular CI
uses the committed references and does not require a simulator install.
