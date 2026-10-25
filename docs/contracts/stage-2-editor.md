# Stage 2 Editor and Project File Contract

**Status:** Draft for review by the Application and shared/Engine workstreams.
**Scope:** Interactive editing and persistence for the Stage 1 schematic and
component catalog. This document extends, but does not replace,
[ARCHITECTURE.md](../../ARCHITECTURE.md) or [stage-1.md](stage-1.md).

## 1. Boundaries and compatibility

- The document being edited is the Stage 1 **Schematic**: components, wires,
  explicit junctions, reference markers, and their logical-grid geometry.
- Stage 2 supports only the catalog types already present in Stage 1: resistors
  and constant DC voltage sources. New physical models and simulation modes are
  outside this contract.
- The Rust Engine owns component defaults, parameter domains, electrical
  connectivity, diagnostics, and all computed results. The UI owns editor
  gestures, symbol rendering, and document editing.
- EditorCore remains plain TypeScript/PixiJS and does not import React.
  React panels communicate document changes through the document store.
- Preserve the Stage 1 schema, stable-ID rules, SI units, integer grid,
  rotations, wire geometry, and connectivity semantics. A necessary schema
  change requires an explicit version change, migration, generated-type update,
  tests, and review by both workstreams.
- A saved project contains no computed nodes, connectivity graph, solver state,
  diagnostics, simulation readings, result layout, or frames.

## 2. Entry points and document lifecycle

### 2.1 Initial, example, and new documents

- When the app starts without an explicitly opened project, it opens the
  canonical Stage 1 voltage-divider example, preserving current behavior.
- The example is also available through an explicit **Open Example** action.
  Loading it creates an editable in-memory copy; it never edits the golden
  fixture file.
- **New** creates an empty version 1 schematic with empty components, wires,
  junctions, and referenceMarkers arrays.
- A new or example document has no project path. **Save** on a document without
  a path invokes **Save As**.
- Opening the example, creating a new document, opening a file, or closing the
  window while the document is dirty prompts with **Save**, **Discard**, and
  **Cancel**. Cancel leaves the current document active.
- If the user chooses Save but cancels the file dialog or saving fails, the
  pending New, Open, Open Example, or close action is canceled and the dirty
  document remains active.
- Last-window session restoration and multiple open documents are out of scope
  for Stage 2.

### 2.2 Saved document

The native project file uses the .ocircuit extension and contains one
UTF-8 JSON Schematic object, with no project wrapper:

~~~json
{
  "schemaVersion": 1,
  "components": [],
  "wires": [],
  "junctions": [],
  "referenceMarkers": []
}
~~~

- Version 1 uses exactly the top-level fields and object forms defined by the
  Stage 1 contract. All component parameter values are serialized explicitly;
  placement defaults are read from the Rust catalog and then become document
  values.
- JSON whitespace and object-key order have no semantic meaning. Saving should
  use readable, deterministic formatting so project files can be reviewed in
  version control.
- The save path is application state, not part of the Schematic and not
  serialized into the project file.
- Selection, active tool, viewport pan/zoom, undo/redo history, component
  display names (section 3.2), and temporary gesture previews are editor state
  and are not persisted.

## 3. Editing model

### 3.1 Coordinates and view

- All committed component positions, wire points, junctions, and reference
  markers use signed integer logical-grid coordinates as defined in Stage 1.
  Zoom and display pixels never change saved coordinates.
- Component rotations are 0, 90, 180, or 270 degrees clockwise in screen
  coordinates. Pin locations continue to be derived from the symbol catalog
  geometry and the Stage 1 transform.
- The canvas starts in **Select** mode. The user can pan with middle-button
  drag or Space plus left-button drag. The wheel zooms around the pointer.
  A **Fit to circuit** command frames all visible schematic geometry.
- The view transform maps logical-grid coordinates to canvas pixels as
  `screen = grid × scale + offset`, with one scale for both axes. A pointer
  position maps back through the inverse transform and snaps to the nearest
  integer grid point on each axis; exact halves round toward positive
  infinity. Every move, placement, and wire gesture uses these snapped points.
- Wheel zoom keeps the grid position under the pointer fixed and stays within
  a fixed scale range. The view fits the circuit on startup and follows
  window resizes until the user pans or zooms; **Fit to circuit** restores
  that behavior.
- Selection, pan, zoom, and preview state do not mark the document dirty.

### 3.2 Selection and component tools

- Clicking a component, wire, junction, or reference marker selects that one
  object. Clicking empty canvas in Select mode clears selection. Multi-selection
  and group transforms are deferred.
- Hit testing uses a pick radius fixed in screen pixels. A component is hit
  anywhere within its symbol's extent or its label. Where objects overlap,
  junctions and reference markers take precedence over components, and
  components over wires, matching the drawing order. Selection is held by
  stable ID, so it survives redraws and edits, and it clears when its object
  is removed.
- The palette is populated from the Rust component catalog. Selecting a type
  shows a snapped placement preview; clicking places the component using
  catalog defaults. The placement tool stays active for repeated placement.
- R rotates the placement preview or selected component clockwise by 90
  degrees.
- Moving or rotating a component preserves its stable ID. A drag commits one
  snapped move on pointer release. Wire endpoints that exactly coincide with
  old pin positions follow the new pin positions with orthogonal rubber-band
  routing: the moved endpoint bends once to rejoin the rest of the path. Of
  the two possible bends, the one that runs through the component's body or
  over its other pins fewer times is chosen; on a tie the endpoint leaves in
  the direction of the wire's old end segment. Around the new bend, repeated
  points and points inside a straight run are removed; the rest of the wire
  keeps its drawn path. The component and its following wires change in one
  history transaction. A wire that would collapse to a single
  point is left unchanged. A pin that only touches the interior of a wire
  segment has no stored attachment; moving or rotating that component leaves
  the wire fixed and may disconnect the pin.
- Deleting a selected component removes only that component. Wires and markers
  remain as drawn, even if they become dangling or detached; the user may edit
  or remove them separately.
- Components are shown under display names such as `R1` and `V1`. A name
  belongs to the component's stable ID and does not change while the document
  stays open: components present when the document is opened or created are
  numbered per type in file order, a component placed later takes the next
  number of its type, and numbers are never reused. Deleting, placing,
  moving, rotating, editing, undoing, or redoing therefore never renames
  another component, and undoing a deletion restores the name too. Names are
  editor state: they are not saved, and reopening a file numbers its
  components in file order again. Results and diagnostics use the same names,
  including for a component that was deleted after the result was computed.
- The existing Inspector commits a parameter edit as one document operation.
  It reads parameter names, units, defaults, and domains from the Rust catalog.
  The Engine remains authoritative for validation and recalculation.

### 3.3 Wire, junction, and reference tools

- The Wire tool starts at a snapped grid point, pin, or conductor. Each click
  adds a waypoint. Consecutive waypoints are connected by horizontal/vertical
  segments; if both coordinates differ, the editor inserts an orthogonal bend.
  The default bend is horizontal-first; holding Shift chooses vertical-first.
- Clicking a pin or existing conductor ends the route at that exact point.
  Enter finishes at the snapped pointer, a free point. A double-click
  finishes where it lands: clicking the last waypoint a second time ends the
  route there. A route that never leaves its start draws nothing. Escape
  cancels the in-progress route without changing the document, and so does
  Undo or switching tools. The preview shows the pending orthogonal path
  including the leg to the pointer. Each bend is fixed by the Shift state at
  the click that created it; a later straight continuation merges into one
  segment. The Wire tool stays active for the next wire until Escape returns
  to Select mode.
- A committed wire has at least two distinct points, no repeated adjacent
  points, and no diagonal segments. Selecting a wire shows a handle at each
  vertex, ends included. Dragging a handle moves that vertex by the snapped
  drag offset and keeps the neighboring segments orthogonal: each neighbor
  keeps the direction of its old segment and gets one bend if it no longer
  lines up, and repeated or straight-through points are then removed. A drag
  commits once on release as one history transaction; an operation that
  cannot produce a valid path is not committed, and Escape cancels the drag.
- Endpoint, endpoint-on-segment, and pin-on-segment contacts form T-junctions
  automatically. A visual dot at such a connection is derived and is not saved
  as an explicit junction.
- Segment-interior crossings remain disconnected unless the user adds an
  explicit junction marker at that exact grid point. One continuous
  self-crossing wire remains connected. A marker that no longer sits at a
  crossing has no electrical effect and is shown as detached.
- The Junction tool is available from the editor toolbar. Hovering an
  unconnected segment-interior crossing previews a connection dot; clicking
  commits a junction marker there with a fresh stable ID. The tool only commits
  where at least two distinct conductors cross and does not already connect.
  Clicking elsewhere leaves the document unchanged and indicates that a
  crossing is required; the indication lasts until the pointer moves to
  another grid point. While the tool is active, every valid spot is marked
  with a ring. Spots are defined in section 3.3.1.
- An unconnected crossing is drawn without a node dot. An explicit junction is
  drawn as a filled connection dot. The same filled dot is used as a derived
  visual indicator for automatically connected T-junctions; only explicit
  junction markers are stored in the document.
- A selected junction marker can be dragged to another crossing and committed
  there as one document operation. Releasing away from a valid crossing cancels
  the move. Deleting a selected explicit junction removes only that marker and
  then recalculates the local connection state. If no reference marker or
  automatic endpoint/pin contact remains at that point, the crossing is shown
  without a node dot and is disconnected there. If another such connection
  remains, it stays connected and the filled node dot remains; a reference
  marker also retains its reference glyph and global-reference behavior.
- A reference marker is a global reference for every conductor and pin exactly
  at its position. Placing one on an otherwise unconnected crossing therefore
  intentionally joins the conductors at that reference. This explicit action
  differs from an unmarked segment-interior crossing.
- The Reference tool places a marker only on a pin or conductor. Reference
  markers remain point-based objects; moving their former conductor does not
  silently move them. Multiple reference markers intentionally identify the
  same global reference. If an edit detaches a marker, keep it in the document
  and surface the Engine diagnostic so the user can move or delete it.
- A selected reference marker can be dragged to another pin or conductor and
  committed there as one document operation. Releasing away from a valid
  target cancels the move.
- A wire may have a free endpoint while being edited. Electrically incomplete
  but structurally valid documents can be saved and reopened; the Engine
  reports circuit diagnostics without preventing editing.
- Removing a wire deletes that wire path only. Removing an explicit junction
  can disconnect a crossing. Removing a reference marker removes only that
  marker.
- Automatic A* routing is deferred.

### 3.3.1 Editor geometry hints

The editor needs a few facts about the drawing to guide the user. They are
hints only: the Engine alone decides what is connected and reports the
diagnostics (stage-1.md section 2.1), and `tests/golden/stage-2-connectivity.json`
pins these hints and the Engine's verdicts to the same drawings. The editor
evaluates them on exact integer grid points.

- **Attachment.** A point is an attachment if a pin is there or if it lies on a
  wire segment, the segment's ends included. The Wire tool ends a route on an
  attachment, and the Reference tool and a dragged reference marker accept
  only attachments. At most one reference marker sits at a point.
- **Junction spot.** A point is a junction spot if a horizontal segment of one
  wire path and a vertical segment of a different wire path cross there with
  the point strictly inside both segments, and no segment end of any wire, no
  pin, no junction marker, and no reference marker is at the point. Any of
  those already connects the conductors there, and a path crossing itself is
  one conductor already.
- **Connection dot.** A dot is drawn at a point with at least three arms and no
  explicit junction. A segment ending at the point is one arm, a segment passing
  through is two, and a pin is one. Two arms are a corner or a lead-in and get
  no dot. A bare crossing has no dot because nothing is there to connect it.
- **Detached marker.** A junction or reference marker with no pin and no wire
  segment at its point is shown in the warning color. The Engine rejects such a
  marker (`InvalidGeometry` for a junction, `DetachedReferenceMarker` for a
  reference marker), so the circuit is not solved until the marker is moved or
  deleted; the document stays editable and the diagnostic is shown with the
  kind of marker it concerns.

### 3.4 Keyboard commands

Shortcuts are handled only while the app window is active and never intercept
text-entry fields. Canvas tools do not act when a text-entry field has focus:

| Command | Windows/Linux | macOS |
|---|---|---|
| New | Ctrl+N | Command+N |
| Open | Ctrl+O | Command+O |
| Save | Ctrl+S | Command+S |
| Save As | Ctrl+Shift+S | Command+Shift+S |
| Undo | Ctrl+Z | Command+Z |
| Redo | Ctrl+Shift+Z | Command+Shift+Z |
| Delete selection | Delete or Backspace | Delete or Backspace |
| Rotate selected component or preview | R | R |
| Finish the wire being drawn at the pointer | Enter | Enter |
| Bend new route legs vertical-first | Hold Shift | Hold Shift |
| Cancel current gesture/tool | Escape | Escape |

If Escape is pressed during a wire route or drag preview, it cancels that
uncommitted gesture. Otherwise it returns a placement, wire, junction, or
reference tool to Select mode.
Undo during an uncommitted gesture cancels that gesture first; otherwise it
undoes the latest committed document operation.

## 4. Document history and dirty state

- A completed user action is one history transaction: component placement,
  deletion, move, rotation, committed parameter edit, completed wire path,
  wire edit, junction edit, or reference-marker edit.
- Pointer-move preview frames and each keystroke in an uncommitted parameter
  field do not create history entries. A drag commits once on release; Escape
  cancels it.
- Undo and redo change only the schematic document. They trigger normal Engine
  recompilation/update and do not restore old snapshots or runtime frames.
- Undo/redo history is cleared when a new document, example, or file is loaded.
  A new edit after undo discards the redo branch.
- Dirty state means the current serialized schematic differs from the last
  successfully opened or saved schematic. Returning to that exact document
  through undo clears the dirty state. Selection and viewport changes do not
  affect it.
- The document remains editable when Engine compilation fails. Diagnostics are
  displayed; an invalid/incomplete circuit is not replaced by a mock result.
- A result is shown as current only for a document the Engine has accepted. If
  a lightweight parameter update is rejected, fails, or cannot be confirmed,
  the Engine does not have that edit, so the queued updates behind it are
  dropped and the whole current document is compiled at once, without waiting
  for another edit. The outcome of that compile is what the user sees: a
  fresh result, or the Engine's diagnostics with no result, or the transport
  error. A later update never produces a result that leaves out a refused
  edit.

## 5. Open, save, validation, and versioning

- Native file dialogs and filesystem access belong to crates/app. The UI
  passes document values through typed Tauri commands; it does not access
  arbitrary filesystem paths directly.
- Schema validation and version migration are pure Engine responsibilities,
  independent of Tauri and file I/O. The Engine may validate a document without
  solving its topology so that electrically incomplete circuits remain
  editable.
- Stage 2 reads and writes schema version 1. A missing, malformed, or
  unsupported schema version is rejected with a clear error; it is never
  interpreted as version 1. Any future migration must be an explicit,
  deterministic, sequential, tested transformation to the current version.
- Reject malformed JSON, unknown fields, duplicate or invalid stable IDs,
  invalid rotations, out-of-domain/non-finite parameters, and invalid wire
  geometry before replacing the active document. Reuse the Engine catalog and
  Stage 1 validation rules rather than maintaining a second set in UI code.
- A structurally valid document with floating, disconnected, dangling, or
  otherwise electrically invalid topology may still open. Display Engine
  diagnostics while retaining the document for editing.
- If loading or parsing fails, leave the active document, path, history, and
  dirty state unchanged.
- Save writes the current versioned schematic only. A successful save updates
  the path and saved-document baseline and clears dirty state. A failed save
  leaves the document dirty and preserves the previous saved file.
- Saving should replace a file atomically where the platform supports it, so an
  interrupted write does not truncate the previous valid project.

## 6. Verification

- Unit tests cover coordinate transforms, snapping, route geometry, document
  transactions, undo/redo, dirty-state restoration, and serialization.
- Integration tests prove that a saved and reopened schematic validates and
  compiles through the real Rust Engine. Stage 1 golden electrical values and
  connectivity rules remain unchanged.
- Manual acceptance covers New, Open Example, component placement and editing,
  wire authoring, undo/redo, Save As, reopen, and the unsaved-change prompt.

## 7. Non-goals

Multi-selection, copy/paste, autosave/session restoration, multiple project
tabs, cloud/collaborative editing, new component models, transient C/L
simulation, SPICE export, and smart A* routing are not part of Stage 2.
