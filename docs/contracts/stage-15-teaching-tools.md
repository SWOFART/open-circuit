# Stage 15 teaching tools contract

The project owner approved the Engine and Application contract for
implementation, as recorded in [#217](https://github.com/SWOFART/open-circuit/issues/217)
under [#160](https://github.com/SWOFART/open-circuit/issues/160).

## Boundaries

The Engine owns measurement, target resolution, model time, and assignment
evaluation. The native app owns scheduling, file I/O and IPC. TypeScript
chooses probe targets and presents replies, clocks, examples and feedback.
Shared DTOs are defined in Rust and generated with ts-rs. This stage keeps
the fixed 1 ms public step, existing substeps, integrator, event ordering,
earlier tolerances, OCF2 and OCT1. Projects stay at schema 3 and store drawn
inputs. Probe selections, transport settings and scores belong to the
session and are neither project inputs nor undoable drawing edits.

## Probe query (#218)

A read-only request names the active `runId`, `documentRevision`, and
observed `frameSequence`. The Engine atomically reads the latest committed
public endpoint, including the settled result of any edit. An older
sequence than the latest is allowed so a continuously advancing run can
be measured. A sequence ahead of it, a mismatch in run or revision, and
an absent run use the existing stale/invalid request diagnostics. The
reply carries the actual run, revision, frame sequence and simulation step
when the request identifies a valid endpoint, including a measurement
refused at that endpoint. Every request specifies exactly one mode and its one
or two targets. The application coalesces repeated reads to at most one
outstanding request and at most 20 requests per second. A probe query
does not advance a run, change trace history or emit a frame.

### Voltage

Each voltage target is an integral schematic grid point and selects a
resolved wire, rail or terminal at that point. All hits must resolve to
one net. At a crossing with two unjoined nets, or at a point without
conductive geometry, the query returns, respectively, `AmbiguousProbe`
or `UnconnectedProbe` with the point. It never chooses a net based on
visual draw order. A resolved but floating continuity island may be
measured internally; across independent voltage-reference domains the
query returns `IncomparableProbeReferences`. Measure the positive target's
potential minus the negative target's potential, in volts, at the same
public endpoint. Targets on the same net yield zero. The Engine retains
committed node potentials and reference-domain identity for queries
separately from the frame layout. A failed solve retains the last
committed data.

### Current clamp

A clamp measures instantaneous current in amperes without inserting an
ammeter stamp or changing the circuit. A terminal target names a component
ID and pin ID and is positive for current from the net into the terminal.
Two-terminal branch endpoints have opposite signs. A multi-terminal device
must expose that terminal's current directly or via a unique modeled branch.
An open contact reports zero when its terminal is connected. An unconnected
target gives `UnconnectedProbe`.

A conductor target names a wire or rail by stable ID and a span by its two
ordered grid endpoints. The Engine resolves spans from the effective
compiled connectivity, splitting the routed conductor at corners and
actual connections such as terminals, junctions, and rail attachments.
Distinct wires at an unjoined crossing stay separate. A wire's
self-crossing remains connected, as in the existing connectivity rules.
The endpoint order defines the
positive direction, and reversing it negates the answer. The app requests
the Engine's selectable span descriptors for the current run and revision.
Return at most 256 descriptors per page in stable order by conductor ID
and then along its route. Use an opaque cursor tied to that run and
revision. The list contains geometry only.

Within the resolved net, remove the selected span in a scratch connectivity
graph. If its endpoints are still joined by another conductor path, report
`AmbiguousConductorCurrent`. If removing it separates the net into two
sides, the current from the first endpoint toward the second is the negative
sum of currents flowing from the first side into attached component terminals.
Each terminal is counted once. If a side has no terminals or implicit
connections, its sum is zero. The graph must preserve the compiled
connectivity, including implicit reference connections and fault effects.
If a required terminal current or implicit connection current cannot be
accounted for, report `UnsupportedProbeTarget`. The computation is bounded
by the existing circuit size limits. The span graph also uses the rail
contact-work budget for segment pairs and probe attachments. Exceeding
that budget reports a model-range diagnostic for the probe without
invalidating the simulation.

### Resistance

Resistance mode targets two points by the same grid rules as voltage.
It measures equivalent DC resistance of a connected, source-free,
resistor-only network using an independent unit-current solve. Inspect
the entire resolved island containing the targets before the scratch
solve. Devices other than ordinary resistors on that island, including
sources, switches, nonlinear elements and stored-energy components,
return `ResistanceUnavailable`. Other independent islands are irrelevant.
A supported pair on the same net reports 0 Ω. Two different passive
islands report `openCircuit` without a numeric value. Reject an
unsupported island before returning a same-net or open-circuit value.
Use the effective compiled topology, including active fault overlays.
Apply the existing Engine circuit size and finite-value limits to the
scratch calculation. A refusal never changes the live circuit or its
trace. The scope is equivalent resistance of educational resistor
networks; circuit break-in and source deactivation are separate exercises.

The query distinguishes a finite reading, an open circuit and a
diagnostic. It returns no numeric value with an invalid result. Use
additive probe diagnostics with existing severity and reference
conventions. Test signed divider voltage, terminal and routed conductor
current, a conductor loop with indeterminate path current, a crossing, an isolated target, incomparable floating domains, source-free
series and parallel resistance, and an energized resistance refusal.
Compare settled numerical readings with independent calculations within
`1e-8 + 1e-3 × |expected|` in the reported SI unit, or a tighter prior
tolerance when applicable.

## Multimeter interaction (#219)

Voltage and resistance use two ordered probes. Current clamp uses one
supported terminal or conductor span. The canvas tool displays pending
and placed targets and allows moving or removing them without changing
the schematic.
A keyboard workflow must offer the same targets and explicitly name the
selected pins, grid points or conductor spans. The mode and polarity
are visible. The UI presents the Engine value and unit and never derives a reading
from other slots.

Use the Engine's instantaneous reading. Absolute values above 30 V or
1 A show an educational range warning while preserving the reading.
Equality is in range. The warning makes no claim about real equipment.
An open circuit reads “Open circuit.” Refusals explain why there is no
reading. Changing mode, target, run, or revision invalidates pending
replies. Reject responses for an earlier selection, a mismatched run or
revision, and frames older than the last accepted probe reply. A probe
value may be older than the latest live frame because requests are
bounded; show its model time. No previous reading is shown while
compiling a replacement run. Test placement, keyboard selection,
polarity, range equality, refusals and out-of-order replies.

In the **Multimeter** panel, choose Voltage or Resistance, then select
the probe and use **Place on canvas**. Use the X and Y fields and the
corresponding Place button for keyboard placement. Current clamp offers
a terminal or conductor-span selector. **More conductors** loads the
next page, and **Reverse clamp direction** exchanges the span endpoints.
Escape returns to the Select tool.

## Playback and single step (#220)

The native playback state is scoped to the active run. New runs begin
playing at 1×. Play and pause are idempotent. Single step is accepted
only while paused and asks for exactly one existing public step. It
returns the resulting snapshot through the same encoding, ordering and
validation as live frames. A failed advance uses the existing stopped
run behavior and keeps the last committed result. A pause stops wall
clock accumulation; time spent paused is never replayed as catch-up.
Changing speed resets the scheduling origin at the current endpoint.
Supported wall-clock multipliers are 0.25×, 0.5×, 1× and 2×. The speed
only changes the number of unchanged public steps due over wall time.
The renderer's frame cap, acknowledgement and trace bounds still apply.

Serialize playback commands with compile, update, and tick in the app
worker. Reject a command with a stale run or document revision. A paused
operator action is applied and settled at the current public endpoint
using the Engine's existing action semantics for that device, including
the established newest-interval rule where applicable. It returns its
current-step snapshot and updates that step's trace according to the
existing update rules. It does not advance a step or silently resume.
A live parameter update follows the existing same-step behavior.
A compile-only edit produces a new run and uses the new-run defaults.

The Tauri command `control_playback` takes `runId`, `documentRevision`,
and `action` as strings. Supported actions are `play`, `pause`, `step`,
`0.25`, `0.5`, `1`, and `2`. The command acknowledges success or
returns an error. Result frames continue on the existing frame channel.

Tests use a fake clock and real Engine to cover paused wall time,
deterministic single/batched advances, pause-time actions, stale commands,
speed changes, and trace/stream ordering.

## Example library (#221)

Ship a local, versioned manifest with `schemaVersion: 1` and an ordered
`entries` array. Each entry has a unique kebab-case `id`, a display
`title`, a short `description` and a path to a bundled schema-3
`.ocircuit` resource. The manifest is a build-time catalog with at most
64 entries; resources resolve beneath its bundle directory. Validate
each resource with the same project reader and limits used by Open.
Unknown manifest versions, duplicate IDs and missing resources fail the
catalog check. The initial set is divider, RC, relay latch, start-stop,
rail circuit and timed three-phase starter.

Choosing an example uses the existing unsaved-change workflow and creates
an untitled editable project backed by a new document instance. Use Save
As for the first save. Example opening never sets the bundled resource
as the project's output path. Geometry and simulation compile normally;
premade result snapshots and trace histories are absent. Each bundled
example has a test that opens, compiles and advances it through the real
Engine. At least one assertion per example checks a known circuit result
or milestone. Assignment restrictions on opening other documents still
apply.

## Assignment hints and scoring (#222)

Assignment schema 1 and its existing evaluation results retain their
behavior. A new assignment schema 2 opts into scoring for `build`
tasks. Its grading metadata is an explicit `grading` object with a
`criteria` array and a `hints` dictionary. The array contains exactly
one record per authored check, with `criterionId`, integer `points`
from 0 to 1000, and optional `hintKey`. Keys are case-sensitive
ASCII letters, digits, hyphens and underscores, 1–64 characters. The
dictionary has at most 64 entries and maps each key to nonblank plain
text, at most 500 Unicode scalar values. Every referenced key must
exist, criteria must identify existing checks exactly once, and the
total possible points must be positive. Existing limits on check count,
project size and time apply. Schema 2 rejects unknown fields and
unsupported task kinds. Saving an unchanged legacy task keeps schema 1.
The teacher explicitly enables scoring to create schema 2.

The Engine first runs the existing evaluator. The schema-2 reply has
a `schemaVersion` of 2, a `result` containing the existing result intact,
and a `score` with integer `earned`
and `possible` totals and a `criteria` array in authored check order.
Each row contains `criterionId`, `earned`, `possible` and an optional
`hint` with `key` and `text`. The legacy reply retains its existing
shape. A passing criterion earns its assigned points. Failed and `notEvaluated` criteria
earn zero. A locked-input failure or compilation failure sets all
earned points to zero; the possible total stays fixed and the existing
outcome explains the failure. A run that stops after compilation can
still earn points for criteria already passed. The locked-input guard
never awards points. A failed evaluated criterion can return its
teacher-defined `hintKey` and mapped literal text. Passing or
unevaluated criteria do not return hints. Locked-input and compile
diagnostics keep their existing presentation. No grade is persisted
with the student's drawn circuit.

Teacher UI validates point values, defines hints and previews the
grading configuration, then uses native assignment persistence. Student
UI presents per-criterion earned and possible points, the total and
applicable hints from the Engine result. Hint text is rendered as text.
There is no attempt counter or server. Tests cover legacy round-trip and
verdicts, version/limit/reference rejection, mixed verdicts, locked and
compile failure, stopped runs, repeated deterministic evaluation and
teacher/student UI.

## Acceptance (#223)

Draw a canonical circuit using the editor and demonstrate the student
measuring it, pausing and stepping it, then receiving an Engine-computed
score and an applicable hint. Compare against independent measurement,
step and scoring expectations. Add a Stage 15 acceptance record with
local automated checks, native desktop observations and the exact
candidate's Windows, Ubuntu and macOS CI results. Distinguish tests that
actually ran from builds and native observations. Keep earlier stage
fixtures and behavior intact.

## Deferred work

AC meter windows, physical probe loading, current distribution through
parallel ideal paths, equivalent resistance of energized or nonlinear
networks, fault-diagnosis grading, course management, accounts, network
features and grading servers are outside Stage 15.
