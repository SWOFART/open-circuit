# Stage 16 Contract: Editor Improvements

**Issue:** [#224](https://github.com/SWOFART/open-circuit/issues/224). **Parent:** [#161](https://github.com/SWOFART/open-circuit/issues/161). **Status:** proposal for the approval of both workstreams and the project owner. Implementation (#225 to #232) follows acceptance of this contract.

**Scope:** Multi-selection, group move and delete, copy, cut, paste and duplicate, a half-turn and a mirror transform, canvas search, a history panel, light and dark themes, autosave with crash recovery, and a shortcut window. It changes the editor, the document store, one optional field of the saved format, and one new native service. It does **not** change the simulation: no part, model, analysis or Engine result changes, and the fixed 1 ms step and every solver rule stay as they are.

## 1. Boundary and compatibility

| Earlier text | What changes |
|---|---|
| stage-2-editor.md and `hitTest.ts`: "the one selected schematic object; multi-selection is deferred" | Section 2 replaces the single selection with an ordered set of selected objects. A one-element set behaves exactly as the single selection did. |
| stage-2-editor.md section 3.4, editing shortcuts (`R`, `Delete`, `Escape`, `Enter`, undo, redo) | Kept. Section 10 lists them with the new ones in one table, and the table becomes the only place bindings are defined. |
| stage-11-drawer-and-editing.md section 3.6, preferences in the web view's local storage | Reused for the theme (section 8). No new storage kind for preferences. |
| stage-12-cabinet-documentation.md: the saved project is schema 3, strict | One **optional, additive** field, `mirrored` on a component (section 5.2). **No earlier stage added a field to the schematic** (stage 13 added component types and parameters only; its additive fields are in the assignment file, a different file), so this is the first, and keeping the schema version at 3 is a decision for the owner (section 13, decision 1). An older reader refuses a file that uses the field through its existing unknown-field refusal; there is no downgrade. A file without the field means what it meant before. |
| stage-5-assignments.md and stage-8-supply-rails.md: assignment restrictions and no rails in assignments | Respected by every operation of this stage (sections 3, 4 and 5.3). Nothing here lets a student do what the assignment forbids. |
| ARCHITECTURE.md: "store drawn inputs only" | Selection, the clipboard, the history labels, the theme and the recovery file are never part of a project file, an assignment file, the Engine's input or a result. The recovery file holds the drawn document and nothing derived. |

Nothing here is validated by the Engine except what the Engine already validates: a pasted or restored document goes through the same strict schematic decoding as an opened file.

## 2. Selection model

- A **selection** is an ordered set of `{ kind, id }` pairs, kinds `component`, `wire`, `junction`, `referenceMarker`, `rail` and `note` (the kinds of `SelectableKind` today), with no duplicates. The last member added is the **primary** one. The set is editor state: it is never saved, never part of the history, and is cleared by opening, creating or restoring a document.
- **Pruning.** After any change of the document (an edit, an undo, a redo, a live update from the history panel) members whose object no longer exists are removed, and the primary is the last remaining member. A selection never refers to an absent object.
- **Pointer.** A click on an object replaces the selection with it, as today. `Shift` + click adds the object, or removes it when it is already a member. A click on empty canvas clears the selection. `Shift` keeps its other meaning while a wire is being routed (the bend direction); in select mode it means "toggle membership", and the two never overlap because routing is its own mode.
- **Box select.** A drag that starts on empty canvas draws a rectangle. On release it **replaces** the selection with every object **entirely inside** the rectangle (a component's pin and body bounds, every point of a wire or a rail, a marker's point, a note's box); with `Shift` held at the release it **adds** them. A rectangle smaller than the click tolerance is a click on empty canvas. Objects of the kinds the active assignment does not let the student touch are still selectable (they can be copied and moved), and the operations of section 3 check the restrictions.
- **Keyboard.** `Ctrl/Cmd+A` selects every object, `Escape` does **exactly one** of these per press, in this order: it closes an open dialog or the search field; otherwise it cancels an uncommitted gesture (a route, a drag, a box, a placement preview), as stage 2 section 3.4 has it; otherwise it clears the selection. Arrow keys move the selection by one grid unit (`Shift` + arrow by ten), as one history entry per key press; auto-repeat events are ignored, so holding a key moves once (a held key must not flood the history), and the owner confirms that choice (section 13, decision 7).
- **Inspector and panels.** With exactly one member the inspector and the device panels show that object as today. With several, the inspector shows a count by kind ("3 parts, 2 wires") and the operations that apply to all of them; it never edits a parameter of several parts at once (deferred, section 12).
- **Hit testing and priority** keep the order of stage 8 section 7.1 (point markers, components, rails, wires, notes) for clicks. A box does not use priority: it takes everything inside.
- **The canvas** draws every member with the selection highlight; the primary member also keeps its editing handles (a rail's resize handles) and a lone selected wire its bend handles. Handles of non-primary members are hidden.

## 3. Group move and group delete

- **Move.** Dragging any selected object, or pressing an arrow key, moves **the whole selection** by an integer number of grid units `(dx, dy)`. Components, junctions, reference markers, rails and notes move by `(dx, dy)`. A **selected wire** translates with its whole path. A wire that is **not** selected but ends on a pin of a moved component, or on a moved junction, or on a moved rail, keeps its other end and has its last leg re-routed orthogonally, which is what moving one part does today (`transform.ts` wire following), now applied to every moved object at once. A wire that has both ends on moved objects and is not selected is re-routed the same way, not translated: only a selected wire is translated, so a selection never changes a wire it does not contain.
- **Locked parts** (stage 5 section 3) may be moved: a lock forbids deleting and editing the part, not placing it. A rail cannot be moved in an assignment (no rails there).
- **Delete.** `Delete` or `Backspace` removes every selected object that the assignment lets the student delete, and **reports** the ones it kept: if any selected component is locked, the command removes nothing and says which parts are locked (it is all or nothing, so one press never leaves a half-deleted selection). Deleting a component keeps today's rule for the wires on its pins; the whole delete is one history entry.
- **One history entry.** A move, a delete, a paste, a cut, a duplicate, a rotation or a mirror of a selection is **one transaction**: one undo restores the document exactly, and one redo repeats it. A move whose `(dx, dy)` is `(0, 0)` records nothing.
- **Collisions.** A move or a paste may put pins on pins; as everywhere in the editor, coincident pins are connected, and the Engine's diagnostics report what that does. The editor does not stop it (it shows a plain message when a move connects pins that were not connected before: "This move joins 2 pins").

### 3.1 Groups

A **group** in this stage is the multi-selection acted on as one: it moves, rotates, mirrors, is copied and deleted as one transaction. **Groups are not saved**: there is no named group in the project file. A saved, named group needs a format change (a list of member references in the documentation section, a validation rule for members that are deleted, and a migration note) and is **deferred** to a later stage unless the owner decides otherwise (section 13, decision 2).

## 4. Copy, cut, paste and duplicate

### 4.1 What is copied

`Ctrl/Cmd+C` copies the selection; `Ctrl/Cmd+X` copies it and then deletes it as section 3 does (one history entry, refused whole if a member is locked). The copy holds:

- every selected component, junction, reference marker, rail and note with its drawn fields (type, position, `rotationDeg`, `mirrored`, parameters, a rail's name and extent, a note's text and position);
- every **selected wire**, and every **unselected wire whose two ends are both on copied objects** (so a copied part with the wire between two copied parts keeps its connection);
- the documentation that belongs to the copied objects and can be repeated: a note's text. **Designators, wire numbers and terminal entries are not copied** (a designator is unique in a document, and a copy would break that); a pasted part is named by the automatic numbering.

Nothing derived is copied: no result, no state, no diagnostic.

### 4.2 The clipboard format

The text of the system clipboard is one JSON object, so that it also works between two windows and as plain text:

```json
{ "format": "open-circuit-clipboard", "version": 1, "schematic": { "components": [], "wires": [], "junctions": [], "referenceMarkers": [], "rails": [], "notes": [] } }
```

The members have the shapes of the saved schematic of the same names (schema 3, with the optional `mirrored`), with stable IDs as they were in the source. `version` is an integer; an unknown version is refused with a message. The payload is **untrusted input**: the clipboard can hold anything another program put there. A paste validates it with the strict schematic decoding (unknown fields, types, ranges, the limits of the project file) and refuses the **whole** paste on the first problem; it never repairs, truncates or executes anything. A payload larger than the project limit (16 MiB) is refused before it is parsed; the interface can only measure the text after the clipboard has been read, so the check is on the length of that string, and nothing is built from a longer one.

### 4.3 ID rules

Every pasted object gets a **fresh stable ID** from the document store's factory, and every reference inside the payload (a wire to a junction, a documentation entry to an object) is rewritten to the new ID. A pasted object never keeps a source ID, even when pasted into a different document, so a document never holds a duplicate ID and a later undo of the paste removes exactly the pasted objects. A paste that cannot assign IDs is refused whole.

### 4.4 Placement and the preview

- `Ctrl/Cmd+V` starts a **placement**: the pasted objects are drawn as a translucent preview, with the top-left of their bounds following the pointer (snapped to the grid), and **a click** commits them. `Enter` commits at the preview's current place, `Escape` cancels it (the precedence of section 2), and nothing is recorded until it commits. The first preview is offset by `(+2, +2)` grid units from the bounds of the source selection when that selection is still in the document, and is centered in the view otherwise.
- **Duplicate** (`Ctrl/Cmd+D`) is a copy and an immediate paste at `(+2, +2)` from the selection, with no placement mode and no clipboard change. A second duplicate offsets from the new selection, so repeated presses step along a diagonal.
- A commit is **one history entry**, and the pasted objects become the new selection.
- A paste that would put a pin on an existing pin follows the collision rule of section 3 (it is allowed, and says so).

### 4.5 Assignment restrictions

In an assignment (stage 5 section 3): a pasted component of a type outside `allowedComponentTypes` refuses the whole paste ("This assignment does not allow <type>"); a rail in the payload refuses the whole paste (no rails there); a locked part can be copied but a cut that would delete it is refused whole; pasted parts are never locked and so belong to the student. A fault-finding assignment keeps its rule that the faulted parts stay: the copy of one is allowed, the cut is not.

## 5. Rotate 180 degrees and mirror

### 5.1 Turns of a selection

`R` is a quarter turn clockwise and `Shift+R` a half turn, of the **selection** as a whole, as one history entry. No saved field is added.

- **The pivot.** One selected component turns about its own origin. A selection of several objects turns about the center of the bounds of its members, with each coordinate rounded toward positive infinity to a grid point, so every result is on the grid.
- **Every member** is carried by the same map, a point `(x, y)` about the pivot `(px, py)`: a half turn takes it to `(2px - x, 2py - y)`, a quarter turn clockwise to `(px - (y - py), py + (x - px))`. A component moves its origin by it and adds the angle to `rotationDeg` (modulo 360); a junction, a reference marker and a note move their point (the text box of a note keeps its orientation, only its anchor moves); a **selected wire** has every point of its path carried by the map; a wire that is not selected but ends on a moved object keeps its other end and is re-routed orthogonally, as in section 3.
- **Rails** are vertical (stage 8). A half turn maps a rail onto a vertical rail and is allowed; a quarter turn would make it horizontal, so a selection that contains a rail is **refused whole** for a quarter turn, with the message "A rail cannot be turned a quarter".
- A turn that changes nothing (an empty selection) records nothing.

### 5.2 Mirror

A mirror flips a component about the **vertical axis of its symbol, before the rotation is applied**: a local point `(x, y)` of the symbol becomes `(-x, y)`, and then the component's rotation is applied as now. The result is a `mirrored` boolean on the component:

- `mirrored` is **optional**, absent or `false` meaning "not mirrored", and is **omitted when false**, so an unmirrored project file is byte-identical to what it was.
- It is present on every component type. The Engine accepts it as a boolean and does nothing else with it, because the Engine takes the pin positions from the geometry the editor sends with every compile; a mirrored part reaches the Engine as the pin positions it really has.
- A mirror of the selection uses the pivot of section 5.1 and the map `(x, y)` to `(2px - x, y)` for every member (so one part is mirrored about its own vertical axis and several about the vertical axis through the center of the selection); a component also toggles `mirrored`, a note moves its anchor only, a rail stays vertical and moves with the map, and wires follow as in section 3, all as one history entry. Mirroring twice is the identity.
- The pin positions of a mirrored, rotated part are `rotate(mirror(local))` for every symbol, including the ones whose pins are symmetric, so a drawn part and its recompiled geometry always agree. Every symbol of the registry must pass a test that a mirrored pin is at the mirrored place and that mirror then mirror returns the original.
- A project that uses `mirrored` is read by this version and by later ones; an older version refuses it through its unknown-field refusal, like any file with a newer feature.

### 5.3 Assignments

Rotating and mirroring are moves of a part's drawing, not edits of its parameters, so they are allowed for locked parts (as a move is). A mirrored part in an assignment's starting circuit is part of the circuit and is kept.

## 6. History panel

- The document store keeps a **label** with each history entry: a message key and its values (for example `history.move` with a count, `history.paste`, `history.deleteSelection`, `history.setParameter` with the part and the parameter). A transaction without a label shows the generic "Edit". Labels are editor state and are never saved.
- The panel lists the entries **newest first**, with the redo entries after the current point shown dimmed and marked as undone. Selecting an entry **undoes or redoes, one ordinary step at a time, until the document is in the state just after that entry**; each step is an ordinary undo or redo, so the redo stack and the dirty flag behave as they always do, and the panel adds nothing a keyboard user could not do with repeated `Ctrl+Z`. Choosing the entry that is already current does nothing.
- The panel has no entry for opening, creating or restoring a document (those start a fresh history). The history is bounded to **1000 entries**, the oldest dropped first; this bound is new (it was unbounded), and the panel says when older entries were dropped. Whether the document is dirty never depends on the stack (it compares the document with the saved one), so dropping the entry of a save point leaves a document that is still clean when it equals the saved one and dirty otherwise.
- A transaction that the editor refuses leaves no entry.

## 7. Canvas search

- `Ctrl/Cmd+F` opens a search field. The query matches, case-insensitively and as a substring, **display names and designators** (`R1`, a stored designator), the **catalog names** of part types, a **rail's name** and the **text of a note**. A result lists its kind, its name and, for a part, its type.
- `Enter` or a click on a result **selects** it (replacing the selection) and **centers the view on it** without changing the zoom unless the object is outside the view; `Up` and `Down` move through the results. `Escape` closes the field and keeps the selection. An empty query shows nothing; no match says "Nothing found".
- Search reads the document and the labels; it never changes the document and records nothing in the history. In an assignment it finds the parts and the notes the student can see.

## 8. Themes

- Three modes: **System** (the default; follows `prefers-color-scheme` and changes with it), **Light** and **Dark**. The choice is a **preference**, stored as stage 11 stores the drawer's, in the web view's local storage under one key, never in a project, an assignment, the Engine or the history. A stored value that is unreadable or unknown falls back to System, and unavailable storage means the choice lives for the session.
- The colors of the interface **and of the canvas** come from one set of named tokens (CSS custom properties for the interface; the same values handed to the canvas renderer). No component hard-codes a color. The tokens cover surfaces, text, borders, the selection highlight, wires, symbols, the grid, results, warnings, errors and the preview of section 4.
- **Contrast.** In both themes: text against its surface is at least **4.5:1**, and wires, symbols, borders and focus outlines against the canvas surface at least **3:1** (the WCAG 2.2 AA rules for text and for non-text contrast). A test computes these ratios from the tokens, so a changed token that breaks one fails the build. A state shown by color (lit, picked, tripped, hazard) is also shown by a word or a shape, so no meaning rests on color alone.
- Switching a theme redraws; it never recompiles or restarts a run. Printing and the cabinet drawing exports keep their own fixed light colors (stage 12) in both themes.

## 9. Autosave and crash recovery

Native I/O is owned by `crates/app`; the interface asks for the operations below and shows the answers.

- **Where.** One recovery file in the application's local data directory (the operating system's per-user, per-application location), under `recovery/`. It is **never the project file** and never next to it. The user's project file is only written by an explicit save.
- **What.** The drawn document exactly as a project file holds it (schema 3), and a small envelope: the format name and version, the time it was written, the path of the project it came from (or none), and the application version. No result, no state, no preference. A recovery file is at most the project limit (16 MiB): the writer refuses a larger one and the reader reuses the project reader's bounded read (`MAX_PROJECT_BYTES` in `crates/app`), refusing a larger file unread.
- **When.** While the document is dirty, after **2 seconds without an edit** and then at most **every 30 seconds**; never while a save or an open is running. A write goes to a temporary file in the same directory and is **renamed over** the previous recovery file, so a crash during a write leaves either the old file or the new one, never half of one.
- **When it is removed.** After a successful explicit save of the document, when the user creates a new document or opens another one and chooses to discard the changes, and when the user chooses **Discard** at a recovery offer. At every start, recovery files older than **30 days** are removed.
- **At start.** If a recovery file is present, the application, before anything else, shows an offer: the time it was written, the project it came from and the choices **Restore** and **Discard** (`Escape` is not a choice; the dialog waits). **Restore** opens the recovered document as an **unsaved** document with **no path**, so a plain save asks for a place and can never overwrite the original project by surprise, and keeps the recovery file until the first explicit save or an explicit discard. **Discard** deletes it. A file that cannot be read or decoded is offered for deletion with the reason, and the application starts clean.
- **Failure.** A write that fails (a full disk, a permission, a missing directory that cannot be created) never interrupts editing: the interface shows one non-blocking notice ("Autosave is not working: <reason>"), keeps retrying on the next interval and clears the notice when a write succeeds. Autosave never deletes the previous good recovery file before a new one is written.
- **Privacy.** The recovery file holds the user's drawing and so is user data, not telemetry: it is never sent anywhere, and the application does not log its content.
- **Assignments.** Autosave is **off** while an assignment is open in this stage (a restored circuit would need the assignment file to mean anything, and an assignment's own rules govern how its work is saved); recovery of assignment work is deferred (section 12).

## 10. Shortcut map and the shortcut window

- **One registry.** Every binding is an entry of one table in the editor code: an identifier, the keys per platform (`Ctrl` on Windows and Linux, `Cmd` on macOS), a message key for its description, a group, and the scope in which it applies (canvas, anywhere, a dialog). The key handlers look a binding up in this table; none matches a key on its own. The window is **generated** from the same table, so a binding that works is listed and a listed binding works. A test fails when a handler is not in the table, when an entry has no description, and when two entries in one scope share keys.
- **The window.** Opened with `F1` or `?` (outside a text field) and from a visible **Keyboard shortcuts** button in the toolbar's help group (the application has no native menu bar). It is a dialog with a focus trap, labelled, closed by `Escape` and by its close button, returning focus to what opened it, readable in both themes, and listing the bindings by group with the keys of the running platform.
- **The map.** Existing bindings keep their keys.

| Group | Binding | Keys |
|---|---|---|
| File | New, Open, Save, Save As | `Ctrl+N`, `Ctrl+O`, `Ctrl+S`, `Ctrl+Shift+S` |
| Edit | Undo, Redo | `Ctrl+Z`, `Ctrl+Shift+Z` |
| Edit | Cut, Copy, Paste, Duplicate | `Ctrl+X`, `Ctrl+C`, `Ctrl+V`, `Ctrl+D` |
| Edit | Select all | `Ctrl+A` |
| Edit | Delete the selection | `Delete`, `Backspace` |
| Edit | Close a dialog, cancel a gesture, clear the selection (one per press, in that order) | `Escape` |
| Edit | Open the editor of the selected part; finish a route; commit a placement; open the chosen search result (by scope) | `Enter` |
| Find | Move through the search results | `Up`, `Down` |
| Edit | Move the selection by 1, by 10 units | arrows, `Shift` + arrows |
| Transform | Rotate a quarter turn, a half turn | `R`, `Shift+R` |
| Transform | Mirror | `M` |
| Find | Search the canvas | `Ctrl+F` |
| Help | Keyboard shortcuts | `F1`, `?` |
| Wiring | Hold to route the vertical leg first | `Shift` (while routing) |

Keys are matched by physical key code where a letter is used, so they work on other keyboard layouts (as today). No binding takes a key while a text field has the focus, except the ones the field does not use (`Escape` closes a dialog, `F1` opens help).

## 11. Accessibility

Every new control is reachable and operable by keyboard; the history panel, the search results and the shortcut window have roles, names and a visible focus; a selection change is announced in a live region ("3 objects selected"); and the recovery dialog and the shortcut window trap focus and restore it. Interactive targets are at least 24 CSS pixels (WCAG 2.2). The tests check names and roles on the rendered markup, as the existing interface tests do.

## 12. Deferred

Saved, named groups (section 3.1); editing a parameter of several selected parts at once; copying designators, wire numbers and terminals with a paste; a cross-application paste of drawings from other tools; a vertical mirror (a mirror then a half turn gives it); non-orthogonal rotations; a history that survives a restart; recovery of assignment work; several recovery files for several documents; a configurable shortcut map; high-contrast and user-defined themes; and anything that changes the simulation.

## 13. Decisions that need approval

1. **A saved `mirrored` field** on components (section 5.2), additive and optional, with the schema version **left at 3**. This is the first new field of the schematic itself (stage 13's additive fields are in the assignment file), so the choice is the owner's: the alternative is a schema 4 with a migration, which the stage 12 migration showed to be heavier; the other is to drop the mirror. The cost of the field is one optional boolean on every component variant of the Engine's schematic and the generated types.
2. **Groups are not saved** (section 3.1): a group is the multi-selection acted on as one. A saved, named group is deferred.
3. **Box select takes objects entirely inside** the rectangle, not objects it touches (section 2).
4. **Autosave timing and place** (section 9): 2 s idle then at most every 30 s, one recovery file in the application's local data directory, **Restore opens an unsaved document with no path**, a 30-day clean-up, and autosave off in assignments.
5. **The history bound** of 1000 entries (section 6), new for a store that had none.
6. **Theme storage** in the web view's local storage as the drawer's preferences are (section 8), and the 4.5:1 and 3:1 contrast floors.
7. **The key map** of section 10 (and that holding an arrow key moves once), including `M` for mirror, `Shift+R` for the half turn, `Ctrl+D` for duplicate and `F1` or `?` for the shortcut window.
8. **The clipboard payload** of section 4.2 (a JSON text with a version) and that designators, wire numbers and terminals are not copied.

## 14. Verification and ownership

- **Contract** (#224): both workstreams review this document; the decisions of section 13 are answered or recorded as deferred; ARCHITECTURE.md records the format field with this contract.
- **Application** (#225 to #231), one issue per feature, in the order selection, clipboard, transforms, search and history, themes, autosave, shortcuts. Each adds tests at the level it works: the document store (transactions, IDs, restrictions, labels), the editor core (selection, box, hit testing, preview, geometry of every symbol under rotation and mirror), the interface markup (roles, names, the tokens' contrast) and, for autosave, `crates/app` (atomic write, failure cases, cleanup, the offer at start).
- **Engine** (#227 only): the optional `mirrored` field on the component variants and its generated types, with a test that a file with and without it round-trips and that the geometry the editor sends decides the pins.
- **Acceptance** (#232): `docs/acceptance/stage-16-editor-improvements.md` with the commands, native desktop observations including keyboard use, candidate CI on three platforms and the earlier workflows intact.
