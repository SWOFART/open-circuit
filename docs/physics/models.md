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

| Rule or formulation           | Relation                                                                         | Use and assumptions                                                                                                                                      |
| ----------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Kirchhoff's current law (KCL) | Algebraic sum of branch currents at a node is zero.                              | Expresses charge conservation for the lumped network. The selected sign for each branch current must be applied consistently.                            |
| Kirchhoff's voltage law (KVL) | Algebraic sum of voltage rises and drops around a closed loop is zero.           | Applies to the lumped-circuit abstraction; field coupling that matters to a circuit must be represented by an explicit model such as a coupled inductor. |
| Ohm's law                     | `v = R * i`, or `i = v / R`.                                                     | Stage 1's linear resistor uses finite `R > 0`; temperature dependence and parasitics are not modeled.                                                    |
| Passive sign convention       | `p = v * i`.                                                                     | A useful conservation check is that the signed powers of a complete circuit balance, within numerical tolerance.                                         |
| Modified nodal analysis (MNA) | KCL equations plus device equations form a linear or nonlinear algebraic system. | Node voltages are unknowns; ideal voltage constraints introduce branch-current unknowns. MNA is an equation-assembly method, not a component law.        |

KCL and KVL are foundational network equations; Ohm's law supplies the Stage 1 resistor
constitutive relation. The MNA formulation and its use of branch currents for voltage sources
are described in the original [Ho, Ruehli, and Brennan paper](https://doi.org/10.1109/TCS.1975.1084079).

## Implemented models

| Model                               | Status                                  | Constitutive behavior                                                                                                                                                   | Parameters and boundaries                                                                                | Deliberately omitted effects                                                                              |
| ----------------------------------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Linear resistor                     | Implemented in Stage 1                  | `i = (V1 - V2) / R`                                                                                                                                                     | `resistanceOhms > 0`; default `1000 Ω`.                                                                  | Tolerance, temperature coefficient, noise, parasitic inductance/capacitance, thermal behavior.            |
| Constant independent voltage source | Implemented in Stage 1                  | For `Rs > 0`, terminal relation is `v = Vs + Rs * i`, with current positive into `pin1`. For `Rs = 0`, enforce `v = Vs` and solve its branch current as an MNA unknown. | `voltageVolts` is finite and signed; `internalResistanceOhms >= 0`; defaults `5 V` and `0.1 Ω`.          | Time-varying waveform, battery chemistry, state of charge, polarization, temperature, and current limits. |
| Ideal linear capacitor              | Implemented in Stage 3                  | `q = C * v`; `i = C * dv/dt`, integrated with the trapezoidal companion model.                                                                                          | Finite `capacitanceFarads > 0`; default `10 µF`; optional finite signed initial voltage.                 | ESR, leakage, ESL, voltage/temperature dependence, and manufacturer-specific profiles.                    |
| Reference marker                    | Implemented as topology/solver metadata | Selects a shared reference potential; it is not an energy-storing or dissipative device.                                                                                | Explicit markers join marked nets; otherwise each island gets its own deterministic numerical reference. | A symbol or marker does not represent an earth connection unless the model explicitly says so.            |

For finite source resistance, the Engine uses the Norton-equivalent MNA stamp while reporting
terminal voltage and current using the pin convention above. Exact failure cases for ideal
sources and the source defaults are specified in the Stage 1 contract.

## Planned model families

These entries are a research roadmap, not a claim that the models are implemented. Stage
ordering follows [`ARCHITECTURE.md`](../../ARCHITECTURE.md).

| Family                             | Planned relation or state                                                                                                                                       | Current decisions and open questions                                                                                                                                                                                                                                                                                                                                                 |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Inductor                           | For constant `L`, flux linkage `lambda = L * i` and `v = L * di/dt`; state can be inductor current or flux linkage. Ideal stored energy is `E = 1/2 * L * i^2`. | Agreed Stage 3 baseline: finite `L > 0`; optional finite signed initial current. Without a user value, initialize from the DC operating point. Zero and negative inductance are invalid for this component type. Start with an ideal linear model; add winding resistance only in a selected non-ideal model. Specified in the [Stage 3 contract](../contracts/stage-3-dynamics.md). |
| Dependent linear sources           | Voltage or current is controlled by another voltage or current.                                                                                                 | Pin/control polarity, dimensionless versus dimensional gain, well-posedness, and matrix singularities.                                                                                                                                                                                                                                                                               |
| Switches and relays                | A discrete state changes the connected electrical model; a relay adds a coil model, thresholds, and hysteresis.                                                 | Event ordering, contact resistance, initial state, chatter limits, and solver retry behavior. Architecture places relay/event behavior after C/L dynamics.                                                                                                                                                                                                                           |
| Diodes                             | Nonlinear current-voltage relation; practical models may also include charge storage and series resistance.                                                     | Selected equation/model level, temperature dependence, limiting and convergence strategy, and parameter provenance.                                                                                                                                                                                                                                                                  |
| BJTs and MOSFETs                   | Nonlinear semiconductor terminal relations with multiple coupled terminals and internal state/charge effects in more detailed models.                           | Named model family/version, required parameters, supported operating regions, convergence, thermal assumptions, and compatibility with imported model cards. Architecture defers these to the extended-model stage.                                                                                                                                                                  |
| Coupled inductors and transformers | Multiport flux/current relations with a mutual-inductance matrix.                                                                                               | Dot convention, matrix passivity/positive-semidefinite constraints, initial magnetic state, leakage, and winding losses.                                                                                                                                                                                                                                                             |

## Agreed C/L model baseline

The following choices apply to the Stage 3 capacitor and inductor models, which
the [Stage 3 contract](../contracts/stage-3-dynamics.md) turns into a schematic,
Engine, and oscilloscope specification:

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
  equilibrium. Unspecified C/L states retain their operating-point values: the
  values from the DC solve with every capacitor open and every inductor short,
  whatever the other components specify. A circuit with no DC operating point
  therefore needs every state explicit. The procedure and its consequences for
  mixed circuits are in
  [`stage-3-dynamics.md`](../contracts/stage-3-dynamics.md) section 3.
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

| Fixture          | Configuration and reference relation                                                                                       |   At 10 ms |   At 20 ms |   At 30 ms |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------- | ---------: | ---------: | ---------: |
| RC charge        | Ideal 10 V source (internal resistance 0 Ω), R = 1 kΩ, C = 10 µF, initial capacitor voltage 0 V; v_C = 10(1 − e^(−t/(RC))) | 6.321206 V | 8.646647 V | 9.502129 V |
| RC discharge     | R = 1 kΩ, C = 10 µF, initial capacitor voltage 10 V; v_C = 10e^(−t/(RC))                                                   | 3.678794 V | 1.353353 V | 0.497871 V |
| RL current rise  | Ideal 10 V source (internal resistance 0 Ω), R = 10 Ω, L = 100 mH, initial inductor current 0 A; i_L = 1(1 − e^(−tR/L))    | 0.632121 A | 0.864665 A | 0.950213 A |
| RL current decay | R = 10 Ω in parallel with L = 100 mH, initial inductor current 1 A; i_L = e^(−tR/L)                                        | 0.367879 A | 0.135335 A | 0.049787 A |

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
| ------------: | ---------------: | -----------: |
|          1 ms |     20.033663 ms |       +0.83% |
|        0.5 ms |     19.910502 ms |       +0.21% |
|       0.25 ms |     19.879521 ms |      +0.052% |

Thus, the 0.25% period-error target for this fixture requires an internal
step no larger than 0.5 ms. This target is fixture-specific, not a global
accuracy claim. The Engine must retain its 1 ms base/API step and use smaller
internal steps when required by the accepted accuracy checks; the general
sub-step selection rule is in
[`stage-3-dynamics.md`](../contracts/stage-3-dynamics.md) section 4.2.

### Stage 3 acceptance

This section is the numerical acceptance for the fixtures above. The contract
that defines the models, the initialization, the sub-step rule, and the traces
is [`stage-3-dynamics.md`](../contracts/stage-3-dynamics.md). Every fixture
runs through the real Rust Engine from a saved schematic, with the public 1 ms
step, and must pass for **any** internal substep count the contract's rule can
choose. The reference frame is Stage 1's: `v = V(pin1) − V(pin2)`, current
positive entering `pin1`, and the negative pin of each source (or the marked
node) is the explicit reference.

#### Fixture circuits

| Fixture                      | Circuit (all pins in the order `pin1`, `pin2`)                                                                                      | Initial condition               | Quantity compared          |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | -------------------------- |
| RC charge                    | V1 ideal 10 V (`internalResistanceOhms` 0): `pin1` to node `in`, `pin2` to ground. R1 1 kΩ: `in` to `out`. C1 10 µF: `out`, ground. | `initialVoltageVolts` 0         | C1 `voltage`               |
| RC discharge                 | C1 10 µF and R1 1 kΩ, both from node `out` (`pin1`) to ground (`pin2`). No source.                                                  | `initialVoltageVolts` 10        | C1 `voltage`               |
| RL rise                      | V1 ideal 10 V: `in`, ground. R1 10 Ω: `in` to `mid`. L1 100 mH: `mid`, ground.                                                      | `initialCurrentAmps` 0          | L1 `current`               |
| RL decay                     | R1 10 Ω and L1 100 mH, both from node `a` (`pin1`) to ground (`pin2`). No source.                                                   | `initialCurrentAmps` 1          | L1 `current`               |
| DC initialization, capacitor | V1 ideal 5 V: `in`, ground. R1 1 kΩ: `in` to `out`. C1 10 µF: `out`, ground.                                                        | none                            | C1 `voltage`, C1 `current` |
| DC initialization, inductor  | V1 ideal 5 V: `in`, ground. R1 1 kΩ: `in` to `mid`. L1 1 H: `mid`, ground.                                                          | none                            | L1 `current`, L1 `voltage` |
| Lossless LC                  | C1 100 µF and L1 100 mH, both from node `n` (`pin1`) to ground (`pin2`). No source.                                                 | C1 10 V, L1 0 A (both explicit) | C1 `voltage`, L1 `current` |

Each circuit has one reference marker on the ground node. The saved schematics
and the expected-value files are added with the Engine issues (#45, #46) under
`tests/golden/stage-3-*`. They must contain exactly these components, values,
and initial conditions.

#### Signs and values at step 0

Checked against the Stage 1 convention before the Engine exists:

| Fixture                      | Step 0 values                                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------- |
| RC charge                    | `v_C = 0 V`, `i_C = +10 mA` (the source charges `pin1` positive)                                            |
| RC discharge                 | `v_C = +10 V`, `i_C = −10 mA` (the capacitor discharges through R1, so the current enters `pin1` negative)  |
| RL rise                      | `i_L = 0 A`, `v_L = +10 V`                                                                                  |
| RL decay                     | `i_L = +1 A`, `v_L = −10 V` (node `a` is at −10 V: R1 carries −1 A, since `i_R + i_L = 0`)                  |
| DC initialization, capacitor | `v_C = 5 V`, `i_C = 0 A`                                                                                    |
| DC initialization, inductor  | `i_L = 5 mA`, `v_L = 0 V`                                                                                   |
| Lossless LC                  | `v_C = +10 V`, `i_C = 0 A`, `i_L = 0 A`, `v_L = +10 V`; then `i_L(t) = 0.316228 sin(ωt)` A and `i_C = −i_L` |

#### Comparison with the analytical solution

For RC charge, RC discharge, RL rise, and RL decay, compare the state quantity
of the table above at every public step `t = k × 1 ms` for `k = 0 … 50`, and
at the 10, 20, and 30 ms values of the table. The accepted error is **at most
0.1% of full scale**: 10 mV for the RC fixtures (full scale 10 V) and 1 mA for
the RL fixtures (full scale 1 A).

This is met with margin by the trapezoidal rule at the 1 ms step, the least
accurate case the sub-step rule can choose. With `r = (1 − h/2τ)/(1 + h/2τ)`
and `τ = 10 ms`, the charge follows `10 (1 − r^k)`:

| Internal step | Worst error against `10 (1 − e^(−t/τ))` | Where |
| ------------: | --------------------------------------: | ----- |
|          1 ms |          3.07 mV (0.031% of full scale) | 10 ms |
|        0.5 ms |                       0.77 mV (0.0077%) | 10 ms |
|       0.25 ms |                       0.19 mV (0.0019%) | 10 ms |

At 10, 20, and 30 ms the 1 ms step gives 6.324275 V, 8.648904 V, and 9.503374 V
for the charge, against the analytical 6.321206 V, 8.646647 V, and 9.502129 V.
The RL fixtures have the same time constant and the same error.

The Rust Engine verifies both RC fixtures against the analytical solution at
every public step through 50 ms. It also checks the stored ngspice reference
samples at 10, 20, and 30 ms. The checked-in netlists and CSVs are
`stage-3-ngspice-rc-charge.cir` / `.csv` and
`stage-3-ngspice-rc-discharge.cir` / `.csv` in `tests/golden/`; they were
generated with ngspice 36 (Ubuntu package `36+ds-1ubuntu0.1`). The CSV values
preserve ngspice's printed `meas` values.

The DC fixtures must give the operating-point values with at most 1 µV in
voltage and 1 nA in current (the Stage 1 tolerances), and must hold them, with
the same tolerances, for 1000 public steps: a constant solution is a fixed
point of the trapezoidal rule.

#### Lossless LC: period and energy

Use the capacitor-voltage samples at the public step. For a source-free
lossless linear circuit the trapezoidal step is an exact rotation, so the
sampled voltage is a pure sinusoid `x_n = A cos(θ n + φ)`, with
`θ = ω_num × 1 ms`. Estimate it from the samples without interpolation:

```text
cos(θ) = Σ x_n (x_{n+1} + x_{n−1}) / (2 Σ x_n²)     over n = 1 … 198
T_est  = 2π × 1 ms / θ
```

using the first 200 samples (about ten periods). Accept the fixture when
`|T_est / 19.8691765 ms − 1| ≤ 0.25%`. For a 0.5 ms internal step the estimator
gives 19.9105019 ms, +0.208%, matching the table above. The check would reject
a 1 ms internal step (+0.83%), so it does detect an insufficient step.

Energy: `E(t) = ½ C v² + ½ L i²` computed from the C1 `voltage` and L1
`current` samples must stay within a relative `1e-6` of 5 mJ at every public
step for 10 s of model time (10 000 steps). The peak inductor current is
0.316228 A; it is checked through the energy, not by sampling, because a
sampled maximum can fall between two steps.

#### Comparison with ngspice

ngspice is the independent reference, used offline: the references are
generated once, stored under `tests/golden/stage-3-ngspice-*.csv` together with
the exact netlist and the ngspice version that produced them, and checked in.
CI does not run ngspice. A fixture's reference is regenerated only in a reviewed
change that records the new version.

Netlists (node `0` is ground; ngspice's `IC=` and `uic` apply the explicit
initial conditions; the default `trap` method is kept; `tmax` forces steps far
below the Engine's so the reference is not the limiting error):

```text
* RC charge            * RC discharge          * RL rise
V1 in 0 DC 10          R1 out 0 1k             V1 in 0 DC 10
R1 in out 1k           C1 out 0 10u IC=10      R1 in mid 10
C1 out 0 10u IC=0      .tran 10u 50m 0 10u uic L1 mid 0 100m IC=0
.tran 10u 50m 0 10u uic                         .tran 10u 50m 0 10u uic

* RL decay             * Lossless LC
R1 a 0 10              C1 n 0 100u IC=10
L1 a 0 100m IC=1       L1 n 0 100m IC=0
.tran 10u 50m 0 10u uic .tran 10u 200m 0 10u uic
```

The default-initialization fixtures are compared the same way with `.op`
(ngspice treats a capacitor as open and an inductor as a short there):

```text
* DC initialization, capacitor     * DC initialization, inductor
V1 in 0 DC 5                       V1 in 0 DC 5
R1 in out 1k                       R1 in mid 1k
C1 out 0 10u                       L1 mid 0 1
.op                                .op
```

`v(out)` must equal the Engine's C1 `voltage` and `i(L1)` its L1 `current`,
each within 1 µV and 1 nA. The capacitor current is zero by construction in an
open circuit (ngspice reports none in `.op`), so the Engine's own test checks
that C1 `current` is within 1 nA of zero.

The transient fixtures use this procedure: take the ngspice column for the
compared quantity (`v(out)` for the RC fixtures, `v(n)` and `i(L1)` for the LC
fixture, and `i(L1)` for the RL fixtures; ngspice reports `i(L1)` positive from
`pin1` to `pin2`, the same sign as the Engine) and interpolate it linearly onto
the Engine's public step times. For RC and RL the accepted
difference is **at most 0.2% of full scale** (the two methods' errors add). For
the LC fixture a pointwise comparison would only measure the 0.2% period error
accumulating over time, so compare `T_est` computed with the formula above on
both series: the two periods must agree to within 0.25%, and each must be
within 0.25% of the analytical period.

#### Initialization diagnostics

These circuits check the initialization rules of the contract (sections 3.2
and 3.3). Components are as in the table above unless stated. "References" are
the components the diagnostic names; `FloatingNode` references the wires of the
undetermined node, as in Stage 1.

| Circuit                                                                                                                                                                         | Initial conditions                  | Expected result                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| V1 ideal 5 V with C1 directly across it                                                                                                                                         | none                                | Valid: `v_C = 5 V` and `i_C = 0` from the DC solve.                                                                                                                                                                                                                                                                                |
| The same circuit                                                                                                                                                                | C1 explicit 3 V                     | `InconsistentState`, referencing C1: 3 V cannot hold across a 5 V ideal source.                                                                                                                                                                                                                                                    |
| The same circuit                                                                                                                                                                | C1 explicit 5 V                     | `SingularSystem`: the two ideal constraints are consistent but redundant (section 3.3). Leaving C1 unset starts it at its DC state.                                                                                                                                                                                                |
| A loop of V1 ideal 5 V, R1 1 kΩ, L1, and L2 in series (L1 `pin2` joined to L2 `pin1`, nothing else on that node)                                                                | none                                | Valid: 5 mA in both, from the DC solve.                                                                                                                                                                                                                                                                                            |
| The same circuit                                                                                                                                                                | L1 explicit 1 mA, L2 explicit 2 mA  | `InconsistentState`, referencing L1 and L2: the series currents must be equal.                                                                                                                                                                                                                                                     |
| The same circuit                                                                                                                                                                | L1 explicit 1 mA, L2 not set        | `InconsistentState`, referencing L1: L2 starts from its DC value, 5 mA, which cannot equal the 1 mA of L1 in series.                                                                                                                                                                                                               |
| The same circuit                                                                                                                                                                | L1 explicit 1 mA, L2 explicit 1 mA  | `FloatingNode`: the currents agree, but the node between the inductors then has only current sources on it (section 3.3, a known limitation). The conflict check comes first, which is why 1 mA and 2 mA above give `InconsistentState`.                                                                                           |
| Series RLC: V1 ideal 5 V (`pin1` to R1), R1 100 Ω, L1 10 mH (`pin1` joined to R1), C1 10 µF (`pin1` joined to L1 `pin2`, `pin2` to ground)                                      | L1 explicit 20 mA, C1 not set       | Valid: C1 starts at its DC value, `v_C = 5 V`, and carries `i_C(0) = +20 mA`; `v_L(0) = −2 V` (5 V − 2 V across R1 − 5 V across C1).                                                                                                                                                                                               |
| V1 10 V with its default internal resistance 0.1 Ω (`pin1` to C1), C1 and C2 in series (`pin2` of C1 joined to `pin1` of C2, which has nothing else on it), C2 `pin2` to ground | none                                | `FloatingNode`: the middle node is undetermined at DC.                                                                                                                                                                                                                                                                             |
| The same circuit                                                                                                                                                                | C1 explicit 2 V, C2 not set         | `FloatingNode`: the DC solve ignores explicit values, so C2's DC state is still undetermined. Set C2 as well.                                                                                                                                                                                                                      |
| The same circuit                                                                                                                                                                | C1 explicit 2 V, C2 explicit 8 V    | Valid: the middle node is 8 V, the two voltages add up to the source's 10 V, and the source current is 0 A. The source is not an ideal constraint, so the loop is not redundant.                                                                                                                                                   |
| The same circuit with V1's internal resistance 0 Ω                                                                                                                              | C1 explicit 2 V, C2 explicit 8 V    | `SingularSystem`: the loop of ideal constraints is consistent but redundant (section 3.3). With C2 explicit 6 V instead it is `InconsistentState`, referencing C1 and C2.                                                                                                                                                          |
| V1 ideal 5 V directly across L1 (1 H)                                                                                                                                           | none                                | `InconsistentIdealConstraint`: no DC operating point exists.                                                                                                                                                                                                                                                                       |
| The same circuit                                                                                                                                                                | L1 explicit 0 A                     | Valid, since every state is explicit and no DC solve is done. `i_L(t) = 5 t` A, so 0.05 A at step 10, exactly (the trapezoidal rule is exact for a linear ramp).                                                                                                                                                                   |
| R1 1 kΩ and C1 1 nF (time constant 1 µs) driven by V1                                                                                                                           | any valid set                       | Valid, with the warning `StepAccuracyLimited` (the rate exceeds the 64-substep limit).                                                                                                                                                                                                                                             |
| V1 ideal 5 V with C1 directly across it, started with no explicit conditions                                                                                                    | edit V1's voltage to 8 V at step 10 | Accepted: step 10 is computed again from the state at the end of step 9 with the new value, and the run goes on from there. The capacitor takes a large current in that step (an ideal source charging a capacitor), and no diagnostic is reported, since after step 0 the capacitor is a conductance and not an ideal constraint. |

#### What the acceptance does not claim

It does not claim accuracy for arbitrary circuits, stiff circuits beyond the
substep limit, or non-ideal parts. The 0.25% LC and 0.1% RC/RL figures are
fixture targets.

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
