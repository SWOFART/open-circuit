# Stage 3 Transient Analysis and Oscilloscope Contract

**Status:** Draft for review by the Engine and Application workstreams (issue #44).
**Scope:** Ideal linear capacitors and inductors, transient stepping, and
bounded oscilloscope traces. This document extends, but does not replace,
[ARCHITECTURE.md](../../ARCHITECTURE.md), [stage-1.md](stage-1.md), and
[stage-2-editor.md](stage-2-editor.md). Where it states a rule for a Stage 1 or
Stage 2 topic, section 1.2 lists the earlier text it amends. The physical
models and fixtures are in [../physics/models.md](../physics/models.md).

## 1. Boundaries and compatibility

### 1.1 What Stage 3 adds

- Two component types: `capacitor` and `inductor`, ideal and linear.
- Transient analysis as the single simulation mode: the Engine advances the
  circuit in time from an initial state. A circuit without a capacitor or
  inductor is a transient run whose state never changes.
- Optional initial conditions: a signed capacitor voltage and a signed
  inductor current, stored in the schematic.
- Oscilloscope traces: bounded, time-ordered voltage and current histories
  produced by the Engine, independent of the 60 Hz frame cap.

### 1.2 Earlier text this contract amends

| Earlier text | Amendment |
|---|---|
| stage-1.md section 1: "Stage 1 supports resistors and constant independent voltage sources. Capacitors, inductors, ... remain out of scope"; stage-2-editor.md section 1: "Stage 2 supports only the catalog types already present in Stage 1"; the doc comment on `ComponentSpec` in `crates/engine` ("Version 1 supports only the resistor and constant independent voltage source") | Version 1 also supports `capacitor` and `inductor` (section 2). Nothing else those scope statements exclude (nonlinear devices, AC analysis, adaptive stepping) changes. The doc comment is updated with the implementation. |
| stage-1.md section 2: "NaN, positive/negative infinity, null, unknown parameters, and out-of-domain values are rejected"; "the resolved parameter value passed to Engine must always be explicit" | Still true, with one exception: the initial-condition parameters of section 2.3 may be **absent**, which means "not specified". `null` stays invalid. |
| stage-1.md section 3: "Stage 1 devices are memoryless" and "a rejected update leaves the previous valid compiled state unchanged" | Devices now have state. Section 3.4 states what a parameter update does to a running circuit. A rejected update still leaves the previous valid state and result unchanged. |
| stage-1.md section 4, set parameter: "return a new snapshot under the same layout" | Unchanged for live parameters; initial-condition parameters are refused (section 3.4). |
| stage-2-editor.md section 1: "A necessary schema change requires an explicit version change, migration, generated-type update, tests, and review by both workstreams" | Stage 3 treats the new component types and the optional parameters as additive, not as a change to the meaning of anything in version 1, so the version stays 1 and no migration exists (section 1.3). The generated types, tests, and review by both workstreams still apply, and the choice is listed in "Points for review". |
| stage-2-editor.md section 2.2: "All component parameter values are serialized explicitly" | Except an unspecified initial condition, which is omitted. |
| stage-2-editor.md section 3.2 and ARCHITECTURE.md section 5: a lone parameter edit goes through the lightweight update | Except an initial-condition parameter, including setting or clearing it, which always recompiles (section 3.4). ARCHITECTURE.md is updated. |
| stage-1.md section 8 (desktop bridge): the compile result and the reply to `set_parameter`, and the commands | The compile result and the successful `set_parameter` reply gain `internalSubsteps` (section 4.2), and trace retrieval is added as new commands (section 7.4). Both are specified in the bridge issue (#48) as additions to section 8; frames and their envelope are unchanged. |
| ARCHITECTURE.md section 4: "Topology change: fast recompile with state transfer ... keyed by component ID" | Deferred. In Stage 3 every successful compile starts a new run at time zero (section 3.4). ARCHITECTURE.md is updated in the same change. |

### 1.3 Saved project

The project file is still one version 1 schematic (stage-2-editor.md section
2.2). Stage 3 changes it only by allowing the two new component types and the
optional initial-condition parameters. A project file never contains:

- the state the simulator computes (capacitor voltage and inductor current at
  any time after zero, node voltages, currents, energies);
- trace samples, trace history, or the oscilloscope's selected signals, window,
  or scale (those are editor state);
- the number of internal substeps or any other solver setting.

**Schema version.** The version stays `1`. The change is additive: files
without the new types read as before, and an older reader rejects a file that
uses them with its existing "unknown component type" diagnostic, never reading
it as something else. Reviewers should confirm this choice (see "Points for
review" at the end).

### 1.4 Out of scope

Non-ideal parts (ESR, ESL, leakage, winding resistance, saturation),
mutual inductance, nonlinear devices, switches and relays, AC/phasor
analysis, adaptive global time steps, carrying state across recompiles,
oscilloscope triggers and cursors, and sampling faster than the public step.

## 2. Components

All values are SI. For every two-terminal component the Stage 1 conventions
hold: `voltage = V(pin1) - V(pin2)`, current is positive entering `pin1`, and
`power = voltage x current` (positive means absorbed).

### 2.1 Capacitor

| Item | Definition |
|---|---|
| Type ID | `capacitor` |
| Pins | `pin1`, `pin2`. The pins are interchangeable physically; `pin1` is the reference for the sign of the voltage and of the initial voltage. The symbol marks `pin1`. |
| Required parameter | `capacitanceFarads`: finite and strictly greater than zero. Catalog default `1e-5` (10 uF). |
| Optional parameter | `initialVoltageVolts`: finite, signed, voltage `V(pin1) - V(pin2)` at time zero. Absent means "start from the operating point". |
| Equation | `q = C v`, `i = C dv/dt`. |
| State | The capacitor voltage `v`. The Engine also keeps the branch current `i` of the last internal step, which the trapezoidal rule needs. Neither is saved. |
| Stored energy | `E = C v^2 / 2` (joules). |
| Zero or negative `C` | Rejected with `InvalidParameter`. An open circuit is drawn by leaving the pins unconnected. |

### 2.2 Inductor

| Item | Definition |
|---|---|
| Type ID | `inductor` |
| Pins | `pin1`, `pin2`; `pin1` is the reference for the sign of the current and of the initial current. The symbol marks `pin1`. |
| Required parameter | `inductanceHenries`: finite and strictly greater than zero. Catalog default `0.1` (100 mH). |
| Optional parameter | `initialCurrentAmps`: finite, signed, the current entering `pin1` at time zero. Absent means "start from the operating point". |
| Equation | `lambda = L i`, `v = L di/dt`. |
| State | The inductor current `i`. The Engine also keeps the branch voltage `v` of the last internal step, which the trapezoidal rule needs. Neither is saved. |
| Stored energy | `E = L i^2 / 2` (joules). |
| Zero or negative `L` | Rejected with `InvalidParameter`. A short circuit is drawn with a wire. |

### 2.3 Catalog metadata

The Engine's catalog stays the single source for parameter names, units,
defaults, and domains. It gains the following, all additive:

- `Unit` gains `farad` and `henry`. The initial-condition parameters use the
  existing `volt` and `amp`.
- `ParameterDefinition` gains two flags:
  - `optional`: the parameter may be absent. Its `defaultValue` is only the
    value an editor offers when the user chooses to set it (`0`); the Engine
    never applies it implicitly.
  - `initialCondition`: the parameter only sets the state at time zero. The
    Engine refuses to change it on a running circuit (section 3.4).
- Parameter IDs: `capacitanceFarads`, `initialVoltageVolts`,
  `inductanceHenries`, `initialCurrentAmps`. Message keys follow the existing
  scheme (`component.<typeName>.<short parameter name>`, as in
  `component.dcVoltageSource.voltage`): `component.capacitor.name`,
  `component.capacitor.capacitance`, `component.capacitor.initialVoltage`,
  `component.inductor.name`, `component.inductor.inductance`, and
  `component.inductor.initialCurrent`.
- Components keep the Stage 1 identity rules: a stable UUID per component,
  unchanged by moving or rotating it, and a new one for a new component.

The editor builds its Inspector from this metadata (stage-2-editor.md section
3.2). For an optional parameter it shows an explicit "set initial value"
choice, labelled with the sign reference (`V(pin1) - V(pin2)` or "into pin1"),
and removes the parameter from the document when the user clears it.

### 2.4 Example

~~~json
{
  "id": "00000000-0000-4000-8000-000000000011",
  "typeId": "capacitor",
  "position": { "x": 4, "y": 0 },
  "rotationDeg": 0,
  "parameters": { "capacitanceFarads": 1e-5, "initialVoltageVolts": 0 }
}
~~~

An inductor looks the same with `"typeId": "inductor"` and
`"parameters": { "inductanceHenries": 0.1 }`. Omitting `initialVoltageVolts`
(or `initialCurrentAmps`) leaves the initial state to the operating point.

## 3. Time, state, and initialization

### 3.1 Clocks

- The public step is `timeStepSeconds = 0.001` (1 ms). `simulationStep` counts
  public steps from zero, and `simulationTimeSeconds = simulationStep x 0.001`.
  These definitions are unchanged from stage-1.md sections 3 and 5.
- Internal substeps (section 4.2) are invisible: a public step is always the
  smallest unit of simulation time that the API, a frame, or a trace sample
  names.
- Wall-clock pacing, the 60 Hz frame cap, and the slow-down factor map model
  time to real time. They never change the public step, the internal substeps,
  or what a sample contains.

### 3.2 Initialization at time zero

Compilation produces the state at `simulationStep = 0` in two steps: the state
of every capacitor and inductor, then the quantities of the whole network at
that state.

**Step 1: states.**

- A capacitor or inductor **with** an explicit initial condition has that state:
  `initialVoltageVolts` or `initialCurrentAmps`. An explicit condition
  **overrides** the operating point of that component; the circuit need not be
  in equilibrium (the agreed baseline in
  [models.md](../physics/models.md)).
- If at least one capacitor or inductor has **no** explicit condition, the
  Engine solves the **DC operating point** of the circuit: every capacitor an
  open circuit, every inductor a short circuit (an ideal voltage constraint of
  zero with its current as an unknown), and every explicit value ignored. The
  solve is the Stage 1 one, island by island, with the same reference rules.
  The state of each unspecified component is its voltage or current in that
  solution. If the solve fails, compilation fails with the Stage 1 diagnostics
  of that solve (`FloatingNode`, `InconsistentIdealConstraint`,
  `ShortCircuit`, or `SingularSystem`).
- If every capacitor and inductor is explicit, no DC solve is done. A circuit
  with no capacitor or inductor is solved once, as in Stage 1, and that
  solution is its snapshot.

A circuit that has no DC operating point (an ideal voltage source directly
across an inductor, or two capacitors in series with nothing else on the node
between them) therefore needs **every** state to be explicit.

**Step 2: quantities at step 0.**

- If no capacitor or inductor has an explicit condition, the DC solution of
  step 1 is the snapshot at step 0: capacitor currents and inductor voltages
  are zero.
- Otherwise the network is solved once more with every capacitor imposed as an
  ideal voltage source of its state and every inductor as an ideal current
  source of its state (current entering `pin1`), with the same reference rules.
  The result gives every node voltage and branch current, including the
  current of each capacitor and the voltage of each inductor, and is the
  snapshot at step 0. A capacitor whose unspecified state came from the DC
  solve may carry a current here, for example in a series RLC circuit with an
  explicit inductor current.

When no explicit condition is given, the run starts at the DC operating point.
No impulse is modeled: the state at step 0 is the state of step 1, and the
equations apply from there.

### 3.3 Initialization failures

Only step 2 with at least one explicit condition can fail beyond step 1. It
fails in these ways, checked in this order:

1. **The imposed states conflict with the ideal constraints**: capacitor
   voltages around a loop of ideal voltage constraints that do not add up, or
   inductor currents at a node (or any cut) that do not satisfy KCL. The Engine
   reports `InconsistentState` and references the explicit initial conditions
   that participate in the conflict. It never changes the values. A conflict is
   `InconsistentState` only if the loop or cut contains at least one imposed
   state; a conflict among ideal sources alone (for example two unequal ideal
   sources in parallel, found when every state is explicit and no DC solve
   runs) is `InconsistentIdealConstraint`, as in Stage 1. A component whose state
   came from the DC solve takes part, so one explicit member of a series chain
   of inductors conflicts with the DC value of the others.
2. **The imposed states are consistent but leave the equations redundant or a
   node undetermined.** The Engine reports `SingularSystem` or `FloatingNode`
   with the Stage 1 meaning. A conflict (item 1) always takes precedence over
   these.

Item 2 is a **known limitation of Stage 3**: explicit initial conditions are
not supported where they pin the same quantity twice or leave a node with only
current sources on it. Examples: an explicit capacitor voltage equal to the
voltage of an ideal source directly across it, two capacitors with explicit
voltages in a loop with an ideal source, parallel capacitors with the same
explicit voltage, and series inductors with explicit currents (the node between
them has only current sources). Leaving a component unspecified starts it from
its DC state instead, and a source with a nonzero internal resistance (the
catalog default) is not an ideal constraint, so a loop through one is not
redundant. A live parameter update after step 0 is not affected (section 3.4).

`FloatingNode` is defined in stage-1.md section 4 as a node left unconstrained
after reference selection and the device equations. The Engine issues extend
its detection to nodes left unconstrained by open capacitors in the DC solve
and by current sources in step 2.

Compilation has no usable result when an error-severity diagnostic exists
(stage-1.md section 4). Warnings (`NoGroundReference`, `StepAccuracyLimited`)
accompany a valid result.

### 3.4 Compile, parameter updates, and restart

| Operation | Effect on a transient run |
|---|---|
| Successful compile | Starts a new run: new `runId`, step 0, initialization from section 3.2, a new trace history, and no state carried from any earlier run. A failed compile leaves the previous run, its stream, and its traces untouched. |
| Restart | There is no separate command. Restarting is compiling the same document again with a new document revision. |
| `set_parameter` of a live parameter (resistance, source voltage and resistance, `capacitanceFarads`, `inductanceHenries`) | Same run. The update takes effect from the **start of the current public step**. If the latest snapshot is step `N`, the current step is the one from `N - 1` to `N`: the Engine keeps the state at the end of step `N - 1` for this purpose, and computes step `N` again from it with the new value, choosing the substep count again (section 4.2). The kept state holds the capacitor **voltage** and inductor **current** (not the charge or the flux), together with the branch current or voltage that the trapezoidal rule carries (section 2.1), unchanged when `C` or `L` changes; a changed `C` therefore changes the stored charge and a changed `L` the flux. At step 0 there is no earlier state, and the initialization of section 3.2 is run again with the new value. The reply and the next frame carry the new revision, the same `simulationStep`, and the recomputed values. Earlier steps never change, and the recomputed step replaces the sample of that step (section 7.5). A circuit without capacitors and inductors is simply solved again, as in Stage 1. After step 0 this cannot hit the limitation of section 3.3, because the companion models of section 4.1 are conductances, not ideal constraints; it fails only for the Stage 1 reasons. At step 0 it can fail as a compile can: the update is refused, the run is unchanged, and the application resynchronizes with a compile. |
| `set_parameter` of an initial-condition parameter | Refused with `InvalidParameter`, leaving the run unchanged. The application never sends one: it compiles instead, which restarts the run. Setting or clearing an optional initial condition is such a change. |
| `set_parameter` rejected, failed, or unconfirmed | The application resynchronizes with a full compile, as stated for the Stage 2 editor. |
| Any other schematic change (wires, positions, component add or delete, ...) | A compile, hence a new run at time zero. |

The application decides between `set_parameter` and a compile from the
catalog flag `initialCondition`, not from parameter names.

## 4. Numerical method

### 4.1 Trapezoidal companion models

With internal step `h`, `v` and `i` the voltage and current of the branch at
the start of the step, and `v'` and `i'` at its end:

- Capacitor: `i' = G v' - Ih` with `G = 2C/h` and `Ih = G v + i`.
  Stamp the conductance `G` between the pins and the source `Ih` injected into
  `pin1` (and drawn from `pin2`).
- Inductor: `i' = G v' + Ih` with `G = h/(2L)` and `Ih = i + G v`.
  Stamp the conductance `G` between the pins and the source `Ih` drawn from
  `pin1` (and injected into `pin2`).

After the solve, `v'` is read from the node voltages and `i'` from the
relations above; they become the state for the next step. The conductances
depend only on `h`, `C`, and `L`, so they are constant while those are.

The first internal step starts from the consistent state of section 3.2 and
**uses the trapezoidal rule**: no backward-Euler start-up step. Stage 3 has no
events (switches, relays) that would call for one (ARCHITECTURE.md section
3.4), and a start-up step would add phase error to the source-free LC fixture.

### 4.2 Internal substeps

Each public step is split into `n` equal internal steps, `h = 0.001 / n`.

1. At compile and after every live parameter update the Engine computes a
   deterministic rate bound `omegaHat` (rad/s): a number that is **at least**
   the largest magnitude of any natural frequency of the circuit's linear
   dynamics, and that depends only on the compiled circuit's values, never on
   wall-clock time, the frame rate, or the platform. A circuit with no
   capacitor or inductor has `omegaHat = 0`. The Engine's estimator is
   documented in `docs/solver/stage-3-transient.md`; this contract fixes only
   the properties above.
2. `n` is the smallest power of two, `1 <= n <= 64`, with
   `omegaHat x (0.001 / n) <= sqrt(0.03) = 0.1732`.
3. For trapezoidal integration of an oscillation `omega` with step `h`, the
   period is long by about `(omega h)^2 / 12`. The bound in step 2 therefore
   keeps the period error of any oscillation under 0.25%, which is the target
   of the LC fixture. The margin is small by design: at the boundary
   `omegaHat x h = sqrt(0.03)` the predicted error is 0.2495%.
4. If even `n = 64` leaves `omegaHat x (0.001 / 64) > 0.1732`, the Engine uses
   `n = 64` and adds the warning `StepAccuracyLimited` to the result. The
   circuit still runs; very fast modes are not resolved, and the trapezoidal
   rule may leave a fast mode ringing. Neither is hidden from the user.

The chosen `n` is reported to the application as `internalSubsteps` (an
unsigned 32-bit integer, `1` for a circuit without dynamics) in the compile
result and in the reply to every successful live parameter update, so tests and
the UI read it instead of recomputing it. Two Engines given the same document
choose the same `n`.

Because the bound is conservative, a given circuit may run with more substeps
than the minimum that meets an accuracy target. Acceptance (section 8) is
therefore stated for any `n` the rule can choose, and the fixtures check the
result, not `n`.

### 4.3 Accuracy statement

At the public step, each of these holds for every conforming `n`:

- A source-free lossless LC circuit keeps its stored energy to a relative
  `1e-6` over 10 s of model time (the trapezoidal step of a lossless linear
  circuit is an exact rotation, so any drift is round-off).
- The LC period error is at most 0.25%.
- RC and RL step responses stay within 0.1% of full scale of the analytical
  curve for any `n` the rule can choose. The worst case is `n = 1`, which the
  rule picks for the fixtures when the rate bound is tight; it is about 0.031%
  of full scale (see models.md).

These are targets for the fixtures. They are not a general claim about
arbitrary circuits.

## 5. Diagnostics

The structure is the Stage 1 one: a stable code, a severity, a localization
key, and typed references. Stage 3 adds two codes and uses the others as in
stage-1.md section 4:

| Code | Severity | Meaning | References |
|---|---|---|---|
| `InconsistentState` | error | The capacitor voltages and inductor currents imposed at time zero (the explicit initial conditions, together with the DC values of the components left unspecified) cannot hold together with the circuit's ideal constraints (section 3.3). It is also what a live update at step 0 reports, since it runs the initialization again. | Explicit initial conditions that participate in the reported conflict. |
| `StepAccuracyLimited` | warning | The circuit has dynamics faster than the 64-substep limit can resolve (section 4.2). | None. |
| `InvalidParameter` | error | Also: a capacitance or inductance that is not finite and strictly positive, a non-finite initial condition, an initial-condition parameter set on a running circuit, or `null` for a parameter. | The component. |
| `FloatingNode`, `InconsistentIdealConstraint`, `ShortCircuit`, `SingularSystem`, `NoGroundReference` | as in Stage 1 | Applied to the DC solve and the step-0 solve (sections 3.2 and 3.3) and to every later step. | As in Stage 1. |

User-facing prose lives in the UI, keyed by `localizationKey`.

## 6. Result layout and snapshots

- Each capacitor and inductor contributes the same two slots as every
  two-terminal component: `voltage` then `current`, with the Stage 1 sign
  meaning. Slots stay ordered by canonical component ID bytes, then by catalog
  quantity order. The state of a component is its `voltage` (capacitor) or
  `current` (inductor) slot, so no extra slot is added.
- A layout changes only on recompile. A live parameter update keeps it.
- A frame (stage-1.md sections 5 and 8) carries the latest snapshot as before.
  The frame envelope, the delivery rules, and the 60 Hz cap are unchanged.
  `simulationStep` now advances over time for a circuit with dynamics; frames
  still skip steps when delivery is throttled.
- A frame and a trace sample (section 7) with the same `runId` and
  `simulationStep` carry bit-identical values for the same slot, with one
  exception: a live parameter update recomputes the current step, and the
  sample of that step is replaced (section 7.5).

## 7. Oscilloscope traces

### 7.1 Signals

A **signal** is one slot of the run's result layout, identified by its
layout slot `key`, and so by component ID and quantity (`voltage` or
`current`). Its unit (`volt` or `amp`) and sign meaning come from the slot.
The time axis is simulation time in seconds from the start of the run; time
zero is the compile that created the run. Every layout slot is a signal that
can be traced.

### 7.2 Sampling

- The Engine appends one sample for every signal at every public step
  boundary: step 0 after compile, then each time the run reaches a new
  `simulationStep`. The **sample interval** is the public step, 0.001 s, and
  is sent as `sampleIntervalSeconds` so a later stage may refine it.
- Sampling belongs to the simulation, not to the display. It happens inside the
  simulation thread as the run advances, whether or not a frame is built or
  delivered, so the 60 Hz cap, a lagging UI, a closed oscilloscope, and a
  slow-down factor change which frames the UI sees but never which samples
  exist. As in stage-1.md section 8.3, the run advances while a stream is open;
  the bridge issue (#48) decides whether traces get a stream of their own, and
  it must not make sampling depend on which frames were delivered.
- Samples of a run are strictly ordered by `simulationStep`, and their time is
  `simulationStep x 0.001`. A sample is never reordered or repeated.
- Dynamics faster than the public step are not shown (section 1.4).

### 7.3 Storage and bounds

- The Engine keeps a ring buffer per run, holding the most recent
  `historySamples = min(10000, floor(2^24 / (8 x valueCount)))` samples of
  every slot (at least 1). A circuit with up to 209 slots therefore keeps
  10 s of history, and the buffer never exceeds 16 MiB of values.
- When the buffer is full the oldest sample is overwritten. History older than
  the window is gone; the oscilloscope can show at most the window.
- Traces live with the run. When the stream stops, the run stops advancing and
  its traces stay readable; they are discarded when the run is replaced
  (section 7.6). The buffer of a new run is allocated only after its compile
  has succeeded, and the old one is released then, so at most one buffer
  exists at a time.

### 7.4 Retrieval and decimation

This section fixes what a trace message means. The binary envelope, command
names, flow control, and exact limits are specified when the bridge is built
(issue #48) as an addition to stage-1.md section 8, under these rules:

- A response names the `runId`, the `documentRevision` in force at its last
  sample, the layout version, `sampleIntervalSeconds`, and `historySamples`.
- Two kinds of response exist: **history** (the stored window, or a part of it,
  asked for by step range) and **new samples** since the previous response to
  the same request. Both are bounded.
- A request names at most 8 signals, and a response holds at most `maxPoints`
  points per signal (a limit of at most 4096, set by the application), so one
  response holds at most 8 x 4096 points. Issue #48 may lower these limits. If
  the requested samples exceed `maxPoints`, the Engine
  groups them into consecutive buckets of `k = ceil(count / maxPoints)`
  samples. Each point then carries the step of the bucket's first sample and,
  for each signal, the **minimum and maximum** over the bucket, so that peaks
  survive. With `k = 1` the two are equal to the sample. Buckets start at the
  first sample of the response; buckets of different responses are not aligned
  with each other.
- A response whose range starts after samples that were overwritten before they
  were read reports the number of samples missed, so the display can show a
  gap instead of joining across it. No sample is dropped silently.
- A response delivers only finite values, and refers only to the run it names.
  The application keeps at most one pending request per run and applies the
  same acknowledgement discipline as for frames, so a slow webview never makes
  the backend queue traces without bound.

### 7.5 Run and document association

- Every trace message is bound to a `runId`. The application accepts it only
  for the active run, and for the layout version it holds; anything else is
  discarded without an error, like a stale frame.
- `documentRevision` is carried for information. A live parameter update changes
  the revision but keeps the run, so a trace continues across it. Samples
  before the update are never altered. The update recomputes the current step
  with the new parameters (section 3.4) and **replaces** the sample of that
  step; no sample is added, and steps stay strictly ordered. If the replaced
  sample was already delivered, the next response to a request for new samples
  starts again at that step and is marked `replaced`, so the display overwrites
  the last point instead of keeping the stale one.
- The oscilloscope's choice of signals refers to layout slot keys. A key that
  is not in a new layout (the component was deleted or changed type) is dropped
  from the display; a key that is still there is kept.

### 7.6 After a compile or a document change

| Event | Traces |
|---|---|
| Successful compile (every document change except a live parameter update; also a restart, opening a file, New, and an initial-condition edit) | The previous run's traces are discarded. The new run's history starts at step 0. The oscilloscope clears at once and does not show old traces as if they belonged to the new run. |
| Failed compile | The previous run and its traces stay as they were. The user sees the diagnostics. |
| Live parameter update | Traces continue (section 7.5). |
| Resynchronizing compile after a failed update (stage-2 rule) | As a successful compile. |
| No active run (nothing compiled, or the Engine unavailable) | The oscilloscope shows no traces and says why. |

### 7.7 Display requirements for the application

These bind the oscilloscope UI (issue #49) to the contract; they do not add
physics.

- The time axis is labelled in seconds with an SI prefix and measured from the
  start of the run. Each signal's axis is labelled with its unit from the
  layout. Numbers use the display rounding of stage-1.md section 6.1 and are
  never `NaN` or infinite.
- The UI draws what the Engine sent and calculates no physical quantity. It
  may choose a window within the stored history and may draw a min/max envelope.
- Its memory is bounded by the same limits as the Engine's: it never keeps more
  than the window it can request.

## 8. Acceptance

The fixtures, their exact circuits, the reference values, the tolerances, and
the ngspice comparison procedure are in
[../physics/models.md](../physics/models.md) ("Stage 3 acceptance"). In summary:

| Fixture | Reference | Tolerance |
|---|---|---|
| RC charge, RC discharge, RL rise, RL decay | Analytical, every public step to 50 ms; ngspice | 0.1% of full scale against the analytical curve; 0.2% of full scale against ngspice |
| DC initialization, default (no explicit condition) | Analytical operating point, held for 1000 steps | 1 uV and 1 nA, as in Stage 1 |
| Lossless parallel LC | Analytical period 19.8691765 ms and energy 5 mJ; ngspice | Period error at most 0.25%; energy within a relative `1e-6` |
| DC initialization, default (no explicit condition) | ngspice `.op` | 1 uV and 1 nA |
| Initialization diagnostics | The `InconsistentState`, `FloatingNode`, `InconsistentIdealConstraint`, `SingularSystem`, and `StepAccuracyLimited` cases listed in models.md | Exact code; references as listed in section 5 |

## 9. Verification and change control

- **Walkthrough done for this contract.** The fixture values in models.md were
  recomputed from the analytical formulas; the signs were checked against the
  Stage 1 convention (for the LC fixture, `i_C + i_L = 0` at the shared node
  and `i_L(t) = 0.316228 sin(omega t)`; for the RC discharge,
  `i_C(0) = -10 mA`); the trapezoidal errors, period errors, and substep counts
  quoted here were computed with a script and agree with the models.md table;
  and the saved project, the layout, and the trace metadata were checked against
  the Stage 1 and Stage 2 contracts for fields that would put computed state
  into the file.
- **Executable checks come with the implementation issues:** #45 and #46 (Engine
  models, initialization, diagnostics, fixtures), #47 (editor), #48 (bridge and
  traces), #49 (oscilloscope), and #50 (end to end).
- A change to a public field, an ID meaning, a diagnostic, the layout, the
  trace semantics, or the numerical acceptance needs both workstreams' review
  and an update to this contract. A change to an architectural decision also
  updates ARCHITECTURE.md or adds an ADR.

## Points for review

These are the choices a reviewer is asked to confirm or change:

1. **Schema version stays 1** for the additive change (section 1.3). The
   alternative, version 2, would make older readers reject every file instead
   of only the ones that use C or L.
2. **Recompile restarts at time zero** and state transfer is deferred
   (section 3.4), which departs from the ARCHITECTURE.md section 4 sentence.
   The Stage 3 epic ("clears or replaces traces when the run/document
   changes") points the same way.
3. **Reading of "operating point" for an unspecified state** (section 3.2): the
   DC solution with every capacitor open and every inductor short, whatever
   the other components specify, which is the literal reading of the agreed
   baseline. Consequences: a series RLC circuit with an explicit inductor
   current and an unspecified capacitor works (the capacitor starts at its DC
   voltage and carries the current); but a circuit with no DC operating point
   needs every state explicit, and in a series chain of inductors (or of
   capacitors) setting one member without the others conflicts with the
   others' DC values (`InconsistentState`). The alternative, holding explicit
   components fixed while solving for the rest, handles the second group but
   breaks the series RLC case.
4. **A live edit is applied from the start of the current step** (section 3.4),
   keeping the capacitor voltage and inductor current, not charge and flux. The
   alternative, re-solving the current step with every state imposed as a
   source, fails on redundant constraints that are not an error in a running
   circuit.
5. **Substep rule** (section 4.2): power-of-two `n` up to 64, a conservative
   rate bound, and `StepAccuracyLimited` beyond the limit.
6. **Trace sampling at the public step only** (section 7.2), and a history of at
   most 10 000 samples within 16 MiB (section 7.3).
7. **Parameter update replaces the current sample** (section 7.5) instead of
   appending one, and the replaced sample is delivered again with a `replaced`
   mark.
8. **Step 0 with an explicit condition** (section 3.2 step 2): every capacitor
   and inductor is imposed by its state, and consistent but redundant or
   undetermined constraints are not resolved (section 3.3, a known limitation),
   which excludes explicit conditions in a few topologies (series inductors,
   an ideal source across a capacitor). Resolving them would need the
   dynamics at time zero.
