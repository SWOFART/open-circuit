# Stage 8 Contract: Named Supply Rails and Cabinet-Style DC Drawing

**Status:** Proposed for Engine and Application owner review (issue #128).
This is a documentation candidate, not a claim of either owner's approval or
implemented rail support. The independent technical review does not replace
their acceptance. Both owners must accept the same contract before #130–#134
implement it; any changed decision requires an updated contract and review.

**Scope:** The DC/named-node portion of Stage 8 in
[ARCHITECTURE.md](../../ARCHITECTURE.md) section 12. A user draws a control
circuit between named vertical conductors; it solves like an explicitly wired
circuit. Stage 7's merged implementation at `96be206` is the inspection
baseline. Stage 7's remaining acceptance/closure requirements still apply.

## 1. Boundaries and amendments

A rail is drawn input, not a physical device, voltage source, reference marker
or result. Rust owns its validation and electrical connectivity. The app owns
native I/O, threading and transport. TypeScript owns drawing and editor
gestures; its connectivity hints never become the Engine's netlist.

Existing DC voltage sources supply the circuit, with their explicit EMF and
internal resistance. Existing reference markers establish the global numerical
reference. An unmarked island retains its deterministic local reference and
`NoGroundReference` warning. A rail changes neither rule.

Names including `L1`, `L2`, `L3`, `N` and `PE` are permitted ordinary text under
section 2. They create no phase relationship, neutral, earth connection,
protective conductor, safety behavior or reserved net. The interface describes
this feature as **DC rails**, defaults to `RAIL1`, and states this limitation in
its help. It offers no three-phase/earth preset or safety indication. AC/phasor
analysis, power poles, PE safety simulation and mechanical cabinet CAD remain
deferred. No IEC compliance or installation suitability is claimed.

| Earlier contract                            | Stage 8 amendment, effective only with implementation                                                                                |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Stage 1 section 2; Stage 2 sections 2 and 5 | Project schema v2 adds required `rails`; strict v1 decoding and migration remain available (section 3).                              |
| Stage 1 section 2.1; Stage 2 section 3.3    | A rail is another exact-grid conductor, with the same crossing rules (section 4).                                                    |
| Stage 1 section 4                           | Add rail diagnostic references and four error codes (section 5); compile accepts the normalized current schema.                      |
| Stage 2 sections 3–4                        | Rail tools, selection and history extend editing without changing existing component transforms (section 7).                         |
| Stage 5 sections 2–4 and 7                  | Freeze v1 assignment files' embedded format; migrate rail-free inputs internally and reject rail use in assignment mode (section 8). |
| Stage 6 section 4.4                         | Rail document edits compile a fresh run; operator actions retain existing live-update semantics.                                     |

No model, device catalog entry, pin ID, parameter, public time step, integration
rule, solver tolerance, event rule, frame kind or trace quantity is added.

## 2. Rail identity, names and geometry

The Engine-owned `RailSpec` has exactly these required, non-null fields:

| Field   | Rule                                                                                                 |
| ------- | ---------------------------------------------------------------------------------------------------- |
| `id`    | Canonical lowercase UUID, unique across components, wires, junctions, reference markers and rails.   |
| `name`  | 1–32 ASCII bytes matching `[A-Za-z0-9][A-Za-z0-9_+-]{0,31}`. Case-sensitive, compared byte-for-byte. |
| `start` | GridPoint at the top of the vertical line.                                                           |
| `end`   | GridPoint at the bottom of the vertical line.                                                        |

Coordinates are signed 32-bit integers as before. Require `start.x == end.x`
and `2 <= end.y - start.y <= 1_000_000`, computing the difference in signed
64-bit arithmetic. Reject reversed endpoints; do not silently reorder an input
file. There is no rotation field. Names are displayed as plain text, never
markup. Whitespace, Unicode, control characters and empty strings are rejected;
neither reader nor writer trims, case-folds or normalizes them.

Names must be unique within the document, even for physically connected rails.
`L1` and `l1` differ. Two `L1` objects are a `DuplicateRailName` error regardless
of their positions. Names are labels, **not nonlocal net aliases**: no
disconnected segments connect by name, and no cross-document/global name table
exists. To continue a rail elsewhere, draw an explicit wire and give the second
rail a different name. A rename changes no electrical connection.

Two differently named rails can be deliberately joined by geometry or a wire;
both names then label one conductor. This is valid, not a name conflict. For
example, wiring `L1` to `N` joins them exactly as wiring two ordinary wires
would. Any resulting source short uses the existing source diagnostics. The
software must not infer a special protection rule from those names.

Moving, resizing or renaming retains the rail ID. New/copy-created objects get
fresh UUIDs. Array order is not identity. Rail IDs never enter the component
catalog, geometry envelope's component list or component result-slot keys.

## 3. Project representation, migration and bounds

### 3.1 Current document and examples

The normalized in-memory and newly saved schematic is **schemaVersion 2**.
It requires all existing arrays and a `rails` array, including when empty.
Component parameters/initial conditions retain their Stage 1–7 forms.
`Schematic` and `RailSpec` are owned by Rust and exported with ts-rs; generated
TypeScript is not edited by hand. The compile envelope supplies only component
pin positions: rail coordinates already live in the schematic.

This complete document is structurally valid. It is intentionally not a powered
circuit; compiling its isolated pinless rail reports `FloatingNode`.

```json
{
  "schemaVersion": 2,
  "components": [],
  "wires": [],
  "junctions": [],
  "referenceMarkers": [],
  "rails": [
    {
      "id": "00000000-0000-4000-8000-000000000080",
      "name": "L+",
      "start": { "x": 0, "y": 0 },
      "end": { "x": 0, "y": 20 }
    }
  ]
}
```

The following rail, substituted into that document, is invalid geometry: its
two x coordinates differ. The reader must reject it rather than straighten it.

```json
{
  "id": "00000000-0000-4000-8000-000000000080",
  "name": "L+",
  "start": { "x": 0, "y": 0 },
  "end": { "x": 1, "y": 20 }
}
```

Also reject `"name": " L+"`, `"rails": null`, a missing `rails` in v2,
an extra `voltageVolts` field on a rail, duplicate object IDs and any unknown
field. Unknown/missing/malformed schema versions never default to v1 or v2.

### 3.2 Version boundary

1. Decode the version before selecting an exact input shape. A v1 file has
   exactly the prior top-level fields and cannot contain `rails`, even empty.
2. Validate v1 with its existing structural/parameter rules, then migrate
   purely and deterministically: set version to 2 and add `rails: []`. Preserve
   all IDs, geometry, array order, parameters and optional initial conditions.
3. Decode v2 strictly and validate its structure. Do not solve during loading.
   A topology error such as a pinless rail does not prevent open/save.
4. Future versions are rejected. Older applications reject v2 instead of
   ignoring the new conductor field. Project save always emits v2; there is no
   lossy project downgrade/export to v1.

The Engine's project decoder/migration code owns this conversion; the desktop
and UI receive the normalized current type. Direct compile/check DTOs require
the current normalized type, rather than guessing the meaning of legacy
requests. Legacy file fixtures pass through the versioned decoder first.

New/Example produce v2 documents. Opening v1 does not rewrite the file or mark
it dirty just for normalization: the saved baseline for dirty comparison is
the normalized document. A later explicit Save writes v2 atomically. Failed
open/save leaves document, path, history and dirty state unchanged.

### 3.3 Input and work limits

Preserve the desktop project limit of 16 MiB and assignment limit of 4 MiB.
Check the actual encoded output size on save; neither limit becomes permission
to drop rails. Add at most **256 rails** per schematic and at most 32 name bytes
per rail at every Engine validation boundary, not only native file open.

Before rail contact processing, check this rail-specific work bound using
checked integer arithmetic:

```text
R * (S + P + J + M) + R * (R - 1) / 2 <= 1_000_000
```

`R` is rail count, `S` is total ordinary wire segments, `P` is resolved component
pin count, `J` is junction count, and `M` is reference-marker count. The second
term is zero for `R = 0` (do not subtract one from an unsigned zero).
This bounds additional rail-pair/contact work; it does not alter the existing
wire-only limits or claim to bound all legacy netlisting work. Overflow or an
exceeded bound is `RailLimitExceeded` before rail intersection work. Apply the
rail-count/name/extent limits without a geometry envelope; apply the contact
budget when compile supplies resolved pins. Do not allocate or iterate once per
grid unit along a rail; test point/segment membership arithmetically.

## 4. Exact-grid connectivity

For connectivity each rail is one closed vertical conductor segment. Its
endpoints are ordinary conductor endpoints. Extend Stage 1's segment rules to
wire/rail and rail/rail pairs; do not use label bounds or screen-pixel tolerance.
Each wire polyline remains a continuous conductor, including self-crossings.

| Geometry at an exact grid point                                         | Connection                                       |
| ----------------------------------------------------------------------- | ------------------------------------------------ |
| Wire endpoint at rail endpoint or interior                              | Yes; ordinary endpoint contact.                  |
| Pin at rail endpoint or interior                                        | Yes; all conductors at that pin join.            |
| Horizontal wire interior crossing rail interior, no pin/endpoint/marker | No.                                              |
| Same crossing with explicit junction                                    | Yes, every conductor at the point.               |
| Same crossing with reference marker                                     | Yes, and part of the explicit global reference.  |
| Wire interior at rail endpoint                                          | Yes; the rail endpoint supplies the contact.     |
| Collinear wire and rail sharing an interval or touching endpoints       | Yes.                                             |
| Two vertical rails overlapping or touching on the same x                | Yes, regardless of their distinct names.         |
| Rails at different x, or same x with a gap                              | No, unless explicitly wired/referenced together. |
| Same name on two rail objects                                           | Invalid input, not an alias or a partial solve.  |
| Reference marker on a rail only                                         | Valid attachment; that conductor is referenced.  |
| Marker not on any pin, wire or rail                                     | Detached under the existing marker diagnostics.  |
| Names `N` or `PE`, with no marker                                       | No implicit reference or connection.             |

A pin or endpoint at an otherwise unconnected crossing joins all conductors
there, as in Stage 1. Collinear contact uses closed intervals, not just endpoint
equality. An explicit junction on a single conductor has no additional
electrical effect but remains a structurally valid saved marker; the Junction
tool still offers only crossings that are not already connected. A junction
with no conductor/pin is `InvalidGeometry`.

After geometric union, all explicit reference markers intentionally identify
the same global node, even when drawn on separate rails. This preserves Stage
1/2: two separately marked supplies are no longer isolated. With no such
deliberate connection, independent islands stay separate; automatic references
never join them. Names are not part of either union operation.

Every derived node retains sorted member pin, wire and rail identities for
diagnostics. Sort nodes with pins by their smallest `(componentId, pinId)` as
before. Sort pinless nodes by their smallest tagged conductor identity
`(kind, id)`, with wire before rail, then canonical UUID bytes. Member lists and
diagnostic references use canonical ID order. Array permutations cannot change
connected sets, reference choice or the resulting ordered diagnostics. Derived
node indices are runtime implementation details, never persisted identities.

A rail joined to a conductor containing pins is not independently floating.
A conductor consisting only of rails/wires and no pins reports `FloatingNode`,
including a bare rail with a reference marker. An unattached end on a rail that
otherwise feeds a circuit is allowed; it is not an error merely because the
line extends past its last attachment. Invalid topology remains editable and
saveable, but any compile error prevents a usable new result.

## 5. Diagnostics, results and transport

Add the Engine-owned generated reference variant
`{ "kind": "rail", "railId": <stable ID> }`. Include every affected rail on
rail name/geometry errors; for duplicate names include both conflicting IDs.
Existing codes keep their severities and meanings. Add these error codes:

| Code                         | Localization key                        | Meaning                                                                          |
| ---------------------------- | --------------------------------------- | -------------------------------------------------------------------------------- |
| `InvalidRailName`            | `diagnostic.invalidRailName`            | Name violates section 2.                                                         |
| `DuplicateRailName`          | `diagnostic.duplicateRailName`          | Two rails have the same exact name.                                              |
| `RailLimitExceeded`          | `diagnostic.railLimitExceeded`          | Rail count/contact budget exceeds section 3.3, including arithmetic overflow.    |
| `UnsupportedAssignmentRails` | `diagnostic.unsupportedAssignmentRails` | An assignment check request contains rails, which this release does not support. |

Use `InvalidGeometry` for rail coordinates/extent, `DuplicateId` for ID
collisions, and `FloatingNode`/`DetachedReferenceMarker` as in section 4.
Unknown JSON fields/type errors are project/request decoding failures, not
successful compiles. A work-budget error references all rails in canonical
order (at most 256). An excess-count error has no individual references, so it
can be returned before sorting/traversing an oversized rail list; the UI names
the document-wide limit in words. Cross-kind ID collisions also reference the other
object. A floating conductor references its member wires/rails and any attached
markers so the drawing can be highlighted. A rail-related source diagnostic
retains the affected component reference and adds the rails on that node.

Rails add no `ResultLayout` slot, analog value, `DeviceState`, trace signal or
device catalog entry. Use existing component voltage/current/state results to
inspect the circuit. Ideal-conductor segment current is indeterminate and must
not be displayed as computed rail current. No UI-side voltage inference exists.
There is no rail-voltage field or rail probe in this release.

Compile/update/snapshot metadata, OCF2 and OCT1 stay unchanged. Rail references
travel in structured command diagnostics, not binary state records. Preserve
finite-value, count/length/layout checks, bounded latest-frame delivery,
acknowledgement/backpressure and the 60 Hz wall-clock cap. Rust pin and device
solving is unchanged after connectivity normalization.

## 6. Simulation synchronization

Creating, moving, resizing, renaming or deleting a rail is a document edit and
uses a complete compile with a new document revision, even for a rename with
unchanged topology. Never send it through numeric `set_parameter`. A no-op
edit creates no revision, undo entry or compile.

A successful compile starts a fresh run at step 0: reset operator positions,
continuous/discrete state and trace history under Stages 3/4/6/7. No state
transfer is introduced. A failed compile leaves the Engine's previous run
unchanged; the UI keeps the new drawing editable and marks its readings/traces
unavailable instead of presenting the previous document's results as current.

Preserve newest-document compile coalescing and equality checks for run,
revision, layout and sequence. Delayed replies/frames/traces cannot overwrite a
newer rail document. Undo/redo compiles the resulting drawing, not an old
snapshot. Live device parameters and START/STOP operators retain their existing
atomic refusal, current-step recomputation, trace replacement and event rules;
rails do not create another command or timing policy.

## 7. Drawing, history and cabinet workflow

### 7.1 Tools and selection

Add an accessible **DC rail** toolbar action, separate from the physical-device
catalog. On selection, choose the smallest unused `RAIL<n>` name for positive
integer `n`. The first click fixes snapped x and one y; the second click chooses
the other y at that x. Order the two y values for the newly drawn preview and
commit only a legal extent. Before the first click, preview a 20-unit line
downward; an out-of-range preview cannot commit. The tool remains active and
selects the next unused default after each commit. Escape cancels the preview;
Escape without a preview returns to Select. R does not rotate a rail.

In Select, hit priority is: selected object's editing handles, junction/reference
markers, components, rails, then ordinary wires. Preserve existing priority
among old object kinds. Within a kind, use nearest hit, with last-drawn winning
an exact tie as today. Rail line/label and handles use the existing fixed
screen-pixel picking tolerance; electrical attachment still uses exact grid
coordinates. Label hit bounds include the actual rendered text extent, and
Fit to circuit includes rail lines and labels. Markers at a rail/wire contact
remain directly selectable.

Expose name, x, top y and bottom y in an accessible rail-properties panel.
Committed name edits validate the grammar/uniqueness; duplicate/invalid input
keeps the field error and does not edit the document. Keyboard users can create
a rail with these fields and a **Create rail** action, edit its coordinates,
select rails through the panel's document-object list, and delete the selection.
Canvas shortcuts never intercept a text field. No new mandatory shortcut is
introduced.

### 7.2 Move, resize, wiring and history

Dragging the line moves both endpoints by a snapped integer offset. Endpoint
handles resize vertically at the existing x; no horizontal rail rotation or
diagonal preview commits. Invalid/out-of-range gestures cancel on release.
Moves/resizes **do not move wires, components, junctions or reference markers**:
attachment is geometric, not a stored constraint. Show the prospective
attachments/detached markers while previewing. This explicit rule avoids
silently moving a whole rung when a rail is shortened. Delete removes only the
rail; wires and markers stay for editing. Existing component-pin wire-following
rules remain unchanged.

The Wire tool may start/end at any grid point on a rail; interior-interior
crossings require a junction unless another section 4 contact exists. Extend
attachment/junction-spot/dot/detached-marker hints in Stage 2 section 3.3.1 to
include rails as conductors. A valid junction spot requires a horizontal and
vertical segment from distinct conductor objects, strictly interior to both,
with no automatic pin/endpoint or existing junction/reference at that point.
Draw a connection dot for an accepted junction or three-arm connected contact;
an unconnected crossing has none. A label supplies no electrical attachment.

Create, rename, move, resize and delete are each one immutable-document history
transaction. Pointer previews/field keystrokes are not transactions. Cancel,
blur during a drag or pointer cancellation commits nothing. Undo during a
preview cancels it first; otherwise undo/redo restores IDs and drawn values
and follows section 6. Saving persists names/geometry but never selection,
history, run positions or results. Open/New clear history as in Stage 2.

### 7.3 Minimum user-visible outcome

From a blank document, draw vertical `L+` and `0V` rails, put an existing DC
source between them, explicitly mark the return reference, and wire STOP,
START, a contactor coil and its seal-in NO contact as section 9.2. Read the
Engine's coil current and device state; press/release START and STOP with the
existing operator controls. Save/reopen the drawing and operate a fresh run.
This is cabinet-style electrical drawing, not physical cabinet layout,
automatic rung generation, a terminal schedule or a new power-device model.

## 8. Assignment compatibility

Rail support in assignments is deliberately deferred. Keep `.ocassignment`
schemaVersion 1 and its exact legacy embedded schematic-v1 format. Its
`startingCircuit`/`reference` decoder validates v1 first and normalizes to the
current v2 type with empty rails using the same pure Engine migration as
projects. Its writer explicitly converts rail-free normalized schematics back
to the legacy embedded v1 shape; it refuses nonempty rails rather than dropping
them. This conversion is only for assignment-v1 embedding, not a project
downgrade. Existing assignment byte limits, error ordering and restrictions
remain unchanged.

Embedded v2 documents are refused with `InvalidField` at
`/startingCircuit/schemaVersion` or `/reference/schemaVersion`; embedded v1
with a `rails` field is `UnknownField` at that field. The live typed assignment
DTO uses normalized v2 schematics; attempting to validate/export one containing
rails is `TaskMismatch` at `/startingCircuit/rails` or `/reference/rails`.
All types/converters remain Rust-owned; frontend code does not maintain a
second legacy schema definition.

While authoring or completing an assignment, disable rail creation and reject
rail-bearing document replacement before changing the active assignment or
student document. Tell the user to leave assignment mode to edit DC rails.
At the public Engine `evaluate_checks` boundary, inspect both `startingCircuit`
and student `schematic` before compilation. If either contains rails, return
the existing `CheckResult` shape using the failed-start path: an error
`UnsupportedAssignmentRails`, with the offending rail references, causes the
overall outcome to fail and every run-dependent criterion to be `notEvaluated`
with no measured number. Static component-count/locked checks keep their
existing semantics. Echo the document revision; do not compile, create a run or
modify the live run. Apply the count limit before collecting references as in
section 5. The app forwards this structured result; a direct Rust caller cannot
bypass the guard. The `check_assignment` return shape does not change, and no
new check kind is introduced.

No rail IDs are valid `lockedComponents`, fault targets, component counts or
diagnosis targets. General DC project compile can still contain rails;
ordinary device faults retain their existing rules outside assignment mode.
Legacy assignments and rail-free student documents keep their existing
electrical meaning and restriction checks.

## 9. Canonical acceptance oracles

Issue #129 supplies executable drawing pairs/manifests, not new expected
numbers generated by the Engine. In each pair retain component IDs,
parameters, geometry-envelope pin IDs and operator events; replace only the
named vertical rail segments with ordinary wire segments at identical
coordinates. Assign fresh noncolliding wire IDs and remove the rails. Names
have no analogue in the wire drawing. Compare mapped connected pin sets and
component signal keys, never derived node indices or wire/rail IDs.

At every listed step require identical discrete states, finite results and
the independent physical tolerances below. Additionally compare the two
layouts' mapped numeric values with `abs(a-b) <= 1e-9 + 1e-9*abs(b)` in each
slot's SI unit. This is an equivalence bound, not a relaxation of any fixture's
physical accuracy. Repeated runs in one environment must be deterministic;
cross-platform results satisfy tolerances, not undocumented CPU bit identity.

### 9.1 DC divider

Use source EMF 5 V, internal resistance 0.1 ohm, R1 = R2 = 1000 ohm.
Source pin1 feeds the positive rail; pin2 feeds the return rail, with a reference
on that rail. R1 pin1 is positive rail, R1 pin2 joins R2 pin1, R2 pin2 is return.
Both source and resistor polarities follow Stage 1's passive sign convention.
At steps 0 and 10:

```text
I = 5 / (0.1 + 1000 + 1000) = 0.0024998750062496877 A
V(R1) = V(R2) = 2.4998750062496877 V
I(source, entering pin1) = -I
V(source terminals) = 4.999750012499375 V
P(source) + P(R1) + P(R2) = 0  (terminal powers)
```

At newest step 10, set R1 to 3000 ohm using the existing live parameter API.
The recomputed step 10 and step 20 have I = 0.0012499687507812304 A,
V(R1) = 3.749906252343691 V, V(R2) = 1.2499687507812304 V.
Voltage tolerance is 1e-6 V, current tolerance 1e-9 A, KCL tolerance 1e-9 A
and summed terminal-power tolerance 1e-8 W. These retain the
[Stage 1 reference](../../tests/golden/stage-1-divider.expected.json) targets.

### 9.2 START/STOP seal-in

Use the exact circuit/parameters of
[stage-6-start-stop-latching.expected.json](../../tests/golden/stage-6-start-stop-latching.expected.json):
ideal 5 V source, STOP NC 0.01 ohm, START NO 0.01 ohm, contactor coil 100 ohm
and 0.01 H, pickup 0.04 A, dropout 0.02 A, closed contact resistance 0.1 ohm.
Positive rail feeds STOP then `(START || NO contact)` then coil a1; coil a2
and source negative join the referenced return rail. Leave NC contact pins
unwired. Retain the file's component pin/sign mapping.

| Request sent when newest step is | Request        | Effective from time |
| -------------------------------- | -------------- | ------------------- |
| 5                                | START pressed  | 0.004 s             |
| 10                               | START released | 0.009 s             |
| 15                               | STOP pressed   | 0.014 s             |
| 20                               | STOP released  | 0.019 s             |

These times denote the start of the recomputed current step, not an off-by-one
shift in the observation samples. At steps 0/3: released, 0 A. At 5/8: picked,
0.04999045636742077 A. At 12: picked, 0.04994506043352313 A. At 15/25:
released, 0 A. Current tolerance 1e-4 A, states exact. The settled currents are
`5/(100 + 0.01 + (0.01 || 0.1))` and `5/(100 + 0.01 + 0.1)`.
Do not require an invented exact internal pickup time: follow the existing
event rules and compare the rail/wire last-transition times to 1e-9 s.

Wrong variant: omit both connections to the NO contact. With the same events,
step 5 is picked with 0.04999000199960008 A; step 12 is released with 0 A
(same 1e-4 A tolerance). It must visibly differ from the correct seal-in run.

### 9.3 RC hold/recharge

Reuse every expected row/event in
[stage-6-button-capacitor-hold.expected.json](../../tests/golden/stage-6-button-capacitor-hold.expected.json).
Ideal 10 V source, NC button 0.01 ohm, R = 1000 ohm, C = 100 microfarad with
explicit initial 0 V; source and series button–R–C span the two rails with a
referenced return. Press at newest step 50, release at 100. With
`tau = 0.100001 s`, the capacitor holds its 49 ms value through 99 ms:
3.8737060396891634 V, then 3.9346630765681274 V at 100 ms and
7.768664929075354 V at 200 ms. Tolerance 0.005 V; initial zero 1e-9 V;
open-branch current 1e-12 A. Preserve the existing recomputation/BE policy;
do not introduce an alternate RC integration formula to fit these numbers.

### 9.4 Isolation, reference and negative geometry

- Two disjoint source/load circuits: ideal 5 V with 1000 ohm and ideal 10 V
  with 1000 ohm. Use four distinct rail names and no reference markers.
  Expect two electrical islands, two `NoGroundReference` warnings, load
  voltages 5/10 V and currents 0.005/0.01 A at steps 0/10 with the divider's
  voltage/current tolerances. Absolute potentials across islands are not
  comparable. Renaming their rails to `N` and `PE` changes no connection or
  warning. Adding explicit markers to both returns deliberately joins them.
- Two same-name rails: structurally invalid with `DuplicateRailName` and both
  rail references; no numerical result. Two distinct names joined physically:
  one node, no name diagnostic. A rename to a third unique name preserves the
  mapped connected pin sets.
- A horizontal segment from (-2, 5) to (2, 5) crossing a rail from (0, 0) to
  (0, 10) stays separate with no marker/pin; a junction at (0, 5) joins them.
  A wire ending at (0, 5), or a pin/reference there, also joins them. A
  collinear wire from (0, 5) to (0, 15) joins by overlap. A rail ending at
  (0, 5) joins the horizontal wire by endpoint contact. Reference at (3, 5)
  with no conductor is `DetachedReferenceMarker`.
- A lone rail (including one with only a reference marker) is structurally
  saveable but compiles with `FloatingNode`; a gap of one integer unit does
  not connect. Overflow/out-of-bound extent and work limits fail before
  solving. Permute the inputs and require the same connected sets and
  canonical diagnostics.

## 10. Verification, acceptance and unresolved approval

This issue changes documentation only. It does not claim that the current
binary reads schema v2 or draws rails. Implementation ownership and validation:

| Issue             | Required evidence                                                                                                                               |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| #129, shared      | Independent numbers above, exact geometry truth table, offline paired fixtures and provenance.                                                  |
| #130, Engine      | Strict v1/v2 decode, normalization and assignment conversions; bounds, malformed cases, round-trip and generated DTO tests.                     |
| #131, Engine      | Connected-set oracles, island/reference tests, mapped physics/equivalence and unchanged old regressions.                                        |
| #132, Application | Real Engine compile/project/service/diagnostic/stale-frame/trace tests, legacy assignment handling and bundle checks.                           |
| #133, Application | Rail rendering, hit/attachment geometry, text bounds, shared truth-table tests and native canvas inspection.                                    |
| #134, Application | Keyboard/pointer authoring, history, assignment guards, persistence/reset, real-Engine fixture generation and user guidance.                    |
| #135, shared      | Exact-candidate three-platform checks/Tauri builds, measured canonical values and actual desktop observations in the Stage 8 acceptance record. |

Owner review must explicitly confirm names without aliases, duplicate-name
rejection, geometric contact/reference rules, schema v2 migration and the
assignment-v1 boundary, rail work limits, fixed wires during rail transforms,
and these numerical fixtures. Neither owner approval is recorded by this
document's existence or by an automated Gate A PASS. The draft PR is the
concrete artifact for that review. Until both workstreams accept it, #128's
owner-acceptance criterion and Stage 8 implementation gate remain pending.

All existing applicable checks remain required for implementation and final
acceptance. Preserve the public 1 ms step, deterministic substeps, coupled motor
physics, nonlinear convergence/rollback and discrete event rules. Scope changes
need both workstreams' review and an updated contract/architecture before code.
