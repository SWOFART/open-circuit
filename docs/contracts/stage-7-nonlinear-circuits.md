# Stage 7 Contract: Diodes, Transistors, and Nonlinear Circuits

**Status:** Draft for review by the Engine and Application workstreams (issue #108).
**Scope:** A generic junction diode and generic NPN and PNP bipolar transistors; the bounded Newton–Raphson solver that nonlinear devices need; how that solver joins the operating point, the transient steps, the events, and the live updates that already exist; the diagnostics, results, catalog entries, and symbols; and the canonical circuits with independently computed answers. This document extends, but does not replace, [`ARCHITECTURE.md`](../../ARCHITECTURE.md), [stage-1.md](stage-1.md), [stage-2-editor.md](stage-2-editor.md), [stage-3-dynamics.md](stage-3-dynamics.md), [stage-4-components.md](stage-4-components.md), [stage-5-assignments.md](stage-5-assignments.md), and [stage-6-control-circuits.md](stage-6-control-circuits.md). Where it states a rule for an earlier topic, section 1.2 lists the earlier text it amends. The models are recorded in [../physics/models.md](../physics/models.md).

All quantities are finite SI values. The models are small and generic: they are not manufacturer parts and say nothing about ratings, endurance, or safety.

## 1. Boundaries and compatibility

### 1.1 What Stage 7 adds

- A **diode** (`diode`): an ideal-exponential junction with a fixed small parallel conductance, two terminals, `anode` and `cathode`.
- An **NPN** (`bjt_npn`) and a **PNP** (`bjt_pnp`) bipolar transistor: the transport form of the Ebers–Moll model with a fixed small conductance across each junction, three terminals, `collector`, `base`, and `emitter`.
- A **nonlinear solve** (section 4): Newton–Raphson on the modified nodal equations of one electrical island, with junction-voltage limiting, a bounded deterministic continuation, and a final check of the original equations. A circuit without a nonlinear device is solved exactly as before.

The layers keep their Stage 1 roles:

| Layer           | Owns                                                                                                                                                                                                                                                                                                       |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/engine` | The device equations and their derivatives, the Newton iteration and its limits, the continuation, the integration into operating point, transient, events, and live updates, the diagnostics, the result keys, and the commit and rollback of a solve. Transport-independent: no Tauri, file, or UI code. |
| `crates/app`    | The threads and the delivery of results, including telling the application that a run has stopped and why (section 6.3). It computes nothing about a device.                                                                                                                                               |
| `src/`          | Symbols and pin geometry, parameter entry in SI units, and the presentation of what the Engine reports. It has no semiconductor formula and derives no terminal value.                                                                                                                                     |

### 1.2 Earlier text this contract amends

| Earlier text                                                                                                                                   | Amendment                                                                                                                                                                                                                                                                      |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ARCHITECTURE.md section 3.5: "Newton–Raphson inside each step … with a fallback (gradually added conductance to ground) if convergence fails." | Section 4 settles what the sentence leaves open: the tests, the limits, the limiting function, and the continuation. The architecture's decision is unchanged.                                                                                                                 |
| stage-1.md section 4 catalog: the parameter fields                                                                                             | `ParameterDefinition` gains two optional fields, `minimumValue` and `maximumValue` (section 2.3). They are absent for every earlier parameter.                                                                                                                                 |
| stage-1.md, stage-3-dynamics.md section 5, stage-4-components.md section 4: the diagnostic codes                                               | Two codes are added, `NonConvergence` and `ModelRangeExceeded` (section 6). Nothing else about the earlier codes changes.                                                                                                                                                      |
| stage-3-dynamics.md section 4.2, internal substeps                                                                                             | Nonlinear devices do not enter the rate bound (section 5.2). The rule for linear and relay circuits is unchanged.                                                                                                                                                              |
| stage-3-dynamics.md section 3.4 and stage-6-control-circuits.md section 4.2: a refused update                                                  | An update that makes the nonlinear solve fail is refused with the diagnostic of section 6 and leaves the run, its positions, its revision, and its trace history as they were.                                                                                                 |
| stage-4-components.md section 4: a run that stops on `EventChatter`                                                                            | The same rule covers `NonConvergence`, `ModelRangeExceeded`, and `NonFiniteState`. The application must tell the user that the run has stopped and why (section 6.3); today the stream ends without a message, and that gap is closed for every code, `EventChatter` included. |
| stage-2-editor.md and stage-5-assignments.md: the saved project                                                                                | Unchanged in kind. A project is still a version 1 schematic with three more component types (section 7.4).                                                                                                                                                                     |

### 1.3 Out of scope

AC and phasor analysis; MOSFETs, JFETs, thyristors, and imported SPICE model cards; temperature as a parameter or a sweep; every junction effect in the deferred column of section 3.5; adaptive stepping; sparse, GPU, or WASM solvers; supply rails (Stage 8); editable part names; and checks, faults, and diagnoses for the new devices in assignments. A separately approved scope change has to enter this contract before anything is built on it.

## 2. Catalog entries

The Engine catalog is authoritative. Type IDs, pin IDs, and parameter IDs are stable serialized strings; parameter IDs use the existing camelCase format.

### 2.1 Types, pins, and parameters

| Type ID   | Ordered pin IDs and roles                                                | Parameters (SI; domain and allowed range; default)                                                                                                                                             |
| --------- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `diode`   | `anode` (positive), `cathode` (negative)                                 | `saturationCurrentAmps` (`strictly_positive`; 1e-18 to 1e-9 A; 1e-14 A), `emissionCoefficient` (`strictly_positive`, `dimensionless`; 1 to 2; 1)                                               |
| `bjt_npn` | `collector` (unpolarized), `base` (unpolarized), `emitter` (unpolarized) | `saturationCurrentAmps` (1e-18 to 1e-9 A; 1e-14 A), `forwardBeta` (`strictly_positive`, `dimensionless`; 1 to 1000; 100), `reverseBeta` (`strictly_positive`, `dimensionless`; 0.01 to 100; 1) |
| `bjt_pnp` | `collector`, `base`, `emitter`                                           | the same three parameters with the same defaults                                                                                                                                               |

The transistor pins are listed in the order collector, base, emitter everywhere: in the catalog, in the symbol, and in the result layout. Pin IDs `anode`, `cathode`, `collector`, `base`, and `emitter` are new. The new parameter IDs are `saturationCurrentAmps`, `emissionCoefficient`, `forwardBeta`, and `reverseBeta`.

Defaults are reproducible educational values (a small-signal silicon junction at 27 °C), not ratings or manufacturer specifications. A diode with the defaults drops about 0.65 V at 1 mA. All parameters are live parameters (stage-3-dynamics.md section 3.4): none is an initial condition.

### 2.2 Cross-parameter constraints

There are none. In particular `forwardBeta` need not exceed `reverseBeta`. A value outside its range, or a non-finite value, is `InvalidParameter`, checked at compile and at every update. The ranges keep the junction equations in a region where a double-precision solve is meaningful: below 1e-18 A a junction needs about 0.9 V to carry a milliampere, and its leakage is lost under the `g_min` term; above 1e-9 A it conducts at a few tenths of a volt, where the supported range of section 3.1 is no longer the limit that matters.

### 2.3 Parameter bounds in the catalog

`ParameterDefinition` gains the optional fields `minimumValue` and `maximumValue` (finite numbers, inclusive, in the parameter's unit). When present, the interface shows them as the allowed range and the Engine enforces them with `InvalidParameter`; the domain still applies. They are omitted for every parameter of the earlier types, which keep their domains alone. This is an additive change to the catalog DTO and the golden catalog file; an older reader never meets it, because the application and the interface ship together.

### 2.4 Message keys

The new catalog entries carry these keys, in the existing `component.<type>.<name>` form:

| Type      | `displayNameKey`        | `descriptionKey` of its parameters                                                                   |
| --------- | ----------------------- | ---------------------------------------------------------------------------------------------------- |
| `diode`   | `component.diode.name`  | `component.diode.saturationCurrent`, `component.diode.emissionCoefficient`                           |
| `bjt_npn` | `component.bjtNpn.name` | `component.bjtNpn.saturationCurrent`, `component.bjtNpn.forwardBeta`, `component.bjtNpn.reverseBeta` |
| `bjt_pnp` | `component.bjtPnp.name` | `component.bjtPnp.saturationCurrent`, `component.bjtPnp.forwardBeta`, `component.bjtPnp.reverseBeta` |

The new diagnostics keep the form `diagnostic.<code>` (`diagnostic.nonConvergence`, `diagnostic.modelRangeExceeded`).

## 3. Component models

### 3.1 Constants and the exponential

The temperature is fixed at `T = 300.15 K` (27 °C, the usual circuit-simulator default) for every device and every run; it is not a parameter. With the exact SI constants `k = 1.380649e-23 J/K` and `q = 1.602176634e-19 C`, the thermal voltage is

```text
V_T = k T / q = 0.02586492578632875 V
```

Every junction also has a fixed parallel conductance `g_min = 1e-12 S`. It is part of the model, not a numerical aid: it is always present, it appears in the equations below, in the results, and in the final check, and it is what makes a reverse-biased junction carry a small, defined current instead of `-I_S` alone.

The exponential's argument is `x = v / (n V_T)` for the diode and `x = v / V_T` for a transistor junction. The model is **supported** for `x <= 80`, where `exp(x)` is about 5.5e34. Where the iteration needs to evaluate beyond 80, the exponential is continued by its tangent at 80, `exp(80) (1 + x - 80)`, which keeps the iteration finite. This **extended** function equals the model equation wherever `x <= 80` and is what the iteration, the update test, and the residual test of section 4.4 evaluate; so a circuit whose only solutions lie beyond 80 does converge, in the extended function, and is then refused by the range check of section 4.6 with `ModelRangeExceeded`. The reported device results are only ever those of the model equation, which the range check guarantees coincides with the extended function. The bound is not a current limit: with the default saturation current, 80 thermal voltages is about 2.07 V and about 5.5e20 A, far beyond anything a circuit of this program carries.

### 3.2 Diode

Terminals `anode` and `cathode`. The voltage is `v = V(anode) - V(cathode)`, and the current `i` is positive entering `anode`:

```text
i(v)  = I_S (exp(v / (n V_T)) - 1) + g_min v
di/dv = (I_S / (n V_T)) exp(v / (n V_T)) + g_min
```

`I_S` is `saturationCurrentAmps` and `n` is `emissionCoefficient`. The relation is the ideal diode equation of Shockley; there is no breakdown, so a reverse voltage of any size carries `-I_S - g_min |v|` at most. The result keys are `<id>:voltage` and `<id>:current` with the pin-1/pin-2 convention, pin 1 being `anode`.

### 3.3 NPN transistor

Terminals `collector`, `base`, `emitter`. The junction voltages are `v_BE = V(base) - V(emitter)` and `v_BC = V(base) - V(collector)`. With `I_S`, `beta_F = forwardBeta`, and `beta_R = reverseBeta`:

```text
I_F = I_S (exp(v_BE / V_T) - 1)          I_R = I_S (exp(v_BC / V_T) - 1)

i_C = I_F - I_R (1 + 1/beta_R)           - g_min v_BC
i_B = I_F / beta_F + I_R / beta_R        + g_min (v_BE + v_BC)
i_E = -(i_C + i_B) = -I_F (1 + 1/beta_F) + I_R - g_min v_BE
```

`i_C`, `i_B`, and `i_E` are the currents **entering** the collector, base, and emitter terminals. They sum to zero by construction. This is the transport form of the Ebers–Moll model, with the two junctions' emission coefficients fixed at 1. It is the same set of equations as the Gummel–Poon model with infinite Early voltage, no high-injection knee, no recombination currents, and no parasitic resistances.

The derivatives that the Newton iteration uses, with `g_F = (I_S / V_T) exp(v_BE / V_T)` and `g_R = (I_S / V_T) exp(v_BC / V_T)`:

```text
d i_C / d v_BE =  g_F                     d i_C / d v_BC = -(1 + 1/beta_R) g_R - g_min
d i_B / d v_BE =  g_F / beta_F + g_min    d i_B / d v_BC =  g_R / beta_R + g_min
```

`i_E` follows from the other two. One set of equations covers cutoff, forward active, saturation, and reverse active; the Engine never switches between region formulas, and it does not store or report a region.

### 3.4 PNP transistor

The PNP is the NPN with every voltage and current negated. With `v_EB = V(emitter) - V(base)` and `v_CB = V(collector) - V(base)`, the PNP uses the NPN formulas of section 3.3 with `v_EB` in place of `v_BE` and `v_CB` in place of `v_BC`, and the terminal currents entering the collector, base, and emitter are the **negatives** of the values those formulas give. A PNP circuit in which every source voltage is negated therefore has every node voltage and terminal current negated, exactly, in the equations (and to the solve's tolerance in a result). The fixtures use that symmetry (section 9).

### 3.5 Included and deferred effects

| Effect                                                                                                          | Status       | Treatment                                                                                                              |
| --------------------------------------------------------------------------------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------------- |
| Exponential junction law (diode: with emission coefficient; transistor: transport form with `beta_F`, `beta_R`) | included     | sections 3.2 to 3.4                                                                                                    |
| Reverse leakage                                                                                                 | included     | `-I_S` plus `g_min v`; no other leakage current                                                                        |
| Parallel junction conductance                                                                                   | included     | fixed `g_min = 1e-12 S` across each junction, part of the model                                                        |
| Fixed temperature                                                                                               | included     | `T = 300.15 K`; not a parameter                                                                                        |
| Ohmic series resistance (diode `R_S`; transistor `R_B`, `R_C`, `R_E`)                                           | deferred     | the user places a resistor; without one an ideal source that needs more than 80 thermal voltages across a forward junction (above about 2.07 V for the default diode) is refused with `ModelRangeExceeded`, and a lower voltage gives a large but finite current |
| Breakdown (Zener, avalanche, transistor)                                                                        | deferred     | no reverse current beyond leakage                                                                                      |
| Junction and diffusion capacitance, transit time, reverse-recovery                                              | deferred     | a junction responds instantly; storage is only what the circuit's own capacitors hold                                  |
| Early effect, high-level injection, recombination currents, transistor emission coefficients other than 1       | deferred     | the transistor's output conductance in forward active comes from `g_min` alone                                         |
| Temperature dependence, self-heating, noise                                                                     | deferred     |                                                                                                                        |
| Operation-region display and region-based checks                                                                | deferred     | no state kind, no frame field (section 7)                                                                              |
| MOSFETs, imported model cards                                                                                   | out of scope | section 1.3                                                                                                            |

Nothing deferred is implied by a result. The documentation and the interface say "generic", never "1N4148" or "2N3904".

## 4. The nonlinear solve

### 4.1 Equations and linearization

Each electrical island is solved on its own, as today. Its unknowns are the node voltages relative to the island's reference node and the branch currents of ideal voltage sources and of the companion rows that already exist. A nonlinear device contributes terminal currents `I_k(V)` entering its terminals, with `V` the terminal voltages. At an iterate `V*` the device is replaced by its linearization, and stamped like any linear device:

```text
I_k(V) ≈ I_k(V*) + sum_j G_kj (V_j - V*_j),     G_kj = d I_k / d V_j at V*
```

`G` enters the conductance matrix and `sum_j G_kj V*_j - I_k(V*)` the right-hand side, at the rows of the terminals' nodes. The diode is the one-junction case (section 3.2); the transistor is the two-junction case, whose 3×3 conductance block follows from section 3.3 by the chain rule on `v_BE` and `v_BC`. Each iteration is one dense LU solve with the existing solver and its existing relative residual check. The assembly order, the pivoting, and the iteration order are fixed, so the same inputs give identical results on a given build and platform (the exponential and logarithm come from the platform's math library, so bit-identity across platforms is not claimed). If the LU solve of any iteration fails (a singular or non-finite matrix, or a residual too large), the whole solve fails at once with `SingularSystem` (or `NonFiniteState` for a non-finite number), referencing the island's components as a linear singular system does; the continuation is not tried.

The RHS contribution follows owner approval in #117:
`sum_j G_kj V*_j - I_k(V*)`: positive terminal current enters the device,
so KCL is `G_linear V + I(V) = J`. Existing motor armature-current and
angular-velocity unknowns remain in the same coupled system.

### 4.2 Initial iterate

For an operating point, an initial-state solve, and the first solve of a run, every node voltage starts at 0 V, which makes every junction voltage 0. For every later solve (the internal steps of a transient, a same-time event re-solve, and a live update) the iteration starts from the junction voltages and node voltages of the last committed solution of the same island (for an update, of the step start). A start is a pure function of committed state, never of an earlier failed attempt. The committed junction and node voltages used as a start are solver state that is not in the snapshot; they are kept with the step-start state (`step_start_states`), so that a live update recomputing the newest step starts from the same iterate the original step did.

### 4.3 Junction-voltage limiting

After each linear solve the new junction voltage `v_new` is limited against the voltage `v_old` the stamp was built at, by this function (the `pnjlim` rule as SPICE3 implements it; Nagel 1975 describes the iteration it serves):

```text
v_limited(v_new, v_old, V_t, v_crit):
    if v_new > v_crit and |v_new - v_old| > 2 V_t:
        if v_old > 0:
            arg = 1 + (v_new - v_old) / V_t
            return v_old + V_t ln(arg)  if arg > 0  else  v_crit
        else:
            return V_t ln(v_new / V_t)
    else:
        return v_new
```

with `V_t = n V_T` for the diode and `V_T` for a transistor junction, and `v_crit = V_t ln(V_t / (sqrt(2) I_S))` (for the default diode, 0.7302 V). The next iteration stamps the device at the **limited** voltages. An iteration in which any junction was changed by the limiting is **limited**, and a limited iteration never counts as converged. Limiting is an aid of the iteration only; it never changes a device equation, and the final solution is judged on the unlimited values (section 4.6).

### 4.4 Convergence tests

An iteration is accepted as the solution when it is not limited and both tests pass, evaluated on the new iterate `x_new` against the previous iterate `x_old`:

1. **Update test.** For every unknown `i`: `|x_new,i - x_old,i| <= 1e-9 * max(|x_new,i|, |x_old,i|) + abs_i`, with `abs_i = 1e-9 V` for a node voltage and `1e-12 A` for a branch current. `x_old` is the solution vector of the previous iteration's linear solve (the unknowns themselves, not the limited junction voltages derived from it).
2. **Residual test, on the original equations.** The device terminal currents are computed from the device equations (in the extended function of section 3.1, which is the equation wherever `x <= 80`) at the junction voltages of `x_new` (not from the linearization, not from the limited values). For every node row, the sum `r` of all currents leaving the node must satisfy `|r| <= 1e-9 * S + 1e-15 A`, with `S` the sum of the magnitudes of the **terms** that make up those currents: a resistor's or source's current, a companion element's current, and for a nonlinear device the magnitude of each term of its terminal current as section 3 writes it (the diode's `I_S (exp(x) - 1)` and `g_min v`; the transistor's `I_F`, `I_R`, `I_F / beta_F`, `I_R / beta_R`, `g_min v_BE`, and `g_min v_BC`, in the terminals they enter). `S` is a scale for rounding, so it uses the terms, not their sum: a transistor deep in the extended range has terminal currents that are differences of terms of the same size, and a net current alone would be rounding noise. For every ideal-voltage-source row, the voltage constraint must hold to `1e-9 * max(|E|, |V_a|, |V_b|) + 1e-9 V`.

For a motor angular-velocity unknown, the update absolute tolerance is
`NEWTON_SPEED_ABSTOL = 1e-9 rad/s`, with the same relative tolerance. Check
the original armature equation with an absolute residual floor of `1e-9 V`
and the original mechanical equation with `NEWTON_TORQUE_ABSTOL = 1e-15 N m`;
add `1e-9` times the sum of absolute equation terms (including the RHS).
These additions were approved in #117; the coupled motor equations do not change.

The tolerances are fixed constants, named `NEWTON_RELTOL`, `NEWTON_VNTOL`, `NEWTON_BRANCH_ABSTOL`, `NEWTON_ITOL`, `NEWTON_SPEED_ABSTOL`, and `NEWTON_TORQUE_ABSTOL`, and are not parameters of a circuit.

#### Floating-point evaluation allowance (owner-approved #118)

The residual threshold above additionally includes an explicit arithmetic
`roundoff_bound`; physical-current scale `S` and every existing tolerance remain
unchanged. Define `gamma(m) = m * f64::EPSILON / (1 - m * f64::EPSILON)`.
EPSILON, rather than half EPSILON, conservatively includes rounded stored operands.
For a linear stamp with `n` coefficient products and RHS `b`, add
`gamma(2*n + 2) * (sum_j abs(a_j*x_j) + abs(b))` to that row's bound.
For `G*(V_a-V_b)`, include both `abs(G*V_a)` and `abs(G*V_b)` even though
the implementation subtracts potentials before multiplying. This counts the
products, additions, RHS subtraction and operand representation allowance.
For an ideal voltage constraint, use `gamma(4)*(abs(V_a)+abs(V_b)+abs(E))`.
For a continuation current, use `gamma(2)*abs(g_c*V)`.
Finally add `gamma(k)*S`, where `k` is the count of accumulated linear stamps,
individual nonlinear current terms and continuation currents in the row.
The bound has the row's physical units (A, V or N m). Non-finite bounds or
`m*EPSILON >= 1` fail with `NonFiniteState`; an infinite tolerance is never accepted.

The final criterion is `abs(residual) <= 1e-9*S + absolute_floor + roundoff_bound`.
It still evaluates unlimited voltages on original equations, removes continuation
before acceptance, and does not alter the canonical fixture's accuracy tolerances.

### 4.5 Iteration limit and continuation

A Newton solve takes at most `NEWTON_MAX_ITERATIONS = 100` iterations. If the plain solve (no added conductance) does not meet section 4.4 within the limit, the Engine runs a **conductance continuation**: a fixed ladder of stages `g_c = 1e-2, 1e-3, …, 1e-12 S` (eleven stages), then a last stage with `g_c = 0`. At each stage a conductance `g_c` is added from every non-reference node that has a nonlinear-device terminal to the island's reference node; the stage starts from the previous stage's solution (the first from the initial iterate of section 4.2) and takes at most 100 iterations. A stage must meet section 4.4 against its own equations. If any stage fails, the whole solve fails with `NonConvergence`; stages are never skipped. The total is therefore at most `100 + 12 × 100` iterations, and the order is fixed.

The added conductance is an aid. The last stage has none, and the solution is accepted only from that stage; a result of an earlier stage is never reported, committed, or used as a start for anything but the next stage. No source stepping, no pseudo-transient, and no randomization exist in this release.

### 4.6 The final solution

The accepted solution is the iterate of the plain solve or of the last continuation stage that passed section 4.4 with `g_c = 0`. The range check comes after those tests and never replaces them: a circuit whose settled iterate lies beyond the supported range passes section 4.4 in the extended function (the scale `S` of the residual test uses the terms, which is what keeps that true), and is then refused here. In addition, every junction argument at the accepted solution must satisfy `x <= 80` (section 3.1), or the solve fails with `ModelRangeExceeded`; and every number the solve produces must be finite, or it fails with `NonFiniteState`. Device results are computed from the device equations at this solution, never from a linearization.

## 5. Where the solve is used

### 5.1 Operating point and initialization

At compile, the circuit is initialized as stage-3-dynamics.md section 3.2 says, with each solve of a circuit that has nonlinear devices done by section 4: capacitors open and inductors shorted for the operating point; with explicit initial conditions, capacitors as voltage sources and inductors as current sources at their initial values, and the nonlinear devices still solved. An initial condition that conflicts with an ideal constraint is `InconsistentState`, as before. A nonlinear solve that fails during initialization fails the compile with the diagnostic of section 6; the previous run, if any, is untouched.

### 5.2 Transient steps

Each internal step of a public step solves the companion system of the capacitors and inductors (trapezoidal, unchanged) with the nonlinear devices linearized at each iteration, by section 4, starting from the committed state of the previous internal step. The accepted solution updates the capacitor, inductor, and motor state exactly as for a linear circuit. The public step stays 1 ms, and the internal substep count is chosen by the existing rule; **nonlinear devices do not contribute to its rate bound** (it treats them as open circuits), so a circuit's substep count is the same as for the same circuit with the devices removed. The consequences are stated rather than hidden: the accuracy of a fast conduction change is what a 1 ms public step with the trapezoidal rule gives, and the canonical transient (section 9.4) is chosen with a time constant of several steps. Nonlinear devices are smooth, so a junction turning on or off is not an event: it does not trigger backward Euler. A trapezoidal ringing excited by a stiff turn-off (a rectifier into a capacitor, for example) is a limitation, not a defect to hide.

### 5.3 Events and re-solves

The order of stage-4-components.md section 4 is unchanged: solve the internal step, then evaluate the relay thresholds and the fuse integral on that accepted solution, apply simultaneous transitions together in canonical component-ID order, and re-solve at the same time until the discrete state is stable. Every same-time re-solve is a nonlinear solve by section 4, started from the converged solution of the solve before it. The limit of 16 state-changing re-solves, the repeated-state-vector check, `EventChatter`, and the backward-Euler step after an event are unchanged, and a re-solve that does not converge is `NonConvergence`, never `EventChatter`. The operator changes of stage-6-control-circuits.md section 4.3 recompute the newest step through the same path.

### 5.4 Live updates, operator changes, and atomicity

A `set_parameter` of a nonlinear device's parameter is validated against section 2 and then recomputes the newest public step from the step-start state, by section 5.2. A `set_operator_state` does the same. Both are **all or nothing**:

- The recomputation works on copies of the step-start state. Nothing of the run (the continuous state, the discrete state, the snapshot, the frame sequence, the revision, the positions, the trace history) is replaced until the whole recomputation has succeeded.
- A recomputation that fails, for any reason, leaves the run as it was and returns the diagnostic of section 6 (or `InvalidParameter` for a value out of range). The interface restores what it showed.
- The same holds for `advance`: a public step that fails to solve commits no state, no snapshot, and no trace sample, including the samples of earlier steps of the same request.

### 5.5 Traces

Trace history stays limited to numeric result-layout slots. A recomputed step replaces the newest sample exactly as in stage-3-dynamics.md section 7.5; a refused update changes no sample. The new result slots (section 7.1) are traceable like any other.

## 6. Diagnostics

### 6.1 Codes

Two codes are added. Both are errors.

| Code                 | Meaning                                                                                                                                          | References                                                                      |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| `NonConvergence`     | The Newton iteration, with the continuation of section 4.5, did not meet section 4.4                                                             | The nonlinear components of the failing island, in canonical component-ID order |
| `ModelRangeExceeded` | A solution that satisfies the tests needs a junction beyond the supported range (section 3.1), as an ideal source of more than about 2.07 V across a forward junction does | The components whose junction is beyond the range, in canonical order           |

`InvalidParameter` covers a parameter out of its range, `NonFiniteState` a non-finite number from an accepted input, `SingularSystem` a matrix the LU solve cannot factor, and `FloatingNode`, `NoGroundReference`, `ShortCircuit`, `InconsistentIdealConstraint`, and `InconsistentState` keep their meanings. A nonlinear device with an unconnected terminal is an ordinary floating node. The first error found in a fixed order (parameters, topology, then the solve, island by island in the order of their lowest component ID) is the one the compile reports along with the others of its kind, as for linear circuits; a solve that fails in one island fails the whole solve, and none of its results is used.

### 6.2 What stops, what stays, what is refused

| When it fails                         | Effect                                                                                                                                                                                                                                            |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Compile                               | `CompileResult::Failure` with the diagnostics; the previous run is untouched                                                                                                                                                                      |
| `advance` during a run                | The run stops at its last committed public step. The latest snapshot stays the last committed one and stays visible; no partial frame or sample is produced. A new compile, or a successful update followed by new steps, is how the user goes on |
| `set_parameter`, `set_operator_state` | Refused with the diagnostics; the run continues unchanged                                                                                                                                                                                         |

A parameter update that would make the run's next step unsolvable is therefore refused at the update where it can be, but a circuit that only becomes unsolvable as time passes (a capacitor charges until a junction is beyond its range) stops the run when it happens.

### 6.3 Telling the user that a run stopped

Today a stream that ends because the Engine returned an error ends silently. For Stage 7 the application delivers a **stop notice** to the interface whenever a run stops on an Engine error: the run ID, the document revision, the step at which it stopped, and the Engine's diagnostics. The interface shows them, keeps the last snapshot on screen, and says the run has stopped. The same notice serves `EventChatter` and `NonFiniteState`. The transport (a separate message, or a final frame) is decided in the bridge issue (#113); this contract fixes only that the diagnostics reach the user, are tied to the run and revision, and are never shown for another run.

## 7. Results, compatibility, and generated types

### 7.1 Result layout

A diode exposes the standard two slots, `<id>:voltage` then `<id>:current`, with the pin-1/pin-2 sign convention (pin 1 is `anode`).

A transistor exposes five slots, in this order, with these keys. For a voltage slot the first named terminal is the positive reference; a current slot is positive **entering** the named terminal:

| Key                              | Quantity | Unit | Definition                  |
| -------------------------------- | -------- | ---- | --------------------------- |
| `<id>/base_emitter/voltage`      | voltage  | V    | `V(base) - V(emitter)`      |
| `<id>/collector_emitter/voltage` | voltage  | V    | `V(collector) - V(emitter)` |
| `<id>/collector/current`         | current  | A    | `i_C`                       |
| `<id>/base/current`              | current  | A    | `i_B`                       |
| `<id>/emitter/current`           | current  | A    | `i_E = -(i_C + i_B)`        |

NPN and PNP use the same keys and the same definitions: the voltages are the actual terminal differences and the currents are the actual currents entering the terminals, so a PNP in its normal bias reads negative `base_emitter` voltage and negative `collector/current`. The three terminal currents are computed by the Engine from the device equations at the final solution and sum to zero; nothing in the interface derives a terminal value. The existing quantity and sign-convention values are reused (`voltage` with `voltagePin1MinusPin2`, `current` with `currentIntoPin1`), read as "first named terminal of the slot", as the relay's ports already do; the generated enums are unchanged, and this is intentional.

### 7.2 No new state kind and no format change

A semiconductor has no discrete state: no operation region, no conduction flag, and no stored value besides the circuit's own state. The snapshot's state table, the OCF2 frame (its magic, header, state kinds, and flags), and the OCT1 trace envelope are unchanged. An operation region may be shown later only if a contract change defines it as an Engine-owned state.

### 7.3 Generated types

The following are Engine-owned Rust types, generated for TypeScript with the existing generator (`src/generated/**` is never edited by hand): the additions to `ComponentTypeId` (`diode`, `bjt_npn`, `bjt_pnp`), `PinId` (`anode`, `cathode`, `collector`, `base`, `emitter`), `ParameterId` (the four parameters of section 2.1), and `DiagnosticCode` (`NonConvergence`, `ModelRangeExceeded`); the optional `minimumValue` and `maximumValue` of `ParameterDefinition`; and the `ComponentSpec` variants and parameter structs of the three types. The catalog entries list no fault kinds: `faultKinds` is empty, and a fault on one of them is an `InvalidFault` until a later contract adds some. The stop notice of section 6.3 is a new Engine-or-application DTO whose owner the bridge issue settles; it is generated from Rust like the rest.

### 7.4 Project format and older readers

The project stays a version 1 schematic. It may contain the three new types with their parameters, and nothing else changes: no field for a computed value, a state, or an operating region is ever saved. An older reader rejects a file that uses a new type with its existing "unknown component type" diagnostic and never reads it as something else. Reviewers should confirm this choice, as they did for Stage 6 (see "Points for review").

## 8. Symbols and labels

Symbols follow the electrical-schematic convention (graphical symbols as in IEC 60617): a diode is a triangle pointing from anode to cathode with a bar at the cathode; a transistor is a base line with the collector and emitter leads at an angle to it, and an arrow on the emitter that points out of the base for an NPN and into it for a PNP. The pin positions in the symbol's local grid (x right, y down, rotated by the Stage 1 rules) are the only geometry the Engine needs:

| Type                 | Local pin positions                                                |
| -------------------- | ------------------------------------------------------------------ |
| `diode`              | `anode` (-1, 0), `cathode` (1, 0), as the other two-terminal parts |
| `bjt_npn`, `bjt_pnp` | `base` (-2, 0), `collector` (1, -2), `emitter` (1, 2)              |

The catalog order of the transistor pins is collector, base, emitter; the positions are keyed by pin ID. The editor's label prefixes (editor state, not saved) are `D` for diodes and `T` for transistors, so that `Q` stays the contactor's. The interface shows the Engine's numbers and nothing derived from them: no region, no gain, no power.

## 9. Canonical known-answer fixtures

Expected data is kept separately from the schematic inputs. The values are the solutions of the model equations of section 3, computed by [`tests/golden/stage-7-reference.py`](../../tests/golden/stage-7-reference.py), which uses only the Python standard library: Newton's method in 60-digit decimal arithmetic for the direct-current answers (each checked by its residual in the same arithmetic) and a fourth-order Runge–Kutta integration, checked against half its step, for the transient. They are not Engine output. Regeneration is deliberate: the script and this contract change together, and the golden files are rewritten from its output in the same change. The model parameters are the defaults of section 2.1 unless a fixture says otherwise. Where a fixture says "ideal source" the internal resistance is 0 Ω.

The same circuits are cross-checked against ngspice 47 (issue #109, [`docs/solver/stage-7-references.md`](../solver/stage-7-references.md)) with the equivalent model cards, a temperature of 27 °C, and the other junction effects at their disabling values. Two points of that alignment are part of this contract's meaning: the simulator's own `gmin` is set to 0 and an explicit 1e-12 S resistor is placed across each junction, because ngspice scales its `gmin` through the transistor betas, which is not the conductance across the junction that section 3.1 defines; and ngspice derives its thermal voltage from the CODATA 2014 values of the constants, which differs from section 3.1's exact SI 2019 values by 3.4e-7 relative, so a simulator result and the answers of this section agree to about 3.4e-7 V and not to rounding, unless the equations are evaluated with the simulator's constants. The Engine is held to the answers of this section, never to the simulator's numbers. This contract does not depend on the simulator.

### 9.1 Diode

| Fixture                                                                        | Circuit                                                | Expected check                                                                                                                                                                                     |
| ------------------------------------------------------------------------------ | ------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Diode forward and reverse](../../tests/golden/stage-7-diode-dc.expected.json) | An ideal source, 1 kΩ, and the default diode in series | Forward (5 V): `v_D = 0.692887832378056 V`, `i = 4.30711216762194 mA`. Reverse (-5 V): `v_D = -4.99999999499 V`, `i = -5.00999999499e-12 A` (`I_S` plus `g_min` times 5 V). Tolerances in the file |
| Ideal source across a forward diode                                            | An ideal 5 V source (more than 2.07 V), no resistor                       | The compile fails with `ModelRangeExceeded` referencing the diode; no other error is reported                                                                                                                                  |

### 9.2 Transistor

[Common-emitter stage](../../tests/golden/stage-7-bjt-dc.expected.json): the emitter at ground, 1 kΩ from a 10 V supply to the collector, and a base source through a base resistor. Cutoff (base source 0 V, 430 kΩ): `i_C = 1.0019997e-11 A`, `V_CE` within 20 nV of the supply. Forward active (5 V, 430 kΩ): `i_C = 1.01037633 mA`, `i_B = 10.1037555 µA`, `i_C / i_B = beta_F` to 1e-4 relative, `V_C = 8.98962367 V`. Saturation (5 V, 4.3 kΩ): `i_C = 9.93306 mA`, `i_B = 0.995648 mA`, `V_C = 66.94 mV`, with `i_C / i_B` below `beta_F`. The PNP cases are the same with every voltage negated, and their answers are the NPN answers negated. The wrong variant with the collector and emitter pins exchanged runs in reverse active and gives `i_C = -20.758 µA` into the (swapped) collector pin, not the forward-active answer. Every case's three terminal currents sum to zero within 1e-12 A.

### 9.3 Failure

Besides the diode of section 9.1, the transistor file records `ModelRangeExceeded` for the common-emitter stage of section 9.2 with the base resistor replaced by a wire, so that an ideal 5 V source is directly across the base-emitter junction of the NPN while the collector is wired through its 1 kΩ to the 10 V supply: the compile fails, the diagnostic references the transistor, and no `FloatingNode` or other error is reported.

### 9.4 Switched nonlinear RC

[Diode charging a capacitor](../../tests/golden/stage-7-diode-rc-transient.expected.json): an ideal 5 V source, a latching switch (0.01 Ω closed), 10 kΩ, the default diode, and a 1 µF capacitor with an explicit initial voltage of 0 V, in series to ground. The switch starts open and is closed when step 5 is the newest, so it conducts from the end of step 4. The capacitor voltage 50, 100, and 200 ms after that is 4.43286502 V, 4.52489018 V, and 4.56192038 V, to 5 mV. The diode-reversed variant reaches only 0.25, 0.50, and 1.00 µV. The file gives the tolerance basis: the trapezoidal error at a 1 ms step and a time constant of about 10 ms, and the decaying error of the one backward-Euler step after the switch event. The 5 mV tolerance is provisional; #109 and #111 may tighten it with evidence, and loosening it needs a contract change.

## 10. Layer boundaries and verification

- The Engine validates the parameters and their ranges, derives connectivity, owns the device equations, the Newton solve, the continuation, the final check, and the commit and rollback, creates the result layout, and emits structured diagnostics.
- The application owns threading and delivery, including the stop notice of section 6.3. It calculates nothing about a device.
- TypeScript owns the symbols, the pin geometry, SI parameter entry with the catalog ranges, and the presentation of what the Engine reports. It has no semiconductor formula and uses the generated types.
- Executable checks come with the implementation issues: the shared solve and atomic failure handling (#110), the diode (#111), the transistors (#112), the bridge and stop notice (#113), the interface (#114), and the end-to-end acceptance (#115). Reference data and the simulator cross-check are #109.
- A change to a model equation, a constant, a range, a tolerance, a limit, a diagnostic, a result key, a pin position, or a fixture value needs both workstreams' review and an update to this contract and the golden files. A change to an architectural decision also updates ARCHITECTURE.md or adds an ADR.

## 11. Primary references

- W. Shockley, "The theory of p-n junctions in semiconductors and p-n junction transistors", _Bell System Technical Journal_ 28 (1949), the diode law.
- J. J. Ebers and J. L. Moll, "Large-signal behavior of junction transistors", _Proceedings of the IRE_ 42 (1954), the transistor model of section 3.3.
- H. K. Gummel and H. C. Poon, "An integral charge control model of bipolar transistors", _Bell System Technical Journal_ 49 (1970), for the statement that section 3.3 is its special case without the effects listed as deferred.
- L. W. Nagel, _SPICE2: A Computer Program to Simulate Semiconductor Circuits_, Memorandum ERL-M520, University of California, Berkeley (1975), for the Newton iteration, the junction limiting of section 4.3, and the conductance to ground.
- The ngspice manual of the release that #109 pins, for the equivalent model cards, `gmin`, and the convergence options used in the cross-check.
- The 2019 SI definition of the kelvin and the elementary charge (BIPM, _The International System of Units_), for the exact constants of section 3.1.

These references support the equations and the vocabulary. They do not set the generic defaults, which are educational values.

## Points for review

These are the choices a reviewer is asked to confirm or change:

1. **Model families.** The Shockley diode and the transport-form Ebers–Moll transistor (Gummel–Poon without its extra effects), with the extensions of section 3.5 deferred. The alternative, a fuller Gummel–Poon with Early voltage and series resistances, would add parameters and internal nodes before any fixture can check them.
2. **A fixed temperature of 300.15 K.** A parameter would need a thermal model of the other devices to be honest, and a sweep is out of scope.
3. **`g_min` is part of the model.** It makes reverse leakage and the cutoff answers defined and reproducible (and matches a simulator's default). The alternative, a pure `-I_S` leakage with `g_min` only as an iteration aid, would make the cutoff answers 1e-14 A, below the floor of the convergence tests.
4. **No series resistance.** A circuit that puts an ideal source of more than about 2.07 V across a forward junction is refused with `ModelRangeExceeded` instead of reporting an astronomical current; a smaller voltage across a junction gives a large but finite current, which the model accepts. The user places a resistor, which is also what the circuit would need on a bench.
5. **Supported range of 80 thermal voltages.** Beyond it the model is refused, never extended in a result. The bound is not a current limit: below it a circuit can still report currents of 1e11 A or more that no real part carries. A larger bound only moves the refusal to currents no circuit of this program can carry.
6. **Fixed tolerances and limits** (section 4.4 and 4.5): relative 1e-9, 100 iterations, an eleven-stage conductance ladder from 1e-2 S to 1e-12 S. They are tighter than a simulator's defaults because the Engine is checked against 60-digit answers. If a canonical circuit shows they are unreachable in double precision, the contract changes; they are not loosened silently.
7. **Optional `minimumValue` and `maximumValue` in the catalog** (section 2.3), so that "safe numerical ranges" are shown by the interface from the catalog instead of repeated in TypeScript. The alternative, ranges that only the Engine knows, would give the user an error for a value the field accepted.
8. **Two new diagnostic codes** (section 6), instead of reusing `SingularSystem` or `NonFiniteState`: the user needs to be told "did not converge" and "needs a current this model cannot represent" in different words.
9. **A stop notice for every run that stops on an Engine error** (section 6.3). It is a gap that already exists for `EventChatter`; this contract fixes the requirement and leaves the transport to #113.
10. **Nonlinear devices do not tighten the internal step** (section 5.2). The rate of a conducting junction depends on the operating point and would make the substep count state-dependent, which the existing deterministic rule avoids.
11. **No new state kind and no region display** (section 7.2). Showing a region is a presentation of an Engine value; it needs its own contract if it is wanted.
12. **Provisional transient tolerance** (section 9.4), with its basis in the fixture file.
13. **Label prefixes `D` and `T`** and the pin positions of section 8, so that `Q` stays the contactor's.
14. **Older readers** reject the new types with the existing unknown-type diagnostic, as in Stage 6.
