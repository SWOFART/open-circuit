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

Stage 3 adds the `capacitor` and `inductor` types and their optional initial-condition
parameters, the only parameters that may be absent; see
[`stage-3-dynamics.md`](stage-3-dynamics.md) section 2.

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
- `frameSequence`: a monotonically increasing unsigned 64-bit sequence within the run,
  starting at zero. The Engine takes the next number for every physics advance and every
  parameter update, whether or not a frame is then delivered to the UI, so the numbers the
  UI sees have gaps and the UI relies only on their increasing. It is distinct from
  `simulationStep`; frames may skip physics steps when UI delivery is throttled.

`layoutVersion` is an unsigned 32-bit integer starting at 1 for each run. It changes only
when the slot mapping changes. A new compile always creates a new `runId`, so its version
may start again at 1.

The payload is a contiguous array of IEEE-754 binary64 values, little-endian, in layout
offset order. It contains no header or padding. Its byte length must equal
`valueCount × 8`, checked for overflow; all values must be finite. Compile/layout/diagnostic
metadata is sent outside the numeric payload. The application accepts a frame only when
run ID, document revision, and layout version match the active document/run and its
sequence is newer than the last applied frame. Otherwise, discard it. No JSON is sent for
every physics step; section 8 specifies how frames are framed and delivered by the desktop
bridge.

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

### 6.1 Display rounding

The UI shows every voltage, current, and resistance with an SI prefix and **seven
significant digits**. For the fixture it shows `Vout = 2.499875 V` (the voltage across R2)
and, after R1 becomes 3000 Ω, `1.249969 V`. Both are within `1 µV` of the exact values,
`2.499875006 V` and `1.249968751 V`. Six digits would show `2.49988 V`, 5 µV away, which
is why seven are used. `src/ui/acceptance.test.ts` checks this against the golden values.

Rounding is for display only. The number sent to the Engine, the number decoded from a
frame, and the number a tolerance is checked against are never rounded. A value that
cannot be shown as a finite number is shown as `—`, never as `NaN` or `Infinity`.

This rule is proposed with the end-to-end acceptance (issue #10) and needs sign-off from
both workstreams in review.

## 7. Verification and change control

Review the fixture geometry against the pin transform and connectivity rules, calculate
the two expected values independently, and confirm that no computed topology or result
state appears in the saved file. The implementation issues add executable serialization,
topology, solver, and bridge checks. Changes to these public fields, ID meanings, pin
geometry exchange, diagnostics, API semantics, or frame layout require both workstreams'
review and an update to this contract. Changes that alter an architectural decision also
update `ARCHITECTURE.md` or add an ADR.

## 8. Desktop bridge transport

This section fixes the details that sections 4 and 5 left to the bridge: the commands, the
binary frame envelope, and how frames are delivered. It is implemented by `crates/app`
(Rust) and `src/store` (TypeScript). The decision to use a Tauri `Channel` for frames, and
the evidence for it, are recorded in `docs/adr/0001-binary-channel-frames.md`.

### 8.1 Commands

| Command | Arguments | Result |
|---|---|---|
| `get_component_catalog` | none | `ComponentCatalog` |
| `compile_circuit` | `request: CompileRequest` | `CompileResult`, as JSON. The layout, diagnostics, and initial snapshot are sent once, here. A successful compile starts a new run and ends any frame stream. A failed compile leaves the previous run and stream untouched. |
| `set_parameter` | `request: SetParameterRequest` | `SetParameterResult`, as JSON. |
| `start_simulation` | `runId`, `onFrame: Channel` | Starts binary frames for the active run. Refused with an error if `runId` is not the active run. |
| `stop_simulation` | `runId` | Ends the stream of that run and releases its channel. Ignored if that run is no longer streaming, so a late stop cannot cut off a newer run's stream. |
| `acknowledge_frame` | `frameSequence` | The UI has processed every frame up to and including that sequence. |
| `read_traces` | `request: TraceRequest`, `onTrace: Channel` | Sends one bounded `OCT1` binary response for the active run. |
| `acknowledge_trace` | `runId`, `traceSequence` | The UI has processed the trace response identified by this run and sequence. |

Engine diagnostics are part of the result values. A failure of the transport itself, such as
a stopped simulation thread, is returned as an error string. The TypeScript adapter reports
it as an unavailable engine, never as a circuit error.

The engine serializes `u64` counters (`simulationStep`, `frameSequence`) as JSON numbers in
the JSON results. The TypeScript adapter converts them to `bigint` after checking that each
is a non-negative safe integer, and refuses anything else. Binary frames carry full 64-bit
integers.

### 8.2 Frame envelope

A frame is one binary channel message, arriving in the webview as an `ArrayBuffer`. All
integers and floats are little-endian.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | Magic `OCF1`: the frame format, version 1 |
| 4 | 4 | `layoutVersion`, `u32` |
| 8 | 8 | `frameSequence`, `u64` |
| 16 | 8 | `simulationStep`, `u64` |
| 24 | 8 | `simulationTimeSeconds`, `f64` |
| 32 | 16 | `runId`, the UUID as 16 raw bytes |
| 48 | 4 | `valueCount`, `u32` |
| 52 | 4 | `revisionLength`, `u32`, in UTF-8 bytes |
| 56 | n | `documentRevision`, UTF-8 |
| 56 + n | p | Zero padding up to the next multiple of 8 |
| 56 + n + p | 8 × `valueCount` | The numeric payload: `f64` values in layout offset order |

The numeric payload keeps the shape section 5 requires: contiguous binary64 values with no
header or padding of their own. The metadata that section 5 puts outside the payload is the
envelope before it. The padding makes the payload start on an 8-byte boundary, so a
`Float64Array` can view it in place. The total length is exactly `payloadOffset + 8 ×
valueCount`.

A reader refuses a frame when: it is shorter than 56 bytes; the magic is wrong; the length
differs from the one the header describes; the padding is not zero; the revision is not valid
UTF-8 or is longer than 256 bytes (a leading byte order mark is kept as part of the
revision); `valueCount` exceeds 65 536; or the simulation time or any
value is not finite. Both readers check the limits before using the lengths in arithmetic.
The Rust encoder and decoder (`crates/app/src/frame.rs`) and the TypeScript decoder
(`src/store/frameCodec.ts`) are tested against the same golden frame,
`tests/golden/stage-1-frame.json`, whose bytes were produced by an independent encoder
written from this section.

A change to this envelope is a contract change. An incompatible change gets a new magic.

### 8.3 Trace response envelope

`read_traces` sends one message on `onTrace`, arriving in the webview as an
`ArrayBuffer`. The response combines a bounded JSON metadata header with a little-endian
numeric payload:

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | Magic `OCT1`: the trace format, version 1 |
| 4 | 4 | `headerLength`, `u32`, the UTF-8 JSON header length |
| 8 | n | JSON header: `traceSequence`, `metadata`, selected `signals`, and `pointCount` |
| 8 + n | p | Zero padding up to the next multiple of 8 |
| 8 + n + p | variable | `pointCount` records, each `simulationStep: u64` followed by one `minimum: f64` and `maximum: f64` pair per signal, in `signals` order |

All integers and floats are little-endian. Each signal descriptor carries its layout
`key` and SI `unit`. The metadata carries `runId`, `documentRevision`, `layoutVersion`,
`sampleIntervalSeconds`, `historySamples`, `oldestSampleStep`, `latestSampleStep`,
`firstSampleStep`, `lastSampleStep`, `sampleCount` (raw samples represented before decimation), `missedSamples`, and
`replaced`. The header is at most 8192 bytes. It is followed by at most 2048 points for
at most 8 signals; therefore a response is at most 512 KiB. A reader checks the magic,
header length, padding, dimensions, total length, and finite payload values before using
any decoded lengths. The JSON `u64` fields `traceSequence`, `oldestSampleStep`,
`latestSampleStep`, `firstSampleStep`, `lastSampleStep`, and `missedSamples` are base-10
strings (nullable for the optional steps), preserving full precision in JavaScript.
`firstSampleStep` and `lastSampleStep` are the inclusive raw sample range represented
before decimation; clients use `lastSampleStep` as the cursor for a later `newSamples`
request. They are `null` for an empty response, whose `documentRevision` is the newest
stored sample's revision. Point `simulationStep` values are binary `u64` integers.

`TraceRequest` selects 1–8 unique layout keys and a `maxPoints` value. The application
caps `maxPoints` at 2048 (the Engine also refuses values above 4096). A `history` query
uses an inclusive `[startStep, endStep]` range. A `newSamples` query supplies
`afterStep` and `afterDocumentRevision`: normally it returns samples after that step; if
that step was replaced by a live parameter update and its revision differs, it returns
that step too and sets `replaced`. The JSON step fields are non-negative safe integers in
JavaScript. If history has overwritten requested steps,
`missedSamples` reports the number omitted before the first returned sample. A reader
must use each point's `simulationStep`; it must not join across a reported gap as if no
samples were lost. When decimating, a point's step is the first step in its bucket, and
its per-signal minimum and maximum preserve peaks.

### 8.4 Trace delivery and flow control

- Trace samples are captured by the Engine at every public 1 ms step, independent of the
  60 Hz frame delivery cap. `read_traces` reads the Engine's bounded ring history; it does
  not start or advance the simulation.
- The app admits at most one pending `read_traces` request per active run and rejects a
  second request before it enters the simulation-thread queue. Only one trace response per
  run may be unacknowledged. The UI acknowledges it with the matching `runId` and
  `traceSequence`; an acknowledgement for another response is ignored. After one second,
  an unacknowledged response no longer blocks a new request. The bridge does not queue
  trace responses.
- A successful compile discards the previous trace buffer and any pending response. A
  failed compile preserves both. A stopped run stops advancing but its retained history
  stays readable. A live parameter update replaces the sample at the current step.
- `read_traces` refuses stale run IDs and layout versions. Trace data and commands stay
  outside the pure Engine's transport model; the Engine returns typed batches and `crates/app`
  encodes this envelope.

### 8.5 Frame delivery

- The simulation thread owns the Engine and advances the fixed 1 ms physics step against
  the wall clock. Its pace does not depend on the UI. After a long stall, the backlog is
  dropped rather than simulated in one burst.
- At most one frame is delivered per frame interval (1/60 s). Only the latest snapshot is
  delivered; a lagging UI skips frames instead of queueing them.
- The UI acknowledges frames. With two frames unacknowledged, nothing more is sent, so a
  slow webview cannot make the backend queue frames without bound. A frame that stays
  unacknowledged for one second no longer counts, so a lost acknowledgement cannot stall
  delivery. A webview that stops acknowledging altogether therefore still receives about
  two small frames per second, never more than two at a time, and nothing accumulates.
- `compile_circuit` and `set_parameter` refuse a `documentRevision` longer than 256 bytes
  with an `InvalidParameter` diagnostic, so a revision that no frame could carry never
  ends a stream silently.
- A frame is sent only when its `frameSequence` is newer than the last frame sent. A
  successful `set_parameter` therefore reaches the stream as a frame carrying the new
  revision and values.
- Frames and command replies travel separately and are not ordered relative to each other.
  A frame produced right after an update can reach the UI before that update's own reply.
  The UI applies the section 5 acceptance rule to every frame, and treats a reply that a
  newer frame of the same revision has already superseded as confirmed.

### 8.6 Lifecycle

- `start_simulation` opens a stream for the active run and replaces any earlier stream.
- A successful `compile_circuit`, `stop_simulation` for the streaming run, a closed
  receiver, a run that can no longer advance, and the end of the application each end the
  stream. Ending a stream drops its channel, which releases the subscription in the
  webview.
- Destroying the window ends the stream, and exiting the application stops the simulation
  thread and waits for it.
