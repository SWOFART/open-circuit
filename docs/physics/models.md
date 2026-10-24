# Circuit laws and component model inventory

This document distinguishes the equations used by the simulator from the particular model
chosen for a component. Its Stage 1 details summarize the [current contract](../contracts/stage-1.md)
and [solver notes](../solver/stage-1-resistive.md); those documents remain authoritative for
current implementation behavior.

## Shared conventions

All internal quantities use SI units. For a two-terminal component, voltage is
`v = V(pin1) - V(pin2)`, current is positive entering `pin1`, and power is `p = v * i`.
Under this passive sign convention, positive power is absorbed and negative power is supplied.
These conventions match the Stage 1 contract.

## Equations used by the current engine

| Rule or formulation | Relation | Use and assumptions |
|---|---|---|
| Kirchhoff's current law (KCL) | Algebraic sum of branch currents at a node is zero. | Expresses charge conservation for the lumped network. The selected sign for each branch current must be applied consistently. |
| Kirchhoff's voltage law (KVL) | Algebraic sum of voltage rises and drops around a closed loop is zero. | Applies to the lumped-circuit abstraction; field coupling that matters to a circuit must be represented by an explicit model such as a coupled inductor. |
| Ohm's law | `v = R * i`, or `i = v / R`. | Stage 1's linear resistor uses finite `R > 0`; temperature dependence and parasitics are not modeled. |
| Passive sign convention | `p = v * i`. | A useful conservation check is that the signed powers of a complete circuit balance, within numerical tolerance. |
| Modified nodal analysis (MNA) | KCL equations plus device equations form a linear or nonlinear algebraic system. | Node voltages are unknowns; ideal voltage constraints introduce branch-current unknowns. MNA is an equation-assembly method, not a component law. |

KCL and KVL are foundational network equations; Ohm's law supplies the Stage 1 resistor
constitutive relation. The MNA formulation and its use of branch currents for voltage sources
are described in the original [Ho, Ruehli, and Brennan paper](https://doi.org/10.1109/TCS.1975.1084079).

## Implemented models

| Model | Status | Constitutive behavior | Parameters and boundaries | Deliberately omitted effects |
|---|---|---|---|---|
| Linear resistor | Implemented in Stage 1 | `i = (V1 - V2) / R` | `resistanceOhms > 0`; default `1000 Ω`. | Tolerance, temperature coefficient, noise, parasitic inductance/capacitance, thermal behavior. |
| Constant independent voltage source | Implemented in Stage 1 | For `Rs > 0`, terminal relation is `v = Vs + Rs * i`, with current positive into `pin1`. For `Rs = 0`, enforce `v = Vs` and solve its branch current as an MNA unknown. | `voltageVolts` is finite and signed; `internalResistanceOhms >= 0`; defaults `5 V` and `0.1 Ω`. | Time-varying waveform, battery chemistry, state of charge, polarization, temperature, and current limits. |
| Reference marker | Implemented as topology/solver metadata | Selects a shared reference potential; it is not an energy-storing or dissipative device. | Explicit markers join marked nets; otherwise each island gets its own deterministic numerical reference. | A symbol or marker does not represent an earth connection unless the model explicitly says so. |

For finite source resistance, the Engine uses the Norton-equivalent MNA stamp while reporting
terminal voltage and current using the pin convention above. Exact failure cases for ideal
sources and the source defaults are specified in the Stage 1 contract.

## Planned model families

These entries are a research roadmap, not a claim that the models are implemented. Stage
ordering follows [`ARCHITECTURE.md`](../../ARCHITECTURE.md).

| Family | Planned relation or state | Current decisions and open questions |
|---|---|---|
| Capacitor | For constant `C`, `q = C * v` and `i = C * dv/dt`; state can be capacitor voltage or charge. Ideal stored energy is `E = 1/2 * C * v^2`. | Agreed Stage 3 baseline: finite `C > 0`; optional finite signed initial voltage. Without a user value, initialize from the DC operating point. Zero and negative capacitance are invalid for this component type. Start with an ideal linear model; add ESR and leakage only in a selected non-ideal model. |
| Inductor | For constant `L`, flux linkage `lambda = L * i` and `v = L * di/dt`; state can be inductor current or flux linkage. Ideal stored energy is `E = 1/2 * L * i^2`. | Agreed Stage 3 baseline: finite `L > 0`; optional finite signed initial current. Without a user value, initialize from the DC operating point. Zero and negative inductance are invalid for this component type. Start with an ideal linear model; add winding resistance only in a selected non-ideal model. |
| Dependent linear sources | Voltage or current is controlled by another voltage or current. | Pin/control polarity, dimensionless versus dimensional gain, well-posedness, and matrix singularities. |
| Switches and relays | A discrete state changes the connected electrical model; a relay adds a coil model, thresholds, and hysteresis. | Event ordering, contact resistance, initial state, chatter limits, and solver retry behavior. Architecture places relay/event behavior after C/L dynamics. |
| Diodes | Nonlinear current-voltage relation; practical models may also include charge storage and series resistance. | Selected equation/model level, temperature dependence, limiting and convergence strategy, and parameter provenance. |
| BJTs and MOSFETs | Nonlinear semiconductor terminal relations with multiple coupled terminals and internal state/charge effects in more detailed models. | Named model family/version, required parameters, supported operating regions, convergence, thermal assumptions, and compatibility with imported model cards. Architecture defers these to the extended-model stage. |
| Coupled inductors and transformers | Multiport flux/current relations with a mutual-inductance matrix. | Dot convention, matrix passivity/positive-semidefinite constraints, initial magnetic state, leakage, and winding losses. |

## Agreed C/L model baseline

The following choices apply to the planned Stage 3 capacitor and inductor models:

- The ideal linear capacitance and inductance must be finite and strictly
  positive. Zero-value and negative-value cases are rejected; ideal open and
  short behavior must use explicit topology/component semantics instead.
- The capacitor's dynamic state is its voltage (equivalently, charge for
  constant capacitance); the inductor's dynamic state is its current
  (equivalently, flux linkage for constant inductance).
- With no explicit user initial condition, initialize from the DC operating
  point: capacitors are open and inductors are short in that solve.
- An explicit initial capacitor voltage or inductor current overrides that
  component's operating-point state. The specified state need not be a DC
  equilibrium. Unspecified C/L states retain their operating-point values.
- Check the combined initial state against the circuit's ideal algebraic
  constraints. Return a structured diagnostic when those constraints cannot
  be satisfied; do not silently rewrite user values or model impulse current
  or voltage.
- Initial conditions supplied by the user are part of the schematic input.
  The state subsequently computed by the simulator is runtime state and is
  not saved into the schematic.
- The first supported physical model is ideal and linear. A simplified
  non-ideal model may add capacitor ESR/leakage and inductor winding DCR.
  Later model tiers may add capacitor ESL and parameter dependence on voltage,
  temperature, or frequency; inductor winding capacitance, saturation,
  frequency-dependent losses, and thermal behavior.

### Known-answer fixtures

All values below use SI internally. RC and RL step tests use a 10 ms time
constant so the existing 1 ms base step gives ten steps per time constant.
Analytical references are:

| Fixture | Configuration and reference relation | At 10 ms | At 20 ms | At 30 ms |
|---|---|---:|---:|---:|
| RC charge | Ideal 10 V source (internal resistance 0 Ω), R = 1 kΩ, C = 10 µF, initial capacitor voltage 0 V; v_C = 10(1 − e^(−t/(RC))) | 6.321206 V | 8.646647 V | 9.502129 V |
| RC discharge | R = 1 kΩ, C = 10 µF, initial capacitor voltage 10 V; v_C = 10e^(−t/(RC)) | 3.678794 V | 1.353353 V | 0.497871 V |
| RL current rise | Ideal 10 V source (internal resistance 0 Ω), R = 10 Ω, L = 100 mH, initial inductor current 0 A; i_L = 1(1 − e^(−tR/L)) | 0.632121 A | 0.864665 A | 0.950213 A |
| RL current decay | R = 10 Ω in parallel with L = 100 mH, initial inductor current 1 A; i_L = e^(−tR/L) | 0.367879 A | 0.135335 A | 0.049787 A |

Separate DC operating-point fixtures verify the default initializer:

- An ideal 5 V source, 1 kΩ resistor, and 10 µF capacitor in series to ground yield
  5 V across the capacitor and zero branch current.
- An ideal 5 V source, 1 kΩ resistor, and 1 H inductor in series to ground yield
  5 mA through the inductor and zero inductor voltage.

The lossless parallel LC fixture uses C = 100 µF, L = 100 mH, initial
capacitor voltage 10 V, and initial inductor current 0 A. Its reference period
is T = 2π√(LC) = 19.8691765 ms, peak inductor current is 0.316228 A,
and total ideal stored energy is 5 mJ. Check both period and energy; an energy
check alone does not detect phase error.

For trapezoidal integration without an event-recovery step at initialization,
the predicted LC period errors are:

| Internal step | Predicted period | Period error |
|---:|---:|---:|
| 1 ms | 20.033663 ms | +0.83% |
| 0.5 ms | 19.910502 ms | +0.21% |
| 0.25 ms | 19.879521 ms | +0.052% |

Thus, the 0.25% period-error target for this fixture requires an internal
step no larger than 0.5 ms. This target is fixture-specific, not a global
accuracy claim. The Engine must retain its 1 ms base/API step and use smaller
internal steps when required by the accepted accuracy checks; the general
sub-step selection rule remains to be specified before implementation.

### Real-part profiles

Keep generic electrical models separate from optional manufacturer-specific
datasheet profiles. A profile records manufacturer, part number, datasheet
URL and revision, and the conditions attached to each reported value.

- Capacitor profile fields may include nominal capacitance and tolerance with
  measurement conditions, rated voltage, polarity, temperature range, ESR,
  ESL, leakage, and ripple-current rating.
- Inductor profile fields may include nominal inductance and tolerance with
  measurement conditions, DCR, saturation-current criterion, RMS current and
  temperature-rise condition, self-resonant frequency, Q, and temperature
  range.
- Ratings and catalog metadata do not change electrical behavior unless a
  selected component model explicitly consumes the corresponding parameter.
  Use SI values in the Engine; preserve frequency, temperature, voltage-bias,
  and other relevant measurement conditions in the profile.

## Model record required for a new device

Before implementing a new model, record:

1. Stable component type ID, pins, pin orientation, and current/voltage sign conventions.
2. Constitutive equation(s), state variables, and any algebraic constraints.
3. Parameters with SI units, allowed domain, defaults, optionality, and source/version.
4. Model fidelity: ideal, simplified non-ideal, or a named standard/simulator model.
5. Initial-condition requirements, supported analysis modes, and numerical solver needs.
6. Validity limits, omitted physical effects, singular cases, and expected diagnostics.
7. At least one analytical known-answer test and any external golden fixture needed for the
   selected model.

Do not give a model a generic “all characteristics” claim. For real parts, ratings,
tolerances, temperature ranges, and model parameters are manufacturer- and part-specific.
Record those only as a named datasheet profile with its exact source and conditions; keep
the generic component model separate.

## Source register

- MIT OpenCourseWare, 6.071J, Spring 2006, [Kirchhoff's Laws](https://ocw.mit.edu/courses/6-071j-introduction-to-electronics-signals-and-measurement-spring-2006/resources/03-kirchhoff1/): KCL/KVL and introductory DC components.
- Ho, C. W., Ruehli, A. E., and Brennan, P. A., [“The Modified Nodal Approach to Network Analysis”](https://doi.org/10.1109/TCS.1975.1084079), 1975: MNA equation assembly and branch-current variables.
- BIPM, [SI Brochure](https://www.bipm.org/en/web/guest/publications/si-brochure), 9th edition updated 2026: SI system and unit definitions.
- Modelica Association, [Modelica Standard Library 4.0.0, Electrical.Analog.Basic](https://doc.modelica.org/Modelica%204.0.0/Resources/helpDymola/Modelica_Electrical_Analog_Basic.html): ideal resistor, capacitor, inductor, and other basic component interfaces/equations.
- ngspice project, [official manuals](https://ngspice.sourceforge.io/docs.html): simulator-specific device model behavior and parameters. Use the manual version relevant to any future compatibility claim; do not treat SPICE defaults as universal physical constants.
- TDK Electronics, [general technical information for aluminum electrolytic capacitors](https://www.tdk-electronics.tdk.com/download/185386/5f33d2619fa73419e2a4af562122e90c/pdf-generaltechnicalinformation.pdf): ESR, ESL, impedance, and frequency/temperature dependencies.
- Coilcraft, [current and temperature ratings](https://www.coilcraft.com/en-us/resources/application-notes/current-and-temperature-ratings/): definitions and conditions for saturation and heating current ratings.
