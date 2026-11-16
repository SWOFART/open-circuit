# Stage 9 Contract: Alternating Current, the Three-Phase Source, and AC Measurements

**Status:** Draft for review by the Engine and Application workstreams (issue #164).
**Scope:** Sinusoidal voltage sources, a three-phase source in star and in delta, the time policy that lets the existing fixed-step transient engine solve them, and the Engine-owned AC measurements (RMS, mean, peak, frequency, real, reactive and apparent power, power factor, and phase angle) with their window, validity, signs, and result keys. This document amends the earlier contracts where it says so and nothing else.

All quantities are finite SI values, with angles in degrees where a parameter or a result says so. The models are small and generic: they are not manufacturer parts and say nothing about ratings, endurance, or safety.

## 1. Boundaries and compatibility

### 1.1 The decision this contract implements

Alternating current is calculated **in the time domain, by the existing transient engine**: a sinusoidal source is a voltage source whose value depends on simulation time, solved at the fixed 1 ms public step with the deterministic internal substeps, trapezoidal integration, and event rules of stage-3-dynamics.md. There is **no phasor (frequency-domain) mode** in this stage or in Stages 10 to 16; a frequency response, if wanted, is a separate analysis with its own contract (Stage 17). The decision is recorded in ARCHITECTURE.md section 3.3 (the time model) and in the notes of section 12 (the stage table).

### 1.2 What Stage 9 adds

- A **sinusoidal voltage source** (`ac_voltage_source`), two terminals.
- A **three-phase source in star** (`three_phase_source_star`), terminals `l1`, `l2`, `l3` and the neutral `n`, and a **three-phase source in delta** (`three_phase_source_delta`), terminals `l1`, `l2`, `l3`. Each is one device, not three independent sources.
- Three **measuring devices**: an **AC voltmeter** (`ac_voltmeter`), an **AC ammeter** (`ac_ammeter`), and a **wattmeter** (`wattmeter`). They report values over a moving window (section 5).
- New result quantities and units (section 6.1), so that frames and traces carry the measurements. No new frame kind and no new state kind.

The layers keep their Stage 1 roles: Rust owns every equation and every measurement, `crates/app` carries Engine-owned types and adds no formula, and TypeScript draws inputs and shows what the Engine reports. The interface never computes an RMS value, a phase, or a power.

### 1.3 Earlier text this contract amends

| Earlier text | Amendment |
|---|---|
| stage-3-dynamics.md section 2: sources are constant | A source may depend on simulation time (section 3). A DC source is unchanged. |
| stage-3-dynamics.md section 4.2: the internal substep rule and its rate bound | The rate bound also includes the angular frequency of every AC source (section 3.3). The rule for circuits without an AC source is unchanged. In particular, a purely resistive circuit with an AC source no longer reports `internalSubsteps` 1: it reports 2 or more (section 3.3). |
| stage-1.md section 4 and stage-7 section 2: `Unit`, `ResultQuantity`, `ResultSignConvention`, `PinId`, `ParameterId`, `ComponentTypeId` | New values are added (sections 2 and 6.1). No existing value changes its meaning or its serialized name. |
| stage-1.md section 8 (OCF2) and stage-3-dynamics.md section 7 (OCT1) | Unchanged in format. The new slots appear in the result layout like any other (section 6). |
| stage-4-components.md and stage-6-control-circuits.md: relay and contactor coil thresholds | Unchanged, and **not defined for an alternating coil current**; no protection applies (section 9). |
| stage-8-supply-rails.md: rails are conductors; N and PE are labels | Unchanged. The neutral of a star source is the device pin `n`; a rail named `N` is still only a label. |
| stage-2-editor.md and stage-5-assignments.md: the saved project | A project is still a version 2 schematic; it gains six component types (section 8.3). Assignments are unchanged and do not accept the new devices (section 9). |

### 1.4 Out of scope

Phasor and frequency-domain analysis; harmonic and spectrum analysis; waveforms other than the sine (square, triangle, arbitrary); a DC offset on a sinusoidal source; unbalanced amplitudes or phases inside one three-phase source; a three-phase total-power meter; a phase angle measured against a reference signal; protective earth and safety behavior; multi-pole equipment; assignments, faults and checks for the new devices; frequencies outside section 3.4. Each is deferred, not excluded forever, and nothing in the implementation may imply that it exists.

## 2. Catalog entries

### 2.1 Types, pins, and parameters

Parameter IDs are camelCase on the wire. "Domain" is the Engine's numeric domain, with the catalog's `minimumValue` and `maximumValue` (stage-7 section 2.3) where a finite range applies. The value ranges are checked by the Engine at every boundary and are shown by the interface, which does not invent them.

**`ac_voltage_source`** (pins `pin1`, `pin2`; label prefix `V`; shares the prefix of the DC source)

| Parameter | Unit | Default | Domain and range |
|---|---|---|---|
| `voltageRmsVolts` | volt | 230 | strictly positive, 0.001 to 100000 |
| `frequencyHz` | hertz | 50 | strictly positive, 1 to 400 |
| `phaseOffsetDegrees` | degree | 0 | finite, -360 to 360 |
| `internalResistanceOhms` | ohm | 0.1 | non-negative |

**`three_phase_source_star`** (pins in order `l1`, `l2`, `l3`, `n`; label prefix `G`)

| Parameter | Unit | Default | Domain and range |
|---|---|---|---|
| `phaseVoltageRmsVolts` | volt | 230 | strictly positive, 0.001 to 100000; the RMS of each line-to-neutral voltage |
| `frequencyHz` | hertz | 50 | strictly positive, 1 to 400 |
| `phaseOffsetDegrees` | degree | 0 | finite, -360 to 360; the phase of `l1` |
| `internalResistanceOhms` | ohm | 0.1 | non-negative; in series with each winding |
| `reversedSequence` | dimensionless | 0 | zero or one; 0 is the positive sequence (section 4.1), 1 swaps `l2` and `l3` |

**`three_phase_source_delta`** (pins in order `l1`, `l2`, `l3`; label prefix `G`)

| Parameter | Unit | Default | Domain and range |
|---|---|---|---|
| `lineVoltageRmsVolts` | volt | 400 | strictly positive, 0.001 to 100000; the RMS of each line-to-line voltage, equal to the winding voltage |
| `frequencyHz`, `phaseOffsetDegrees`, `reversedSequence` | | as above | as above |
| `internalResistanceOhms` | ohm | 0.1 | **strictly positive**, in series with each winding |

A delta source has no neutral terminal, so it cannot be given an ideal winding: three ideal voltage windings in a closed loop are a singular system. The domain refuses zero rather than inventing a small hidden resistance.

**`ac_voltmeter`** (pins `pin1`, `pin2`; label prefix `PV`)

| Parameter | Unit | Default | Domain and range |
|---|---|---|---|
| `inputResistanceOhms` | ohm | 10000000 | strictly positive |
| `frequencyHz` | hertz | 50 | strictly positive, 1 to 400; sets the measurement window (section 5.1) |

**`ac_ammeter`** (pins `pin1`, `pin2`; label prefix `PA`): `burdenResistanceOhms` (ohm, default 0.1, strictly positive) and `frequencyHz` as above.

**`wattmeter`** (pins in order `v1`, `v2`, `i1`, `i2`; label prefix `PW`): `inputResistanceOhms` (the voltage circuit, between `v1` and `v2`; default 10000000, strictly positive), `burdenResistanceOhms` (the current circuit, between `i1` and `i2`; default 0.1, strictly positive), and `frequencyHz` as above.

Every parameter above is a live parameter, not an initial condition: the values of a running circuit can be changed through the update of stage-3-dynamics.md section 3.4 and follow the rules of sections 3.6 and 5.4. A live change of `reversedSequence` swaps the phases `l2` and `l3` from the newest step, so the waveforms jump by design.

All six types list no fault kinds (`faultKinds` is empty): a fault on one of them is an `InvalidFault` until a later contract adds some.

### 2.2 Message keys

The catalog entries use keys of the form `component.acVoltageSource.name`, `component.acVoltageSource.voltageRms`, `component.threePhaseSourceStar.name`, `component.acVoltmeter.name`, and so on, by the pattern of the existing entries. The interface supplies the English text; the Engine supplies no text.

## 3. Time

### 3.1 The source value

For a sinusoidal source with RMS value `E`, frequency `f`, and phase offset `φ0` (degrees), the open-circuit value at simulation time `t` seconds is

`e(t) = √2 · E · sin(θ(t) + φ0 · π/180)`, where `θ(t)` is the **accumulated phase** of the source in radians. While the frequency has not been changed since the start of the run, `θ(t) = 2π · f · t`; section 3.5 defines how `θ` is kept and section 3.6 what a frequency change does to it.

`pin1` is the positive terminal: `e > 0` means `pin1` is above `pin2`. The terminal sign conventions of the DC source apply unchanged (stage-1.md section 4): the reported voltage is `V(pin1) − V(pin2)`, and the reported current is positive when it enters `pin1`, so a source that delivers power has a negative reported current at the moment its voltage is positive.

### 3.2 When the value is taken

- At the start of a run (`t = 0`), the circuit is initialized by the same operating-point rule as in Stage 3, with every source at its value `e(0)`, not at zero. Capacitors and inductors without an initial condition start from that operating point; a start-up transient follows, exactly as when a real source is switched on at that phase. The reference answers include it.
- In a transient step from `t_a` to `t_b`, a source's value is its value at the **end** of the interval, `e(t_b)`, enforced as the constraint (ideal) or as the Norton equivalent (finite internal resistance) of that interval. Trapezoidal integration of the reactive elements is unchanged. This is the standard treatment and is what the references assume.
- Discrete re-solves at one instant (stage-4-components.md section 4) use the same `t_b`; a source does not change between a state change and its re-solve.
- A step after an event uses backward Euler for one interval as before; the source value is `e(t_b)` there too.

### 3.3 Internal substeps

The rate bound of stage-3-dynamics.md section 4.2 becomes the larger of the circuit's natural-rate bound and `2π · f_max`, where `f_max` is the highest `frequencyHz` among all AC sources and three-phase sources of the circuit (and zero when there is none). The substep count is chosen by the existing rule: the smallest power of two, at most 64, such that `ω_hat · (1 ms / substeps) ≤ √0.03`. A circuit that reaches the limit of 64 keeps the existing warning. Within section 3.4 the limit is never reached by a source alone.

Consequences, stated for the review: 50 Hz uses 2 substeps and 60 Hz uses 4; 400 Hz uses 16. Worst case, `ω · h = √0.03 ≈ 0.173`.

### 3.4 Frequency range and the public step

The supported frequency is 1 Hz to 400 Hz inclusive. The public step is fixed at 1 ms, so 50 Hz is exactly 20 public steps per period, 60 Hz is 16.667 steps per period (not a whole number), and 400 Hz is 2.5. Frames and traces carry the public-step samples only (OCF2, OCT1); above 100 Hz they show fewer than ten samples per period and the interface says so. **The Engine's own calculation and its measurements use the internal substeps and do not depend on the public sampling.** Nothing in this contract changes the public step.

### 3.5 The accumulated phase

The accumulated phase `θ` of each AC source (and of each three-phase source, which has one `θ` for the whole device) is **committed state**, stored at the **start of each public step** as a pair: a whole number of cycles (an unsigned 64-bit integer) and a fractional part of a cycle in `[0, 1)` (a double). It is independent of the number of internal substeps. Within a public step, the phase at the end of internal substep `j` of `N` is the stored pair advanced by `f · (j/N) · 1 ms` cycles, so the product is at most `f · 1 ms`, which is at most 0.4 cycles; at the end of the public step the stored pair is advanced by `f · 1 ms` and the whole cycles are carried. The sine is evaluated at `2π` times the fractional cycles plus `φ0 · π/180`.

Error budget, so the bound is not taken on trust: rounding the fractional part at each public step costs at most about `5.5e-17` cycles, which is `5.5e-8` cycles (`3.5e-7` rad) over `10^9` steps; and the representation of `f · 1 ms` (the number 0.001 is not exact) costs at most about `4e-8` cycles (`2.7e-7` rad) at 400 Hz over the same run. Together they stay below `1e-6` rad with a margin of a factor of about 1.5, which the implementation must keep.

Consequences: the substep count may differ between two updates without changing the waveform; and a run of `10^9` public steps (`10^6` s) keeps the phase error below `1e-6` radian at any allowed frequency. A conforming implementation may use another exact representation if it meets that bound.

The pair is runtime state. It rolls back with a refused update or a step that fails to solve, it is part of the state kept for the replacement of the newest step (so an update recomputes the newest public step from the pair at its start), and it is reset by a compile. It is never saved.

### 3.6 Live changes

A live update (stage-3-dynamics.md section 3.4) recomputes the **newest public step** from the committed state at its **start**; call that instant `t_c`. The rules for an AC source, in addition to the existing atomic refusal and current-step replacement:

- **`voltageRmsVolts`, `internalResistanceOhms`:** take effect from `t_c`; the phase is continuous.
- **`frequencyHz`:** the new `f` applies from `t_c`: the stored pair at `t_c` is unchanged and advances at the new frequency. The total phase `θ + φ0` is therefore continuous at `t_c`, and the waveform does not jump in phase (its slope changes).
- **`phaseOffsetDegrees`:** `φ0` changes from `t_c`, so the waveform jumps by the change of `φ0`. This is the user's explicit request.
- **`reversedSequence`** (three-phase sources): from `t_c` the phases `l2` and `l3` are exchanged; the waveforms jump.
- A change of `frequencyHz` of a source or a meter restarts the validity of every measurement it affects (section 5.4).
- A refused update (an invalid value, or a value that makes the solve fail) changes nothing, including the stored pair, as for every other update.

## 4. The three-phase source

### 4.1 Phase relations and sequence

Let `E` be the RMS value of a line-to-neutral voltage, `θ(t)` the accumulated phase of the device of section 3.5 (one `θ`, shared by all three phases), and `ϑ(t) = θ(t) + φ0 · π/180` the phase of `l1`. While the frequency has not been changed, `θ(t) = 2π f t`. In the positive sequence (`reversedSequence = 0`):

- `v_l1n(t) = √2 · E · sin(ϑ)`
- `v_l2n(t) = √2 · E · sin(ϑ − 2π/3)`
- `v_l3n(t) = √2 · E · sin(ϑ + 2π/3)`

With `reversedSequence = 1` the roles of `l2` and `l3` are exchanged. The line-to-line voltage is `√3 · E` RMS with `v_l1l2 = v_l1n − v_l2n` leading `v_l1n` by 30° in the positive sequence.

**Star.** Three windings are connected from `n` to `l1`, `l2`, `l3`, each `internalResistanceOhms` in series. The neutral is a real conductor: if `n` is not connected, the circuit is solved without it (a three-wire load), and the neutral point of an unbalanced load floats as physics says.

**Delta.** Three windings are connected `l1`–`l2`, `l2`–`l3`, `l3`–`l1`, each a voltage source of RMS `lineVoltageRmsVolts` with the same sequence rule, in series with `internalResistanceOhms`. The windings are driven so that the open-circuit line-to-line voltages are `v_l1l2 = √2 · E_l · sin(ϑ)`, `v_l2l3 = √2 · E_l · sin(ϑ − 2π/3)`, `v_l3l1 = √2 · E_l · sin(ϑ + 2π/3)`, which sum to zero at every instant.

### 4.2 One device

Each three-phase source is one Engine device with its terminals in the order of section 2.1; it is never built from independent sources, so the phase relations cannot be broken by the user. The device reports no operating state.

### 4.3 Results

The slot keys, units and signs are in section 6.2. Terminal currents are positive **entering the terminal**, and for the star source `I(l1) + I(l2) + I(l3) + I(n) = 0` at every step to the solver's tolerance.

## 5. Measurements

### 5.1 The window

A measuring device has a parameter `frequencyHz`, and its **window** is one period `T = 1/f`, the interval `(t − T, t]` ending at the current simulation time `t`. A measuring device never infers `f` from the circuit: it measures what its parameter says, so a wrong value gives a wrong, stated measurement, and the interface shows the parameter beside the reading. The window uses the internal-substep samples, not the public-step samples.

### 5.2 Definitions

For a signal `x(t)` (a voltage or a current as the section 6.2 signs say) the Engine reports, over the window:

- **mean** `X_avg = (1/T) ∫ x dt`;
- **RMS** `X_rms = √((1/T) ∫ x² dt)`;
- **peak** `X_pk = max |x|` over the samples of the window;
- **frequency** (the voltmeter and the ammeter only): the reciprocal of the interval between the last two rising crossings of the level `X_avg` found inside the last two windows, with the crossing time taken by linear interpolation inside its internal step; reported valid only when two such crossings exist.

For the wattmeter, with `v` the voltage of the voltage circuit (`V(v1) − V(v2)`) and `i` the current of the current circuit (entering `i1`):

- **real power** `P = (1/T) ∫ v · i dt`;
- **apparent power** `S = V_rms · I_rms`;
- **reactive power** `Q = sgn(C) · √max(S² − P², 0)`, where `C = (1/T) ∫ v(t − T/4) · i(t) dt`; for a sinusoid this is the usual `Q`, positive for an inductive load (current lagging voltage) and negative for a capacitive one; for a distorted signal `|Q|` also contains the distortion power, which this contract does not separate;
- **power factor** `PF = P / S` (0 when `S` is 0), signed like `P`;
- **phase angle** `φ = atan2(Q, P)` in degrees, in the range -180 to 180, positive for a lagging current. For a sinusoid it is the angle by which the current lags the voltage.

Load convention: `P > 0` when net power flows from the `i1` and `v1` side into the `i2` and `v2` side, with the current entering `i1`, leaving `i2`, and the voltage taken as `V(v1) − V(v2)`; a source connected to the `i1` and `v1` side and a load on the other side give `P > 0`.

The integrals are the mathematical definitions. An implementation may compute them by any deterministic method; it must match the definition to the tolerance of section 10.1.

### 5.3 Quantities per device

| Device | Reported over the window |
|---|---|
| `ac_voltmeter` | instantaneous voltage and current, RMS, mean, peak, frequency |
| `ac_ammeter` | instantaneous voltage and current, RMS, mean, peak, frequency |
| `wattmeter` | instantaneous voltage of its voltage circuit and current of its current circuit, V RMS, I RMS, P, Q, S, PF, φ |

### 5.4 Validity

Every windowed quantity has a **validity flag** reported with it (section 6.2). A measurement is valid when the device's window is fully inside a time over which nothing it depends on changed its frequency or its meter frequency: it is invalid from the start of a run, and after a live change of the device's `frequencyHz` or of any AC source's `frequencyHz`, until a full period `T` has elapsed. While it is invalid, the numeric slots report 0 and the interface shows no value. A live change of any other parameter does not invalidate: the measurement is of the actual signal, which the user changed. The frequency measurement has its own flag (section 5.2).

### 5.5 State, atomicity, and traces

The accumulators of the windows are part of the committed state of a run: they roll back with a refused update and with a step that fails to solve, they are recomputed with the newest step when an update replaces it, and they are reset by a compile. No measurement value is saved in a project. Traces of the measurement slots follow the existing rules (stage-3-dynamics.md section 7): the newest sample is replaced by a current-step update, earlier samples are never changed.

## 6. Results, compatibility, and generated types

### 6.1 New values

Engine-owned and generated for TypeScript (`src/generated/**` is never edited by hand):

- `Unit`: `hertz`, `degree`, `watt`, `volt_ampere`, `volt_ampere_reactive`. The frequency and angle parameters use `hertz` and `degree`.
- `ResultQuantity`: `rmsVoltage`, `meanVoltage`, `peakVoltage`, `rmsCurrent`, `meanCurrent`, `peakCurrent`, `frequency`, `realPower`, `reactivePower`, `apparentPower`, `powerFactor`, `phaseAngle`, `valid` (used by both `/valid` and `/frequency_valid`).
- `ResultSignConvention`: `unsigned` (RMS, peak, frequency, apparent power), `loadConvention` (P, Q, PF, φ, with the signs of section 5.2), `flag` (the validity slots, 0 or 1). The existing `voltagePin1MinusPin2` and `currentIntoPin1` apply to every instantaneous and mean voltage and current slot, by the rule of the Stage 7 transistor for multi-terminal devices: the first-named terminal minus the second for a voltage (`/l1/voltage` is `V(l1) − V(n)`, `/l12/voltage` is `V(l1) − V(l2)`), and positive into the named terminal for a current (`/n/current`, `/l1/current`).
- `PinId`: `l1`, `l2`, `l3`, `n`, `v1`, `v2`, `i1`, `i2`.
- `ParameterId`: `voltageRmsVolts`, `phaseVoltageRmsVolts`, `lineVoltageRmsVolts`, `frequencyHz`, `phaseOffsetDegrees`, `reversedSequence`. Existing IDs are reused where the meaning is the same (`internalResistanceOhms`, `inputResistanceOhms`, `burdenResistanceOhms`).
- `ComponentTypeId`: `ac_voltage_source`, `three_phase_source_star`, `three_phase_source_delta`, `ac_voltmeter`, `ac_ammeter`, `wattmeter`.

### 6.2 Result slots

Slots are listed in layout order per component; keys use the existing forms (`<id>:voltage` and `<id>:current` for the standard pair, `<id>/<name>` otherwise).

| Device | Slots, in order |
|---|---|
| `ac_voltage_source` | `:voltage` (V, `V(pin1) − V(pin2)`), `:current` (A, into `pin1`) |
| `three_phase_source_star` | `/l1/voltage`, `/l1/current`, `/l2/voltage`, `/l2/current`, `/l3/voltage`, `/l3/current`, `/n/current`. Voltages are `V(lk) − V(n)`; currents enter the terminal. |
| `three_phase_source_delta` | `/l12/voltage`, `/l23/voltage`, `/l31/voltage` (`V(l1) − V(l2)` and so on), `/l1/current`, `/l2/current`, `/l3/current` (entering the terminal) |
| `ac_voltmeter` | `:voltage`, `:current`, `/rms_voltage`, `/mean_voltage`, `/peak_voltage`, `/frequency` (Hz), `/frequency_valid`, `/valid` |
| `ac_ammeter` | `:voltage`, `:current`, `/rms_current`, `/mean_current`, `/peak_current`, `/frequency` (Hz), `/frequency_valid`, `/valid` |
| `wattmeter` | `/voltage` (V, `V(v1) − V(v2)`), `/current` (A, entering `i1`), `/rms_voltage`, `/rms_current`, `/real_power` (W), `/reactive_power` (var), `/apparent_power` (VA), `/power_factor`, `/phase_angle` (degrees), `/valid` |

`valid` and `frequency_valid` are 0 or 1 and have the unit `dimensionless` and the sign convention `flag`. The interface shows a windowed value only when its flag is 1.

### 6.3 No new frame kind and no format change

The result layout is the existing `ResultLayout` with more slots and the new enum values; OCF2 frames, OCT1 traces, and the metadata of compile, update and snapshot are unchanged. The device-state records are unchanged: none of the new devices has a discrete state. The finite-value, count, length, and layout checks apply to the new slots as to every slot.

### 6.4 Project format and older readers

A project is a version 2 schematic with six more component types. No new field is added to the schematic. An application that does not know the new types rejects the file through its existing unknown-type refusal; there is no downgrade.

## 7. Diagnostics, live updates, and determinism

- **Codes.** No new code. An invalid parameter, a value outside its range, a frequency outside 1 to 400 Hz, or a zero internal resistance of the delta source is `InvalidParameter`, referencing the component. A loop of ideal sources (an AC source or a star winding with zero internal resistance in a closed loop with another ideal source) is `ShortCircuit`, by the rule of the DC source, whatever the values. A floating or unreferenced node is reported as before.
- **A run that fails.** A step that fails to solve stops the run with the diagnostic and the notice of stage-7 section 6.3, and commits nothing, exactly as for a DC circuit.
- **Refusals.** Refused updates are atomic (section 3.6 and section 5.5).
- **Determinism.** Identical schematic, parameters, and operator events produce identical bits in one environment; across CPU architectures or operating systems the results are held to the tolerances of section 10, never to stored bits.

## 8. Symbols and labels

### 8.1 Symbols

Symbol geometry is in grid units, local to the component at rotation 0, with the pin positions below; the Application workstream (issue #170) draws the picture. The final pictures, and the choice between the IEC, GOST and ANSI drawings, are decided later by the project owner and change no pin position.

| Type | Pins and local positions |
|---|---|
| `ac_voltage_source` | `pin1` (-1, 0), `pin2` (1, 0) |
| `three_phase_source_star` | `l1` (2, -2), `l2` (2, 0), `l3` (2, 2), `n` (-2, 0) |
| `three_phase_source_delta` | `l1` (2, -2), `l2` (2, 0), `l3` (2, 2) |
| `ac_voltmeter`, `ac_ammeter` | `pin1` (-2, 0), `pin2` (2, 0), as the DC meters |
| `wattmeter` | `v1` (-2, -1), `v2` (2, -1), `i1` (-2, 1), `i2` (2, 1) |

The symbol of the AC source carries a sine mark; a three-phase source shows its terminal names; the meters carry a mark for AC. A symbol shows nothing about the instantaneous value.

### 8.2 Labels

New components are numbered per prefix as the editor already does: `V` for the AC source (shared with the DC source), `G` for a three-phase source, and `PV`, `PA` and `PW` for the meters. The existing DC meters use `VM` and `AM`; the new prefixes are proposals (section 12) and may be aligned with them.

### 8.3 Catalog order

The new types are listed after the existing ones, in the order of section 1.2.

## 9. What is limited or deferred

- **Coils on an alternating current.** The relay and contactor models of Stages 4 and 6 compare the **magnitude of the coil current** with pickup and dropout thresholds, which have hysteresis. Driven by an alternating current, that magnitude crosses the thresholds twice per period, so the contacts toggle between steps. **No protection applies:** `EventChatter` is raised only for re-solves at one instant (a repeated discrete state or too many transitions at the same time), not for a contact that changes state in successive steps, so the run continues with a flickering contact. This stage does not change the coil models: the behavior of an existing coil on an AC source is **undefined**, and Stage 10 defines an AC-aware coil. The acceptance record says so.
- **Measurement scope.** No phase angle against a reference signal, no three-phase total power, no harmonics, no flicker, no DC component of a sinusoidal source.
- **Assignments.** The assignment format, checks, and fault kinds do not accept the new devices; an assignment check that meets one reports the existing unsupported-device result. Stage 13 extends assignments.
- **Frequencies** outside 1 to 400 Hz, and a changing frequency over time (a sweep).
- **Source impedance** other than a series resistance (no source inductance).

## 10. Tolerances and canonical oracles

### 10.1 Tolerances

These are the canonical tolerances, to be confirmed against the reference data of issue #165 and then fixed; they may be tightened by the references but not loosened without a recorded decision.

| Quantity | Tolerance |
|---|---|
| Steady-state amplitude (RMS) of a linear circuit against the closed form, 50 Hz and 60 Hz | 5e-3 relative |
| Steady-state phase against the closed form | 0.3 degree |
| Start-up transient samples against the closed form | 1e-2 of the source amplitude, absolute |
| Measured RMS, mean, peak, P, Q, S of a sinusoid after one period | 2e-3 relative of the larger of the reference and the signal's own RMS scale |
| Measured power factor | 2e-3 absolute |
| Measured frequency | 1e-3 relative |
| Three-phase balance: sum of the three line currents of a balanced load without neutral | 1e-9 A absolute plus 1e-6 of the largest line current |
| Kirchhoff current sum at every node and the power balance, at each step | 1e-9 A and 1e-8 W absolute |

The amplitude and phase tolerances above are those of 50 Hz and 60 Hz. A circuit at 400 Hz (about 0.8 percent of the amplitude at the worst case) gets its own tolerance, fixed in issue #165 from the reference data.

### 10.2 Canonical circuits

Each has an independently computed answer (the closed form of the references, and an ngspice run with the version pinned, as in stage-7-references.md), the exact observation steps, and a wrong variant that must give a distinct answer.

1. **Series RC, RL and RLC on a sinusoidal source**, 50 Hz and 60 Hz: the steady-state amplitude and phase, and the start-up transient, with the source at phase 0 and at phase 90 degrees.
2. **A wattmeter on an RL load** at 50 Hz: `P`, `Q`, `S`, `PF`, `φ`; the same load with the wattmeter's current circuit reversed (wrong variant: the sign of `P` flips).
3. **Balanced three-phase star load** (three equal resistors, then three equal RL loads) on the star source with and without the neutral wire: line currents `I = E / |Z|`, a neutral current of zero, `P_total = 3 E I cos φ`.
4. **Balanced delta load on the delta source** and on the star source: line current `√3` times the phase current, and the phase angles of section 4.1.
5. **Unbalanced star load** with and without the neutral: the neutral current as the sum of the line currents with the neutral, and the floating neutral-point voltage without it.
6. **The reversed sequence** (wrong variant): the same load with `reversedSequence = 1` gives the swapped phase order of the currents.
7. **A frequency change** at a chosen step: the waveform stays continuous, and the measurements are invalid for one period and then valid.
8. **A refused update** (a frequency of 500 Hz) during a run: nothing changes, and the next valid update is accepted.

## 11. Verification and ownership

- Reference data and the comparison with ngspice: issue #165.
- The sinusoidal source: issue #166. The three-phase sources: issue #167. The measurements: issue #168.
- The application bridge and persistence: issue #169. Symbols, inspector inputs and readouts: issue #170.
- Acceptance across the application and the three platforms: issue #171, in `docs/acceptance/stage-9-three-phase-foundation.md`.

## 12. Decisions that need the owner's approval

The following are proposals made for this contract. They are the first things to confirm in review.

1. The frequency range 1 to 400 Hz and the default 50 Hz, the default RMS values (230 V phase, 400 V line), and the default series resistance 0.1 ohm.
2. Meters carry their own `frequencyHz` and measure over one period of it, instead of inferring the frequency from the circuit.
3. Three meters (voltmeter, ammeter, wattmeter) as separate catalog types, rather than extra slots on the existing Stage 4 meters, so that existing layouts and golden data do not change.
4. Validity as a numeric 0 or 1 slot, rather than a new state record.
5. Star and delta as two catalog types, and `reversedSequence` as a parameter.
6. The limit in section 9 on coils driven by an alternating current until Stage 10.
7. The label prefixes `PV`, `PA`, `PW` and `G` (section 8.2).
8. The accumulated-phase representation of section 3.5 and the frequency-change rule of section 3.6 (phase continuous, slope changes).
