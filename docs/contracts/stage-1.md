# Stage 1 Contract: Schematic, Engine API, and Results

**Status:** Draft. Requires approval from both the Engine and Application workstreams.
**Scope:** The first end-to-end milestone: a constant voltage source and resistors.
This document specifies Stage 1 details under [`ARCHITECTURE.md`](../../ARCHITECTURE.md);
it does not replace that document or define deferred device models.

## 1. Contract boundaries

- The saved project is the user-drawn schematic. Rust derives electrical connectivity,
  node identities, and numerical state when compiling it.
- TypeScript owns symbol drawing and local pin geometry. The Rust catalog owns stable
  component type IDs, parameter IDs, and pin IDs. The compile request includes the
  world-grid position of each pin, derived by the frontend from the symbol geometry.
- The Engine validates the supplied pin IDs against its catalog and rejects missing,
  duplicate, or unexpected pins. A cross-layer contract check verifies that the
  frontend's geometry agrees with the catalog pin IDs and counts.
- `crates/engine` exposes a transport-independent API. It has no Tauri, UI, file-dialog,
  or thread-lifecycle responsibilities.
- All physical quantities use SI units and finite IEEE-754 `f64` values. Unit prefixes
  are a presentation concern and are converted before values cross the UI/Engine boundary.
- Stage 1 supports resistors and constant independent voltage sources. Capacitors,
  inductors, nonlinear devices, AC analysis, and adaptive stepping remain out of scope.

## 2. Persisted schematic format

The top-level document has `schemaVersion`, `components`, `wires`, `junctions`, and
`referenceMarkers`. Version 1 has no computed node IDs, currents, solver state, result
layout, or snapshots. Unknown schema versions are rejected or migrated by the project's
versioned migration path before compilation; they are never interpreted as version 1.

Object IDs (`component.id`, `wire.id`, `junction.id`, and `referenceMarker.id`) are
canonical lowercase UUID strings, unique across the document, and stable for the object's
lifetime. Moving or rotating an object does not change its ID. Duplicating an object creates
a new ID. IDs must not be derived from array positions. Pin IDs are stable catalog strings
scoped to a component type, not UUIDs; Stage 1 two-terminal devices use `pin1` and `pin2`.

Component `position` is its symbol origin on the logical grid. Coordinates are signed
32-bit integer grid units; one unit has no physical dimension and is independent of zoom
or screen pixels. `rotationDeg` is one of 0, 90, 180, or 270 degrees clockwise in the
screen coordinate system (x right, y down). Wire points and reference-marker positions
use the same world grid. Save only drawn geometry and explicit parameters; omit catalog
defaults from a saved component if the file-format implementation represents them as
implicit values. The resolved parameter value passed to Engine must always be explicit.

Version 1 component types and parameter domains:

| Type ID | Pin IDs and meaning | Parameters (SI) | Catalog defaults |
|---|---|---|---|
| `resistor` | `pin1`, `pin2` | `resistanceOhms`: finite and strictly greater than zero | `1000 Ω` |
| `dc_voltage_source` | `pin1` is positive; `pin2` is negative | `voltageVolts`: any finite signed value; `internalResistanceOhms`: finite and greater than or equal to zero | `5 V`, `0.1 Ω` |

Catalog metadata is authoritative for parameter names, units, defaults, and allowed
domains. NaN, positive/negative infinity, null, unknown parameters, and out-of-domain
values are rejected. An explicitly configured zero source resistance means an ideal
voltage source and is valid. It is represented using the MNA ideal-source constraint;
inconsistent ideal constraints and singular/underdetermined systems return structured
diagnostics, never a successful NaN result. No source default silently changes to zero.

The source's `voltageVolts` is its constant internal EMF. For current positive into
`pin1`, terminal voltage is `EMF + internalResistance × current`; a discharging source
therefore has negative pin1 current and terminal voltage below its EMF.

### 2.1 Geometry and connectivity

- Pin-local coordinates come from the frontend symbol definition. Rotate them clockwise
  around the component origin, then translate by `position` to obtain pin world positions.
  A compile request supplies exactly one world-grid position for each catalog pin.
  In screen coordinates, the local-vector transforms are `(u, v)`, `(-v, u)`,
  `(-u, -v)`, and `(v, -u)` for 0, 90, 180, and 270 degrees respectively. The resulting
  pin position must be an integer grid point.
- Wire paths contain at least two distinct grid points. Consecutive points must differ and
  each segment must be horizontal or vertical. Zero-length and diagonal segments are
  invalid. Collinear overlapping segments form one conductor.
- Conductors connect at coincident wire endpoints, at a wire endpoint touching another
  segment, and where a pin lies on a wire segment. These endpoint-on-segment and pin-on-
  segment contacts form T-junctions automatically.
- Two segment interiors crossing at a point do not connect unless an explicit junction
  marker is present there. A marker connects every conductor through that exact grid point.
  A continuous self-crossing polyline remains one conductor because it is one wire path.
- All decisions use exact integer grid coordinates. Do not use floating-point epsilon
  matching. Visual bends and route changes do not change connectivity except where the
  rules above create or remove a contact.
- A reference marker must lie exactly on a pin or conductor; a detached marker is an
  input diagnostic, not an implicit wire. Multiple explicit markers identify the same
  global reference and intentionally connect those marked nets.
- Without an explicit marker, Engine selects a deterministic numerical reference for
  each electrically connected island, using the lexicographically smallest stable
  `(componentId, pinId)` in that island. This is a solver reference only: it does not join
  disconnected islands. Emit `NoGroundReference` as a warning for each such island.
  Values for an island without an explicit marker are relative to that island's local
  reference; never compare its absolute node potential to another island's potential.
- If an island has no component pins, or its unknown voltages remain indeterminate after
  applying the reference and device equations, report `FloatingNode` or `SingularSystem`.
  Do not silently join it to another island or ground.

The compile-time geometry envelope is separate from the saved document and contains one
entry per component, with its complete pin set:

~~~json
{"componentId":"00000000-0000-4000-8000-000000000001","pins":[{"pinId":"pin1","position":{"x":-1,"y":0}},{"pinId":"pin2","position":{"x":1,"y":0}}]}
~~~

Reject duplicate component entries, duplicate pin IDs, missing catalog pins, unexpected
pins, and out-of-range/non-integer coordinates. Rust can validate the IDs and coordinate
shape, but not whether a coordinate matches the drawn SVG; the cross-layer contract check
owns that guarantee.

## 3. Sign conventions and initial state

For every two-terminal component:

- `voltage = V(pin1) - V(pin2)`;
- `current` is positive entering `pin1` and leaving `pin2`;
- `power = voltage × current`, following the passive sign convention: positive power is
  absorbed by the component and negative power is supplied.

For the voltage source, pin 1 is its positive terminal. The Stage 1 source is constant in
time. Compilation calculates the operating point at time zero (simulation step 0) before
fixed-step advancement. The physics timestep is fixed at `timeStepSeconds = 0.001` (1 ms);
simulation time is `simulationStep × 0.001 s`. Stage 1 devices are memoryless, so advancing
a step leaves the operating point unchanged unless an input parameter changes. There is no
separate public DC or AC mode. Updating a Stage 1 resistor or source parameter rebuilds its
stamps and recalculates the operating point; a rejected update leaves the previous valid
compiled state and result unchanged.

Physics time and UI delivery cadence are separate clocks. The application throttles UI
frames to at most 60 per wall-clock second (nominal interval 1/60 s, about 16.67 ms),
coalescing intermediate results to the latest snapshot. It must not alter the 1 ms solver
timestep to meet the display rate. A frame may therefore skip simulation steps. Any
simulation slow-down factor changes the mapping from model time to wall-clock time, not the
solver timestep or the UI frame cap.

## 4. Engine API semantics

The public Rust API is a library interface; these operations describe semantics and do not
freeze Rust method spelling or Tauri command names:

1. **Catalog:** return supported component types, stable pin/parameter IDs, units,
   defaults, domains, and parameter descriptions.
2. **Compile:** accept a versioned schematic plus resolved pin world positions and a
   document revision. Validate the whole input, derive topology, select references, build
   the MNA model, and return a compiled-run ID, result layout, and initial operating-point
   snapshot. If an error-severity diagnostic exists, compilation has no usable result.
3. **Set parameter:** accept the current run ID, the expected current document revision, a
   new document revision, component ID, parameter ID, and SI value. Compare revision tokens
   only by equality; the expected token must match the active snapshot, and the new token
   must differ from the active token. Validate atomically, update stamps, and return a new
   snapshot under the same layout if successful. Reject a stale run ID or revision. Invalid
   updates preserve the last valid state/result.
4. **Snapshot:** expose the latest completed state without requiring the caller to know
   solver internals. Engine owns numerical state; the application owns worker-thread
   lifecycle and message scheduling.

Diagnostics are structured data, not parser-specific strings. Each contains a stable code,
severity (`error`, `warning`, or `info`), a localization key, and typed references to
affected component, pin, wire, junction, or derived node IDs. User-facing prose is rendered
in the UI. Stage 1 uses these semantics:

| Code | Severity | Meaning |
|---|---|---|
| `InvalidSchemaVersion`, `InvalidGeometry`, `InvalidParameter`, `DuplicateId`, `UnknownComponentType`, `UnknownPin`, `DetachedReferenceMarker` | error | Input cannot be compiled as specified. |
| `NoGroundReference` | warning | A connected island has no explicit reference; a local numerical reference was selected. |
| `FloatingNode` | error | A node voltage remains unconstrained after reference selection and device equations. |
| `ShortCircuit` | error | An ideal zero-resistance source has both terminals on the same conductor; current is indeterminate, and nonzero EMF also conflicts with that conductor constraint. |
| `InconsistentIdealConstraint` | error | Ideal voltage-source equations conflict in a loop. |
| `SingularSystem` | error | The MNA system is rank-deficient or underdetermined and no more specific diagnosis is available. |
| `StaleRun` | error | An operation refers to a run other than the currently compiled run. |
| `StaleDocumentRevision` | error | The expected revision does not match the active snapshot, or the requested revision is unchanged. |

Warnings may accompany a valid result; any error prevents that result from being accepted.

## 5. Result layout and snapshots

Compilation returns a versioned layout. Each slot has a stable key, component ID, quantity,
unit, sign/terminal meaning, and zero-based offset. Stage 1 exposes component terminal
voltage and current for each supported two-terminal device. Component slots are ordered by
canonical component ID bytes, then by catalog quantity order (`voltage`, `current`). The
layout's `valueCount` is the number of slots; offsets are contiguous from zero to
`valueCount - 1`. Layout changes only on recompile; a numeric parameter update does not
change it.

Each frame has metadata outside the numeric payload:

- `runId`: a new opaque UUID for each successful compile;
- `documentRevision`: an opaque revision token changed for each schematic edit and echoed
  exactly; compare by equality, not numeric ordering. Parameter updates also carry the
  expected current token, which is checked against the active snapshot before applying the
  change;
- `layoutVersion`: the version of the accompanying layout;
- `simulationStep`: an unsigned 64-bit physics-step index, starting at zero;
- `simulationTimeSeconds`: finite model time, equal to `simulationStep × 0.001` seconds;
- `frameSequence`: a monotonically increasing unsigned 64-bit sequence of delivered UI
  frames within the run, starting at zero. It is distinct from `simulationStep`; frames may
  skip physics steps when UI delivery is throttled.

`layoutVersion` is an unsigned 32-bit integer starting at 1 for each run. It changes only
when the slot mapping changes. A new compile always creates a new `runId`, so its version
may start again at 1.

The payload is a contiguous array of IEEE-754 binary64 values, little-endian, in layout
offset order. It contains no header or padding. Its byte length must equal
`valueCount × 8`, checked for overflow; all values must be finite. Compile/layout/diagnostic
metadata is sent outside the numeric payload. The application accepts a frame only when
run ID, document revision, and layout version match the active document/run and its
sequence is newer than the last applied frame. Otherwise, discard it. Transport framing
and Tauri Channel API details are implementation choices for the bridge issue; no JSON is
sent for every physics step.

## 6. Canonical acceptance fixture

The fixture is a series loop. For the example symbol geometry, each two-terminal symbol
has local pin positions `pin1 = (-1, 0)` and `pin2 = (1, 0)`, with rotation zero.
The voltage-source negative terminal is the explicit reference. The saved project is:

~~~json
{
  "schemaVersion": 1,
  "components": [
    {"id":"00000000-0000-4000-8000-000000000001","typeId":"dc_voltage_source","position":{"x":0,"y":0},"rotationDeg":0,"parameters":{"voltageVolts":5,"internalResistanceOhms":0.1}},
    {"id":"00000000-0000-4000-8000-000000000002","typeId":"resistor","position":{"x":4,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":1000}},
    {"id":"00000000-0000-4000-8000-000000000003","typeId":"resistor","position":{"x":8,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":1000}}
  ],
  "wires": [
    {"id":"00000000-0000-4000-8000-000000000004","points":[{"x":-1,"y":0},{"x":-1,"y":4},{"x":3,"y":4},{"x":3,"y":0}]},
    {"id":"00000000-0000-4000-8000-000000000005","points":[{"x":5,"y":0},{"x":7,"y":0}]},
    {"id":"00000000-0000-4000-8000-000000000006","points":[{"x":9,"y":0},{"x":9,"y":-4},{"x":1,"y":-4},{"x":1,"y":0}]}
  ],
  "junctions": [],
  "referenceMarkers": [
    {"id":"00000000-0000-4000-8000-000000000007","position":{"x":1,"y":0}}
  ]
}
~~~

The frontend supplies the resolved world positions for both pins of each component at
compile time. These derived positions are not duplicated in the saved project. The
expected golden results are stored separately from the schematic:

- With `Vs = 5 V`, `Rs = 0.1 Ω`, and `R1 = R2 = 1000 Ω`, current is
  `5 / (0.1 + 1000 + 1000) A`; the voltage across R2 is approximately
  `2.499875006 V`.
- After changing R1 to `3000 Ω`, the voltage across R2 is approximately
  `1.249968751 V`.
- For this fixture, the accepted voltage error is at most `1 µV`; current error is at
  most `1 nA`.

## 7. Verification and change control

Review the fixture geometry against the pin transform and connectivity rules, calculate
the two expected values independently, and confirm that no computed topology or result
state appears in the saved file. The implementation issues add executable serialization,
topology, solver, and bridge checks. Changes to these public fields, ID meanings, pin
geometry exchange, diagnostics, API semantics, or frame layout require both workstreams'
review and an update to this contract. Changes that alter an architectural decision also
update `ARCHITECTURE.md` or add an ADR.
