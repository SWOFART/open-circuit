# Stage 10 contract — Three-phase equipment

Proposal for owner approval under [#172](https://github.com/SWOFART/open-circuit/issues/172).
Implementation follows acceptance of this contract. The devices model
educational lumped circuits. Their trip settings describe the simulation.

## 1. Boundary and compatibility

Use the Stage 9 time-domain engine and the Stage 3 fixed 1 ms public step,
bounded deterministic substeps, trapezoidal integration and backward Euler
after events. Rust owns connectivity, equations, states and diagnostics.
Tauri carries generated DTOs and the editor draws pins and displays results.
Schema 2 gains these component types and their drawn parameters. Runtime
state and derived results are excluded from save files. Older readers
refuse unknown types. Stage 10 devices follow Stage 9's assignment and
fault exclusion. Earlier device types and layouts are unchanged.

## 2. Pins, grouping and results

Catalog order is the table order after the Stage 9 additions. A group is
the named ordered pair of pins of a local branch, with current entering
its first pin and voltage measured first minus second.

| Type ID | Ordered groups or pins |
|---|---|
| `contactor_three_pole` | coil (a1, a2), pole1 (l1, t1), pole2 (l2, t2), pole3 (l3, t3), no (no_1, no_2), nc (nc_1, nc_2) |
| `breaker_three_pole` | pole1 (l1, t1), pole2 (l2, t2), pole3 (l3, t3) |
| `thermal_overload_three_pole` | pole1 (l1, t1), pole2 (l2, t2), pole3 (l3, t3), no (no_1, no_2), nc (nc_1, nc_2) |
| `induction_motor_three_phase` | u (u1, u2), v (v1, v2), w (w1, w2) |
| `transformer_three_phase` | primary1 (p1a, p1b), primary2 (p2a, p2b), primary3 (p3a, p3b), secondary1 (s1a, s1b), secondary2 (s2a, s2b), secondary3 (s3a, s3b) |
| `three_phase_load_star` | l1, l2, l3, n |
| `three_phase_load_delta` | l1, l2, l3 |

Each group publishes `<id>/<group>/voltage` (V) then
`<id>/<group>/current` (A), in group order. A nonconducting pole still
has a voltage and has zero current. The contactor adds
`<id>/coil/current_squared` (A²) after the coil pair. Append
`<id>/tripped` (0 or 1) for the breaker, `<id>/thermal_dose`
(dimensionless) for the overload, and `<id>/angular_velocity` (rad/s)
then `<id>/electromagnetic_torque` (N·m) for the motor.
The star load has branch groups l1, l2, l3 from each line to n, then
`<id>/n/current` into n. Delta has groups l12, l23, l31 from line to line,
then l1/current, l2/current and l3/current into the terminal.

Catalog entries for these devices add generated `pinGroups` entries
of the form `{ id, pins: [first, second] }`, in branch order. The
earlier catalog entries omit this field. Group IDs are unique within an
entry and both pins must be distinct members of its ordered pin list.
The same terminal can occur in multiple groups for a star or delta load.
The geometry DTO continues to supply one grid coordinate per pin, and
the Engine rejects missing, duplicate or foreign geometry pins.

Add generated quantities meanSquareCurrent, thermalDose and
electromagneticTorque, units ampere_squared and second, and
the positivePhaseSequence sign convention for signed motor outputs.
Reuse newton_meter and the existing current, voltage, flag and unsigned
conventions elsewhere.
The Engine publishes one discrete record per switching device.
Reuse the OCF2 contactor record for pickup, fuse record for the
overload latch and switch record for breaker operator position and
actual conduction. The numeric breaker trip slot distinguishes a trip
from a manual opening. Transition time is the last latch change for
the contactor/overload and the last conducting change for the breaker.
Initial state without a change has no transition timestamp. OCF2 and
OCT1 keep their existing envelopes and bounds.

## 3. Switching models

A closed path follows i = v/Rclosed. An open path contributes no
conductance. Existing island/reference, solve and diagnostic rules
apply, including when an ideal inductive path cannot be interrupted
with the available circuit. Every device has one linked state.
Derive all of its contact stamps together. Collect simultaneous
transitions in stable component-ID order and settle through the
existing event loop. Same-time solves never advance continuous state
and every published snapshot contains the settled set of poles.

**Contactor.** The driven coil is v = Ri + L·di/dt. Its AC-aware
detector satisfies τ·dx/dt = i² − x. For each advancing substep use the
exact held-input update with the average of the previous committed
and new pre-transition squared currents as input. Initialize x = i(0)²
from the DC operating point. Pick up at x ≥ Ipickup² and release at
x ≤ Idropout². Pickup closes all three power poles and auxiliary NO,
and opens auxiliary NC. Release reverses those states. Reject direct
operator actuation because the coil controls the latch.

**Breaker.** Conduct when the operator is closed and the trip latch
is clear. At compile and each tentative settle while conducting, trip
if the absolute instantaneous current of any pole reaches the
threshold. All poles open together. An explicit open command resets
the latch and leaves the poles open. Refuse close while latched.
Closing on an overcurrent can succeed with a trip in the same settled
reply. This is an instantaneous trip model.

**Thermal overload relay.** All main sensing paths retain their
series resistance in both states. Feed the maximum squared phase
current to the same first-order detector as the coil, using the
maximum at each endpoint before averaging. Start detector and dose d
at zero. Let q be the detector output divided by rated current squared.
For q > 1, update d to min(1, d + h·(q−1)/K). Otherwise update d to
d·exp(−h/coolSeconds). Hold time to trip at constant q > 1 from dose
d is K·(1−d)/(q−1), plus the effects of detector startup and threshold
sampling when measured from switch-on. At d = 1 latch, open auxiliary
NC and close auxiliary NO. Cooling continues when latched. Accept
one pressed action to reset only when latched, d ≤ resetDose and q ≤ 1.
Reset clears the latch and keeps the accumulated dose. An external
contactor controlled by the auxiliary path interrupts the motor.

## 4. Induction motor

Use a symmetrical linear squirrel cage referred to the stator, with
three externally wired stator windings. Connect U2/V2/W2 for star, or
U2–V1, V2–W1 and W2–U1 for delta. Map winding variables to stationary
αβ0 with the power-invariant Clarke rows
√(2/3)·[1, −1/2, −1/2], √(2/3)·[0, √3/2, −√3/2], [1, 1, 1]/√3.
Use the transpose to return to winding variables. For the αβ vectors,
with J90(x,y) = (−y,x) and p pole pairs, solve

```text
Ls = Lls + Lm                   Lr = Llr + Lm
λs = Ls·is + Lm·ir              λr = Lm·is + Lr·ir
dλs/dt = vs − Rs·is
dλr/dt = −Rr·ir + p·ωm·J90(λr)
λs0 = Lls·is0                   dλs0/dt = vs0 − Rs·is0
Te = p·(λsα·isβ − λsβ·isα)
Jm·dωm/dt = Te − B·ωm − Tload
```

Solve the currents from the positive-definite inductance matrix.
Positive phase sequence produces positive speed. Load torque is signed
and subtracted from shaft torque. The power-invariant transform makes
Te·ωm the electrical-to-mechanical power transfer. At compile,
initialize rotor and stator currents and speed to zero. Stamp the zero
winding currents at t = 0 using existing initial-state diagnostics
for incompatible ideal constraints.

The independent balanced steady-state oracle is Rs + jωe·Lls in
series with the parallel branches jωe·Lm and Rr/s + jωe·Llr, where
s = 1 − p·ωm/ωe. At zero slip the rotor branch is open. At nonzero
slip Te = 3·Ir,rms²·(Rr/s)·p/ωe. Compare the locked-rotor case s = 1
with a fixed shaft in a model test and the stable point Te = B·ωm + Tload.
No shaft-lock control is added to the editor. This oracle applies only to
steady balanced operation. The Engine always solves the time-domain
equations. See [MathWorks' squirrel-cage equations](https://www.mathworks.com/help/simscape-electrical/ref/inductionmachinesquirrelcage.html)
for the underlying flux linkage model.

## 5. Transformer and loads

The transformer has three independently coupled primary/secondary
banks with external star or delta wiring. First pins are dotted. Let
a = Np/Ns, Lp = Llp + Lm, Ls = Lls + Lm/a² and M = Lm/a. Each bank uses
[vp, vs]ᵀ = diag(Rp, Rs)·[ip, is]ᵀ + [[Lp, M], [M, Ls]]·d[ip, is]ᵀ/dt.
Positive leakage guarantees positive definite coupling. Copper
resistance and finite magnetizing current affect the measured ratio.
Windings share equations and retain separate galvanic references.
The DC operating point seeds winding currents.

The star and delta loads use independently parameterized series
R–L branches, v = Ri + L·di/dt, with an algebraic resistor at L = 0.
Use DC operating-point current for the dynamic branches. Star n is
a physical pin that can float. Delta terminal currents follow KCL.

## 6. Parameter contract

All scalars are finite and ranges are inclusive. Use SI in DTOs.
Set bounds in the Rust catalog. Existing parameter IDs retain their meanings.

| Parameters | Bounds and defaults |
|---|---|
| closedContactResistanceOhms (switching devices) | 0.0001–10 Ω, default 0.01 Ω |
| coilResistanceOhms, coilInductanceHenries | 0.1–100000 Ω, 230 Ω; 0.00001–100 H, 0.2 H |
| pickupCurrentAmps, dropoutCurrentAmps, coilSenseSeconds | 0.001–100 A, 0.7 A; 0.0001–100 A, 0.4 A; 0.005–2 s, 0.02 s. Require dropout < pickup |
| initialClosed, instantaneousTripAmps | optional 0 or 1, default 0; 0.1–100000 A, 100 A |
| senseResistanceOhms, ratedCurrentAmps | 0.0001–10 Ω, 0.005 Ω; 0.1–10000 A, 10 A |
| thermalSenseSeconds, tripScaleSeconds, coolSeconds, resetDose | 0.005–2 s, 0.02; 0.01–10000 s, 10; 0.01–10000 s, 30; 0–0.99, 0.25 |
| statorResistanceOhms, rotorResistanceOhms | 0.01–10000 Ω, 1 Ω each |
| statorLeakageHenries, rotorLeakageHenries, magnetizingHenries (motor) | 0.00001–10 H, 0.005 H each leakage and 0.1 H magnetizing |
| polePairs, rotorInertiaKilogramMetersSquared, viscousFrictionNewtonMeterSecondsPerRadian, loadTorqueNewtonMeters | integer 1–8, 2; 0.0001–100 kg·m², 0.05; 0–100 N·m·s/rad, 0.01; −1000–1000 N·m, 0 |
| turnsRatio, primaryResistanceOhms, secondaryResistanceOhms | 0.01–100, 2; 0.001–10000 Ω, 0.1 Ω each |
| primaryLeakageHenries, secondaryLeakageHenries, magnetizingHenries (transformer) | 0.00001–10 H, 0.001 H each leakage and 1 H magnetizing |
| phase1ResistanceOhms, phase2ResistanceOhms, phase3ResistanceOhms | 0.001–1000000 Ω, 100 Ω each |
| phase1InductanceHenries, phase2InductanceHenries, phase3InductanceHenries | 0–10 H, 0 H each |

Add a generated finite integer domain for polePairs. Engine and
inspector enforce its catalog bounds. The motor model requires
|ωm| ≤ 3000 rad/s and each flux coordinate within ±100 V·s.

## 7. Atomic updates and numerical behavior

Use coupled trapezoidal stamps and the existing bounded nonlinear
solve for the motor, including speed, torque and currents at the
right endpoint. Include coupled dynamics in the Stage 3 rate bound
as described in the [solver notes](../solver/stage-10-equipment.md).
Same-time resolves obey the existing event limit and cannot change
stored flux, coil current, detector or dose.

Only motor load torque, contactor pickup/dropout thresholds and breaker trip
threshold are live parameter edits. Route all other edits through
compile, including changes to initial settings and stored-energy
coefficients. Apply this classification per device, so shared parameter
IDs preserve their live behavior on earlier devices. The Engine refuses
direct live updates to compile-only inputs.
Successful live edits recompute the latest public step from its
committed predecessor. Breaker open/close and overload reset settle
at the current time without integrating twice. Compile restarts
the runtime state. Successful edits revise the latest trace sample
and preserve earlier samples. The normal run and revision checks,
publication order, error notices and rollback rules apply.

Use referenced InvalidParameter for invalid scalars or relationships,
unsupported operations or prohibited resets. Use existing topology
and initialization diagnostics for invalid geometry or ideal constraints.
Use ModelRangeExceeded for motor limits, NonFiniteState for nonfinite
state, NonConvergence for failed nonlinear solves and EventChatter for
unsettled contact transitions. Refused edits and failed steps preserve
the committed parameters, states, samples, revision and time. Replays
with identical inputs and events in one environment are bit-identical.
Cross-platform comparisons use tolerances.

## 8. Known answers and deferred scope

[#178](https://github.com/SWOFART/open-circuit/issues/178) pins independent
analytical and ngspice results for these fixtures. Record tool version,
hash, options, signs, inputs, exact observation steps and deliberate
regeneration commands. Use behavioral sources and coupled inductors
where needed. See the [ngspice documentation](https://ngspice.sourceforge.io/docs.html).

| Fixture | Acceptance |
|---|---|
| 50/60 Hz contactor, above pickup and below dropout | Mean-square at steady state within 0.5% of the independent detector waveform at the same phase. Transitions within 2 ms. Linked states exact |
| Manual and overcurrent breaker | Trip at first sampled crossing. All poles and reset interlock exact. Use ≥1% current margin except near the crossing |
| Thermal overload at 1×, 1.5×, 2×, 3× rated DC, plus balanced AC | No trip at rated DC. Trip delay within max(2 ms, 1%) of detector/dose reference. Cooldown and reset exact |
| Motor blocked rotor, run-up and steady state | RMS winding current within 2%. Torque within max(0.05 N·m, 3%), speed within max(0.5 rad/s, 2%), and time to 90% settled speed within max(5 ms, 5%) |
| Transformer no-load and unbalanced load, star and delta R–L loads | RMS voltage/current within 0.5%, phase within 0.3° at 50/60 Hz. Verify terminal KCL and compare linked and floating neutral |
| Direct-on-line and star–delta starters | Draw from blank. Check exact interlocks, overload NC, break-before-make and compare current/speed to motor tolerances. Include missing interlock or swapped-phase variants with distinct diagnostics or readings |

Specify a finite winding-current return path in each starter fixture
that opens an inductive branch. For example, draw and document a
resistive shunt across each winding and include it in both independent
oracles. A variant with an incompatible ideal interruption exercises
the existing failure diagnostic.

Earlier regression tolerances apply. The stage record will distinguish
three-platform automation from native desktop observations. Deferred
work includes protective earth and installation safety, manufacturer
protection curves and import, detailed thermal models, saturation,
harmonics, arcs, frequency converters and more detailed rotor models.
