# Stage 9 reference data

These fixtures are independent checks for the [Stage 9 contract](../contracts/stage-9-three-phase-foundation.md).
The closed-form equations and ngspice circuits do not read Engine output. The complex nodal solve in the generator is
reference mathematics for linear sinusoidal circuits; it does not add a frequency-domain mode to the product.

## Files

All generated data lives under `tests/golden/`.

| File                               | Purpose                                                                                                                     |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `stage-9-manifest.json`            | Fixture inventory, provenance, sign and time conventions, and separate Engine, reference-comparison, and balance tolerances |
| `stage-9-analytical.expected.json` | Closed-form transient and steady-state values                                                                               |
| `stage-9-ngspice.expected.json`    | Results from the pinned ngspice release                                                                                     |
| `stage-9-ngspice/*.cir`            | One reproducible netlist per fixture                                                                                        |
| `scripts/stage9-references.mjs`    | Case table, equations, netlist generation, simulator runner, and offline checks                                             |

## Fixture coverage

The 27 circuits contain 1,460 stored observations:

| Group                                   | Fixtures                                                                                                                                             |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Single-phase transient and steady state | RC, RL, and RLC at 50 Hz and 60 Hz, with source phases 0° and 90°; RC at 400 Hz                                                                      |
| Wattmeter                               | RL load with the normal current orientation and a reversed-current wrong variant                                                                     |
| Star loads                              | Balanced resistive and RL loads with neutral connected and floating; unbalanced load with neutral, without neutral, and with reversed phase sequence |
| Delta loads                             | Balanced and unbalanced delta loads on star and delta sources; a wrong-connection variant with two branches exchanged                                |

Single-phase fixtures record the first microsecond, each millisecond through 40 ms, and 21 points over a later steady-state
period. The wattmeter fixtures record 201 equally spaced points in a 20 ms measurement window. Three-phase fixtures record
21 points over one period after 100 ms, after the RL startup has decayed.

## How the references are computed

The RC and RL transient answers use the first-order differential-equation solutions, and the RLC answers combine the
sinusoidal forced response with the underdamped natural response. Initial capacitor voltage and inductor current match the
source operating point at `t = 0`. The three-phase answers solve the linear sinusoidal network as a complex nodal system,
then evaluate the resulting node voltages and branch currents at each observation time. The wattmeter's RMS, mean, peak,
real power, reactive power, apparent power, power factor, and phase angle are derived from its stated one-period window.
The generator uses only Node built-ins and contains no Engine import.

Netlists use a peak source of `sqrt(2) * Vrms`, a sine source with the contract's phase convention, and the contract's
0.1 Ω source resistance. Passive branch currents are recorded with ngspice `savecurrents`; source winding current is read
through its series resistance. The ngspice version is `ngspice-47`, from the Windows release archive
[`ngspice-47_64.7z`](https://sourceforge.net/projects/ngspice/files/ng-spice-rework/47/ngspice-47_64.7z), SHA-256
`59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a`. The simulator is not stored in the repository or
needed by normal tests.

## Tolerances and checks

`engine` tolerances in the manifest are the contract acceptance limits. In particular, 50/60 Hz amplitude uses 5e-3
relative error, while 400 Hz uses 1e-2 relative error: the contract estimates about 0.8% worst-case amplitude error at
400 Hz. The remaining amplitude, phase, startup, meter, frequency, and three-phase limits are recorded in the manifest
and the Stage 9 contract. Where a circuit has a more specific comparison, its `engineTolerance` is repeated alongside
that fixture in the manifest and expected data.

The independent closed-form answers are checked to 1e-9 A KCL and 1e-8 W power balance. ngspice's transient output is
checked to 1e-8 A KCL and 1e-3 W power balance, to account for numerical error in the simulator's sampled branch
currents. Each ngspice voltage is then compared with the closed form using 1e-4 V absolute plus 1e-6 relative error;
each current uses 1e-6 A absolute plus 1e-6 relative error. Wattmeter powers use 1e-4 W absolute, power factor uses
1e-6 absolute, and phase angle uses 1e-4 degrees absolute; these comparisons also have a 1e-6 relative bound. These
simulator checks do not loosen the Engine's contract tolerances.

Every stored observation is checked against KCL and power balance. Wrong variants must differ from their paired correct
fixture in at least one observed node voltage or branch current. The offline check also verifies that the checked-in
netlists match the case table and that the manifest and analytical answers have the expected structure. Object keys,
array lengths, strings, and netlist text must match exactly; generated numeric fields allow 1e-12 absolute plus 1e-12
relative error to avoid depending on platform-specific last bits from JavaScript math functions. Nothing is rewritten by
`--check`, and expected data is never generated from the Engine or from ngspice.

## Regeneration

From the repository root, Node 24 is sufficient for the offline commands. To reproduce the simulator data, provide the
ngspice-47 console executable separately:

```text
node scripts/stage9-references.mjs --write-netlists
node scripts/stage9-references.mjs --analytical --write
node scripts/stage9-references.mjs --manifest --write
node scripts/stage9-references.mjs --ngspice <path to ngspice-47 console executable> --write
node scripts/stage9-references.mjs --check
```

`--ngspice` validates all simulator results against the independently computed answers before writing them. The ordinary
Vitest run invokes only `--check`, so CI needs no simulator installation.

## How later Stage 9 work uses these fixtures

Engine tests for issues #166–#168 should build circuits through the public Engine inputs and compare the resulting values
with the analytical observations and per-fixture `engineTolerance`. The three-phase fixtures cover neutral handling,
sequence reversal, and delta branch orientation; the wattmeter pair covers current sign. The bridge and application
acceptance work can use the same circuits and observation times without adding derived values to project files.
