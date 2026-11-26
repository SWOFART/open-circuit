# Stage 14 reference fixtures

The [Stage 14 contract](../contracts/stage-14-more-parts.md) defines the
device models and tolerances. The 72 cases in
[tests/golden/stage-14](../../tests/golden/stage-14) cover all 16 device types.
The generator uses the contract equations to calculate expected results and
runs independent ngspice decks. It does not read Engine output.

| Devices | Analytical reference | ngspice reference |
| --- | --- | --- |
| On-delay and off-delay relays | Integer public-step deadlines, cancellation before or at the deadline, and DC coil and contact currents | DC coil and NO/NC divider at the contracted contact positions |
| Limit, emergency-stop, selector and key switches | Open and closed branches for each operator position | DC open circuits, resistive contacts and the loaded selector branches |
| Lamp, indicator and buzzer | Ohm's law, lamp power and exact state thresholds | DC branch current and voltage, including negative indicator polarity |
| Solenoid and valve | DC resistance, pickup on a rising RL step and the closed-form exponential | DC coil operating points and series RL transients with zero initial current |
| LED and zener diode | Shockley equation and piecewise reverse zener branch | Native SPICE diode and a behavioral reverse zener branch |
| N-channel MOSFET | Square-law cutoff, linear, saturation and reverse conduction | Level 1 MOSFET with body tied to the lower-potential channel terminal |
| Operational amplifier | Loaded follower and both output limits | Behavioral dependent source with feedback and output resistance |
| Frequency converter | DC input current, enable threshold and loaded three-phase sinusoids | Independent sinusoidal Thevenin sources, with branches open in the disabled case |

The manifest names each result and records the public sample step, expected
state, analytical and SPICE observations, and SHA-256 of the matching deck.
Each deck declares its actual electrical inputs. Voltages and currents follow
the contract's passive sign convention. The fixture check uses
`1e-8 + 1e-3 * abs(expected)` for every numerical observation. Discrete
states and exact public relay deadlines come from the analytical reference.
SPICE checks the relay contact operating points at those states. The Engine's
Stage 14 suites exercise operator actions, timer event order, rollback,
hysteresis and reset.

## Regeneration

Use Node.js 24.19.0 from `.node-version` and the console executable from the
[ngspice 47 Windows archive](https://sourceforge.net/projects/ngspice/files/ng-spice-rework/47/ngspice-47_64.7z/download).
The archive SHA-256 is
`59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a`.
The generator verifies the ngspice version and skips user startup files.

    node scripts/stage14-references.mjs --write --ngspice <path-to-ngspice-47-console>
    node scripts/stage14-references.mjs --check --ngspice <path-to-ngspice-47-console>
    node scripts/stage14-references.mjs --check

The offline check verifies the analytical values, committed observations
and every generated deck. Passing ngspice also reruns the decks and checks
the simulator samples against the committed observations.
