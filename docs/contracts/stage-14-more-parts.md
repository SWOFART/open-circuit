# Stage 14 proposed contract for additional parts

The owner accepted the contract proposed in [PR #298](https://github.com/SWOFART/open-circuit/pull/298) for [#207](https://github.com/SWOFART/open-circuit/issues/207). It extends [the architecture](../../ARCHITECTURE.md) and the [control circuits](stage-6-control-circuits.md), [nonlinear circuits](stage-7-nonlinear-circuits.md) and [three-phase](stage-9-three-phase-foundation.md) contracts.

## Shared conventions

All values are finite SI quantities. The tables show defaults followed by inclusive allowed ranges. Catalog type and pin IDs are stable. Parameter and result keys are camelCase. Each input and output branch uses the first pin as positive and positive current entering that pin. Branch result keys are `<branch>VoltageVolts` and `<branch>CurrentAmps`. A branch alone uses `voltageVolts` and `currentAmps`. Each pair is reported in that order, in pin-table order, followed by the extra keys listed below. Reference-input voltages use the named reference pin. Output branch current uses the same passive convention, so a source delivering power has negative current.

Symbol coordinates are local grid units before the standard rotation. Common pin layouts are:

- **Two-pin** `p=(-1,0), n=(1,0)`.
- **Relay** `a1=(-3,-2), a2=(3,-2), no1=(-3,0), no2=(3,0), nc1=(-3,2), nc2=(3,2)`.
- **Two-throw** `com=(-2,0), throw1=(2,-1), throw2=(2,1)`.
- **MOSFET** `g=(-2,0), d=(2,-2), s=(2,2)`.
- **Amplifier** `plus=(-2,-1), minus=(-2,1), out=(2,0), ref=(0,2)`.
- **Converter** `inP=(-3,-2), inN=(-3,2), u=(3,-2), v=(3,0), w=(3,2), neutral=(1,3)`.

All devices use existing diagnostics for invalid geometry and parameters, Newton non-convergence, non-finite and out-of-range results, floating references, and unsettled events. No device adds a diagnostic or changes the earlier thresholds. Each known-answer fixture must compare voltages and currents to analytic values or an independent numerical reference using absolute error no greater than `1e-8 + 1e-3 × |expected|` in the reported SI unit after settling, or the earlier contract's tighter tolerance when applicable. Discrete states, slot order, signs and the public step of first transition must match exactly. Out-of-domain input is refused before mutation. Failed solves retain the last committed state, samples and counters.

Rust owns device evaluation, timing and diagnostic production. Application and UI carry generated transport types, symbol geometry and operator requests. Existing frame encodings retain their magic and extend their tables additively for new slot and state kinds. The catalog and encoding tests must check the new entries. The v3 project stores component parameters and geometry only. Assignment authoring and checks for the new parts are deferred, as are manufacturer model imports, detailed thermal and switching-loss models, field buses, and PLC programming.

## Time relays (#208)

| Type | Pins and branch results | Parameters |
| --- | --- | --- |
| `relay_on_delay` | Relay. Branches `coil`, `no`, `nc`. | `coilResistanceOhms` 240 [1, 1e6], `pickupCurrentAmps` 0.08 [1e-6, 100], `dropoutCurrentAmps` 0.04 [0, pickup exclusive], `closedResistanceOhms` 0.01 [1e-6, 100], `delaySeconds` 0.1 [0, 3600]. |
| `relay_off_delay` | Relay. Branches `coil`, `no`, `nc`. | Same as on-delay. |

The coil is resistive, `I = V/R`. The detector takes the magnitude of settled coil current and switches energized at or above pickup and de-energized at or below dropout. Between thresholds it retains its prior state. At compile, the detector starts de-energized and evaluates the settled operating point. The contact stamps are open (zero conductance) or closed (`1/closedResistanceOhms`). The NO contact follows the relay output and the NC contact takes the inverse.

The Engine measures time using integer public steps. An edge at step `k` starts a pending delay with zero elapsed steps. The deadline is `k + ceil(delaySeconds / 0.001)`, using conversion that preserves exact integer millisecond inputs. Only a successful advance to the deadline can complete the delay. Zero delay settles at the edge, including step zero. Each endpoint samples a settled detector value before completing a due delay, so an opposite edge at the deadline cancels the pending transition. Cascading topology changes settle in the established bounded event loop and use the existing backward Euler rule on the next time-advancing internal step. Substeps and zero-time settles never add timer ticks.

- **On-delay** keeps the output off until the energized input persists for the delay. A dropout immediately switches off and clears the counter.
- **Off-delay** switches on immediately at pickup. Dropout keeps the output on until the delay expires. A pickup before or at the deadline cancels pending release.
- **Reset** clears timer history, restores rest contacts and evaluates the step-zero operating point using the same rules as compile. Parameter edits are compile-only in this stage, including delay and thresholds. A refused live edit leaves the run unchanged.

Extra result `timerElapsedSeconds` is the elapsed whole-step time while pending and zero otherwise. State contains `energized`, `outputOn` and `timing`. Known-answer cases cover 0 ms, 3 ms and 3.5 ms settings, cancellation one step before and at the deadline, both initial input levels, reset, refused live edits, and equality across batched or repeated public advances. The DC coil model omits inductance, rectification, mains zero-crossing detection, leakage, arc/suppression models and power-off hold-up dynamics beyond the modeled timer.

## Operator and sensor contacts (#209)

Use the Stage 6 operator request, stale-run validation and event ordering. Each contact uses `closedResistanceOhms` 0.01 [1e-6, 100]. An open contact has zero conductance. A closed contact stamps `I = V/R`. State changes at the current public time, with all affected branches stamped together. A request for the existing position is idempotent. A refused position, stale revision or failed event settle leaves the old position and frame intact. Only the initial-position input is saved. Reset restores it. All numeric edits are compile-only.

| Type | Pins and results | Positions and saved initial input | Omitted |
| --- | --- | --- | --- |
| `limit_switch` | Two-pin | `released` open, `pressed` closed. Starts released. | Mechanical travel, bounce and geometry-driven actuation. |
| `emergency_stop` | Two-pin | `armed` closed, `latched` open. Press latches; releasing does nothing; a distinct reset rearms. Starts armed. | Redundancy, safety ratings, contacts with forced guidance and restart interlocking. |
| `selector_switch` | Two-throw. Branches `throw1`, `throw2`. | `left`, `center`, `right`. Connects common only to the selected throw; center opens both. Optional initial integer position 0/1/2, default 1. | Mechanical transit speed, contact overlap and bounce. |
| `key_switch` | Two-pin | `off` open, `on` closed. Optional initial integer position 0/1, default 0. | Key authentication, key-removal position and mechanical interlocks. |

Each type reports its operator position as a discrete state. There are no additional scalar slots. Test the complete position tables on a divider, both transition directions, reset, an invalid action, no topology change on emergency-stop release, selector branch isolation, and a same-time relay transition before reporting a settled frame.

The existing request object and command name stay the same. Extend Rust's
`OperatorPosition` and regenerate its TypeScript union with lowercase values
`left`, `center`, `right`, and `reset`. Limit switch accepts `released`
and `pressed`. Emergency stop accepts `pressed` (latch), `released`
(successful no-op), and `reset` (rearm), in either state. Selector accepts
only `left`, `center`, and `right`. Key switch accepts `open` (off)
and `closed` (on). Other device/action pairs use the existing invalid-position
rejection path. Existing devices keep their valid actions. The optional
saved parameter for selector and key switch is `initialPosition`.

## Indicators and actuators

| Type and pins | Model and inputs | Extra results and state | Omitted |
| --- | --- | --- | --- |
| `lamp`, two-pin | Fixed hot resistance `resistanceOhms` 120 [1e-3, 1e9]. `I=V/R`. | `powerWatts=V×I`, `lit` at `powerWatts >= 0.25`. | Inrush, filament temperature, light spectrum and burnout. |
| `indicator`, two-pin | Fixed `resistanceOhms` 2400 [1e-3, 1e9]. `I=V/R`. `thresholdVolts` 12 [0.01, 1000]. | `lit` when `abs(V) >= thresholdVolts`. | Internal rectifier, flashing and brightness curves. |
| `solenoid`, `valve`, two-pin | Series RL with `resistanceOhms` 24 [0.01, 1e6], `inductanceHenries` 0.05 [1e-6, 10], `pickupCurrentAmps` 0.4 [1e-6, 100], `dropoutCurrentAmps` 0.2 [0, pickup exclusive]. `V=RI+L dI/dt`, existing inductor companion model. | `energized` when the magnitude reaches pickup, clear at dropout. Valve state is closed at rest, open when energized. | Motion, hydraulic or pneumatic flow, flyback protection, travel time and feedback. |
| `buzzer`, two-pin | Fixed `resistanceOhms` 240 [1e-3, 1e9]. `I=V/R`. `thresholdVolts` 6 [0.01, 1000]. | `active` when `abs(V) >= thresholdVolts`. | Acoustic output, waveform generation and sound-pressure models. |

Indicator states use the settled endpoint; they have no feedback into the circuit. The actuator detector starts off and evaluates the operating point; current initializes under the existing inductor rules. Tests cover DC Ohm's law, threshold equality, an RL step against the analytic exponential, hysteresis in both directions and reset. Inputs are compile-only; the symbols display reported state only.

**Implementation notes (#210).** The pins are `p` and `n` (the common two-pin layout), and the result keys are `<id>/voltageVolts`, `<id>/currentAmps` and, for the lamp, `<id>/powerWatts`. A solenoid or valve with no initial-current input takes its coil current from the DC operating point, as every inductor without one does, so one wired straight across a source above its pickup current is energized at step zero; the starting-off rule applies to the detector before that first evaluation, and a coil switched on later starts at zero current. A topology change makes the first internal step backward Euler (Stage 3), which keeps the transient RL current within 2 % of the exponential for the first steps and within the shared tolerance once settled. The default designator letters are `H` (lamp, indicator, buzzer) and `M` (solenoid, valve), the nearest of the Stage 12 letters.

## Semiconductor and controlled sources

| Type and pins | Model and inputs | Results, state and omissions |
| --- | --- | --- |
| `led`, two-pin | Use Stage 7 Shockley diode and its Newton limiting. `saturationCurrentAmps` 1e-14 [1e-18, 1e-3], `idealityFactor` 2 [1, 4], `litCurrentAmps` 0.001 [1e-6, 1]. | Standard branch pair; `lit` when forward current is at least threshold. No optical model, capacitance, reverse breakdown, or heating. |
| `zener_diode`, two-pin | Same forward diode. Add reverse branch `min(0, (V+breakdownVolts)/breakdownResistanceOhms)`. `breakdownVolts` 5.1 [0.1, 1000], `breakdownResistanceOhms` 5 [0.001, 1e6], `saturationCurrentAmps` 1e-14 [1e-18, 1e-3], `idealityFactor` 1 [1, 4]. | Standard branch pair; `inBreakdown` for `V < -breakdownVolts`. No avalanche transient, capacitance or heating. |
| `mosfet_n`, MOSFET | Gate open; resistor-like square-law channel. For `Vds>=0` let `v=max(0,Vgs-thresholdVolts)`. Channel current is `gainAmpsPerVoltSquared × (v×Vds - Vds²/2)` for `Vds < v`, otherwise `gainAmpsPerVoltSquared × v²/2`. For negative `Vds`, swap source/drain for the channel and negate its current. `thresholdVolts` 2 [0.1, 20], `gainAmpsPerVoltSquared` 0.1 [1e-6, 100]. | `gateSourceVoltageVolts`, `drainSourceVoltageVolts`, `drainCurrentAmps`. State indicates channel conduction. No body diode, gate capacitance, channel-length modulation, switching losses or complementary device. |
| `operational_amplifier`, amplifier | High impedance inputs; a dependent Thevenin output referenced to `ref` gives `V(out,ref)=clamp(openLoopGain×V(plus,minus), -limitVolts, limitVolts) + outputResistanceOhms×Iout`. Defaults/ranges `openLoopGain` 1000 [1, 1e6], `limitVolts` 12 [0.1, 1000], `outputResistanceOhms` 1 [1e-6, 1e6]. | `differentialVoltageVolts`, `outputVoltageVolts`, `outputCurrentAmps`. State is linear or clipped at either limit. No supply loading, slew rate, bandwidth, offset, or noise. |
| `frequency_converter`, converter | DC enable path `Iin=Vin/inputResistanceOhms`. With `Vin >= enableVolts`, stamp three Thevenin outputs to neutral using `sqrt(2) × outputRmsVolts × sin(2π × frequencyHz × t + phase)` with phases 0, −120°, +120° and series output resistance. Otherwise outputs are open. Defaults/ranges `inputResistanceOhms` 10000 [1, 1e9], `enableVolts` 24 [0.1, 1000], `outputRmsVolts` 24 [0, 1000], `frequencyHz` 50 [1, 100], `outputResistanceOhms` 1 [0.001, 1000]. | Branches `input`, `u`, `v`, `w`. State reports enabled. Phase uses absolute model time from step zero and is shared by all poles. No power balance between input and output, DC bus dynamics, control loops, PWM, harmonics, or regenerative operation. |

All semiconductor and converter inputs are compile-only. Preserve the existing Newton damping, convergence, timestep and event rules. For a converter, evaluate the enable condition on settled endpoints and settle a change as a single output event, applying the Stage 9 frequency limit to internal substeps. Use the Engine's existing `EventChatter` path when connected feedback cannot settle. Verify diode forward and reverse points, zener knee and series load, MOSFET cutoff/linear/saturation and reverse sign, amplifier follower and both rails, and balanced converter phase order, load drop, disabled outputs and reset phase. Unsupported transient detail stays outside these teaching models.

**Implementation notes (#212).** The converter pins are `inP`, `inN`, `u`, `v`, `w` and `neutral` (the layout above), its branch results are `<id>/inputVoltageVolts`, `inputCurrentAmps`, `uVoltageVolts`, `uCurrentAmps` and likewise `v` and `w`, in that order, and its state is `enabled` (OCF2 kind 21). The enable condition is `V(inP, inN) >= enableVolts` with no hysteresis, judged on the settled endpoint: the compile solves the operating point with the outputs open and a converter whose input is already at its enable voltage is enabled by the step-zero settle, as one event. A disabled output carries no current and reads the difference of its node voltages. The output amplitude, frequency and phase sequence are fixed by the inputs: there is no V/f law, no ramp and no frequency command; a drive that must change speed is a new run with other inputs. Enabled, the three outputs are exactly the outputs of a Stage 9 star source with the same RMS voltage, frequency and series resistance, and the induction motor test checks that equality over 0.3 s. The default designator letter is `T`, and the category is `MotorsAndLoads`.

**Implementation notes (#211).** The LED and the zener diode have the pins `p` (anode) and `n` (cathode); the MOSFET `g`, `d`, `s`; the amplifier `plus`, `minus`, `out`, `ref` (the layouts above). Branch results are `voltageVolts` and `currentAmps` for the two diodes, `gateSourceVoltageVolts`, `drainSourceVoltageVolts` and `drainCurrentAmps` for the MOSFET, and `differentialVoltageVolts`, `outputVoltageVolts` and `outputCurrentAmps` for the amplifier, which are positive into the first pin of the branch (an amplifier that delivers power has a negative output current). The LED and the zener diode use the Stage 7 junction and its limiting; the MOSFET and the amplifier are not exponential, so the Newton iteration gets voltage ports (`Junction::port`) for them instead of junctions, with no limiting and no exponent range. The ideality factor of the LED and the zener diode goes up to 4, so the iteration's allowed thermal voltage rises from twice to four times the Stage 7 value; the Stage 7 diode keeps its range of 1 to 2. A gate and the amplifier's inputs draw no current but for 1 pS, the floor the diode and the BJT already use, which keeps an open gate or an unconnected input from making the system singular; the amplifier ties its inputs to each other and its minus input to `ref` through it. The amplifier clips at `|openLoopGain * Vd| > limitVolts` (at equality it is still linear) and the zener breaks down for `V < -breakdownVolts`. The MOSFET reports `conducting` when the overdrive (with the source and the drain swapped for a negative `Vds`) is positive. The default designator letters are `H` (LED), `R` (zener diode, as the Stage 7 diode), `Q` (MOSFET) and `A` (amplifier); the category is `Semiconductors`. This change also fixes the iteration for a device with two terminals on one node (an amplifier in a follower, a diode-connected transistor): their currents are now one current of that node, where the Jacobian correction was applied to each.

## State transport

Add the following camelCase `DeviceState` discriminants and fields to the
Rust-generated contract. Every new record includes `componentId`. Only kinds
6–11 have optional `lastTransitionTimeSeconds`, which tracks the latest
output contact change for relays and position change for operator devices.
At compile the timestamp is absent unless a relay changes output during the
step-zero settle.

| OCF2 kind | JSON kind | JSON fields and byte value |
| --- | --- | --- |
| 6 | `relayOnDelay` | Boolean `energized`, `outputOn`, `timing` in value bits 0, 1, 2 respectively. |
| 7 | `relayOffDelay` | Same as kind 6. |
| 8 | `limitSwitch` | `position` 0 released, 1 pressed. |
| 9 | `emergencyStop` | `position` 0 armed, 1 latched. |
| 10 | `selectorSwitch` | `position` 0 left, 1 center, 2 right. |
| 11 | `keySwitch` | `position` 0 off, 1 on. |
| 12 | `lamp` | Boolean `lit`. |
| 13 | `indicator` | Boolean `lit`. |
| 14 | `solenoid` | Boolean `energized`. |
| 15 | `valve` | `position` 0 closed, 1 open. |
| 16 | `buzzer` | Boolean `active`. |
| 17 | `led` | Boolean `lit`. |
| 18 | `zenerDiode` | Boolean `inBreakdown`. |
| 19 | `mosfetN` | Boolean `conducting`. |
| 20 | `operationalAmplifier` | `state` 0 linear, 1 clippedLow, 2 clippedHigh. |
| 21 | `frequencyConverter` | Boolean `enabled`. |

The existing state record width and byte order are unchanged. Boolean values
encode false as 0 and true as 1. Flags for kinds 6–11 use only bit 0 for
timestamp presence. Kinds 12–21 have zero flags and zero timestamp bytes.
Both decoders reject unknown kinds, out-of-table values, reserved value bits,
unsupported flags, and invalid transition times under existing rules. The
UI displays the reported positions and timer state without using them to solve
the circuit. Round-trip each new kind through Rust and TypeScript, reject
malformed encodings, and assert the existing OCF2 and OCT1 fixtures are
unchanged. New quantity keys append to the existing tables when needed.
