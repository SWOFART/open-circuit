# Stage 12 Contract: Cabinet Documentation

**Status:** Draft for review by the Engine and Application workstreams and for approval by the project owner (issue #188).
**Scope:** What turns a drawn circuit into a document that can be handed over: part designators (`Q1`, `K2`, `F1`), terminals, wire numbers, text notes, a bill of materials, a title block, several sheets, and export to SVG, PNG and PDF. It fixes the project format change these need, the rules for names, and what is deferred. This document amends earlier contracts only where it says so.

Documentation is drawn input, like a wire or a rail. None of it is a model, an equation or a result: it never changes what the Engine computes, and the interface calculates nothing in it. The Engine owns the data format and its validation; the interface draws and edits.

## 1. Sources, boundaries and compatibility

### 1.1 The owner's pictures

The project owner supplied photographs of pages of a German electrical-engineering handbook: the table of identification letters for objects (the classification of IEC 61346-2 and DIN EN 61346-2, called "Kennbuchstaben"), the symbol tables of DIN EN 60617 (general symbols, additional symbols and switches in power installations), and examples of an installation circuit diagram drawn all-pole and single-pole with a supply point. They are **design input**. They are books under copyright and the photographs are partly blurred, so they are **not committed** to the repository (section 11). This contract records only the facts it needs from them, in its own words: which identification letter belongs to which kind of part (section 3.1), the rule that parts of one letter are numbered consecutively (`C1`, `C2`), and that the letter follows the purpose of a part, not its type (a transistor that processes a signal is `K1`, one that switches a load is `Q1`).

### 1.2 What Stage 12 adds

- **Designators** for components, saved in the project, editable, with rules for form and duplicates (section 3).
- **Terminals** and **wire numbers**, which are labels on the drawing and never change the circuit (sections 4 and 5).
- **Text notes** (section 6).
- A **title block** and **sheets** (section 7), a **bill of materials** (section 8), and **export** (section 9).
- A new schema version of the project (section 2) and one new catalog field (section 3.2).

### 1.3 Earlier text this contract amends

| Earlier text | Amendment |
|---|---|
| ARCHITECTURE.md, the notes on editable part names ("the names are not saved") | Names are saved from this stage on, as designators (section 3). The notes are updated in the same change as the implementation. |
| `src/store/labels.ts` and its per-type prefixes | The prefixes move into the Rust catalog (section 3.2). The interface keeps no table of its own. The numbering rule of that file ("a number is not reused") is replaced by section 3.4. |
| stage-11-drawer-and-editing.md section 1.3, "Renaming a part (Stage 12 ...)" | Done here: the double-click editor gets a Designator field (section 3.5). |
| docs/format/stage-8-projects.md, schema v2 | v2 stays readable; v3 is the new version (section 2). |
| stage-5-assignments.md | Assignments refer to parts by stable ID and never by designator, as before. Documentation is not part of an assignment (section 2.4). |

### 1.4 Out of scope

Mechanical cabinet layout and wire lengths; cable schedules; ERP, CAD and netlist exchange formats; automatic routing; off-sheet connectors and cross-reference tables between sheets (for example "contact of Q1 on sheet 3"); terminal-strip diagrams; the single-pole overview drawing of an all-pole circuit; zone grids on the frame; CSV export of the bill of materials; documentation inside assignments. Section 10 lists them as the deferred work.

## 2. The project format (schema version 3)

### 2.1 Shape

`Schematic` becomes schema version **3** and gains one required field, `documentation`, a `ProjectDocumentation` object owned by Rust and generated for TypeScript like every shared type (`src/generated/**` is never edited by hand):

```json
"documentation": {
  "designators": { "<componentId>": "Q1" },
  "wireNumbers": { "<wireId>": "12" },
  "terminals":   [ { "id": "...", "strip": "X1", "label": "3", "position": { "x": 4, "y": 2 } } ],
  "notes":       [ { "id": "...", "position": { "x": 0, "y": 0 }, "text": "Main supply" } ],
  "titleBlock":  { "title": "", "drawingNumber": "", "revision": "", "author": "", "company": "", "date": "" },
  "sheets":      [ { "id": "...", "name": "Power", "size": "a3", "origin": { "x": 0, "y": 0 } } ]
}
```

Nothing derived is saved: no net, no bill of materials, no sheet number, no result. The maps are keyed by the stable IDs that already exist; an entry whose ID no longer exists in the schematic is a validation error (the interface removes entries together with the object they belong to, in the same history entry).

### 2.2 Compatibility

- Versions 1 and 2 are decoded by their frozen shapes and normalized to version 3 with empty documentation, the way version 1 is normalized to version 2 today (docs/format/stage-8-projects.md). Opening does not mark the document modified. Saving always writes version 3, which an older reader rejects instead of dropping a field.
- Unsupported or malformed versions are rejected, and the duplicate-field checks of the strict decoders stay.
- A compile request carries the schematic as it does today. The Engine **ignores** `documentation`: no documentation field changes a compile, a run or any result.

### 2.3 Documentation edits do not restart a run

A change that only touches `documentation` is a **documentation edit**. The interface compares the electrical part of the document (everything except `documentation`) and sends the Engine nothing when only documentation changed: the run, its traces and its operator positions stay. Every documentation edit is one history entry and marks the document modified.

### 2.4 Assignments

While an assignment is open the document's documentation is **always empty** and documentation features are off, as DC rails are (stage-8-supply-rails.md). An assignment file stays version 1 and embeds a schematic without documentation. Names of parts in an assignment stay the automatic names of today and are not saved: **placing a part in assignment mode stores no designator**, and the documentation panel, the Designator field and the Note tool are unavailable. Opening a project while an assignment is open: if its `documentation` holds only designators, it opens and the designators are dropped on load (the document is normalized to empty documentation, with the message that they are not kept); if it holds anything else (wire numbers, terminals, notes, sheets or a non-empty title block), opening is refused with a message, before the project path changes, by the same mechanism as the `forbidRails` argument. A later contract may add documentation to assignments.

### 2.5 Limits

At most 4,096 designators, wire numbers and terminals each in total across a project; at most 200 notes; at most 32 sheets. Inputs above a limit are rejected whole.

## 3. Designators

### 3.1 Identification letters

The first letter of a designator is the identification letter of the kind of part, taken from the owner's table. In the words of this contract: **A** objects with several equal purposes; **B** converting a quantity into a signal (sensors, measuring transducers, motor-protection relays); **C** storing energy or information (capacitors); **E** radiating light or heat (incandescent lamps, heaters); **F** direct protection (fuses, circuit-breakers for lines, RCDs, thermal overload releases, surge arresters); **G** generating a flow or signal (generators, batteries, supply units); **H** producing a material or product; **K** processing signals (auxiliary contactors, relays, time relays, signal-processing transistors, controllers); **M** mechanical drive energy (motors); **P** presenting information (measuring instruments, signal lamps, LEDs); **Q** controlled switching of energy flow (power switches, power contactors, motor starters, switching transistors, thyristors); **R** limiting a flow (resistors, chokes, diodes, zener diodes); **S** converting a manual action into a signal (control switches, push buttons); **T** converting energy or signals while keeping the kind (transformers, rectifiers, amplifiers); **U** holding objects in position; **V** processing materials; **W** conducting energy or signals (conductors, cables, busbars); **X** connecting objects (terminals, plugs, sockets). The letters **D, J, L, N, Y, Z** are reserved by the standard for later use and **I** and **O** are not used because they are confused with 1 and 0; the table says so, and designators do not use them as a first letter.

### 3.2 Default letter of each part

The Rust catalog carries the default letter: `ComponentCatalogEntry.designatorPrefix: string` (one to three capital letters, the form of section 3.3). It is a catalog field like `category` in Stage 11, so the interface keeps no table, a new part cannot be added without one (a test checks it), and the golden catalog and generated types change in the same change.

| Letter | Parts (catalog type IDs) |
|---|---|
| `G` | `dc_voltage_source`, `ac_voltage_source`, `three_phase_source_star`, `three_phase_source_delta` |
| `R` | `resistor`, `inductor`, `diode`, `three_phase_load_star`, `three_phase_load_delta` |
| `C` | `capacitor` |
| `F` | `fuse`, `thermal_overload_three_pole` |
| `K` | `relay_spdt` |
| `Q` | `contactor`, `contactor_three_pole`, `breaker_three_pole`, `bjt_npn`, `bjt_pnp` |
| `S` | `switch_spst`, `push_button_no`, `push_button_nc` |
| `M` | `pmdc_motor`, `induction_motor_three_phase` |
| `T` | `transformer_three_phase` |
| `P` | `voltmeter`, `ammeter`, `ac_voltmeter`, `ac_ammeter`, `wattmeter` |

Where this differs from the letters the editor uses today (`V` for sources, `D` for diodes, `L` for inductors, `VM`, `AM`, `PV`, `PA`, `PW` for meters, `QF` for the breaker, `FR` for the overload relay, `Z` for loads), the table of the owner is followed; the differences are proposals for the owner to confirm (section 11). A user can always give a part another letter that the grammar allows (a signal transistor as `K1`, section 3.3). Parts added by later stages bring their own letter with them (lamps `E` or `P`, solenoids `Y` is reserved so `K` or `Q`, and so on), decided by their contract.

### 3.3 Form and rules

- A designator is **one to three capital letters followed by a number from 1 to 9999**: `[ABCEFGHKMPQRSTUVWX][A-HJ-NP-Z]{0,2}[1-9][0-9]{0,3}`. The first letter is an identification letter (section 3.1); further letters, at most two, are free and excluding `I` and `O` (so `QF1` is valid and `QF` has two letters, a subclass chosen by the user). Seven characters at most. No other character, no spaces, no lower case.
- **Unique** across the project: two components never share a designator. Comparison is exact. A duplicate is refused by the Engine's validation, and the interface shows the refusal where the user typed it.
- Rails keep their own names (stage-8-supply-rails.md); wires carry numbers (section 5), terminals carry strip and label (section 4). Designators belong to components only.
- A designator is a label of the drawing. It never selects an Engine model, a parameter or a diagnostic; diagnostics and results keep referring to the stable ID, and the interface shows the designator for that ID.

### 3.4 Numbering

The next number is an editing convenience of the interface, like the suggested date of the title block: the Engine does not compute it and validates the stored result (form and uniqueness) as for any edit.

When a component is placed, the same history entry stores its designator: the part's default letter and the **next number**, which is the highest number in use with that letter plus one (1 when none is). Gaps are allowed and a number freed by a deletion may be taken again by a later placement. Undo removes the designator with the component and redo restores it unchanged. Copies and groups (Stage 16) take their own new designators.

### 3.5 Editing

- The double-click editor of Stage 11 gets a **Designator** text field above the parameters, and the Inspector shows the designator as the part's name. Both use one command, `setDesignator(componentId, text)`, one history entry, the interface's immediate check of the form of section 3.3 (a mirror, as the rail name rules are mirrored) and the Engine's validation as the authority.
- A designator edit is a documentation edit (section 2.3): it does not restart a run.
- Enter commits, Escape restores, a refused value stays in the field with its reason, as every field does.
- A designator may be **missing in a file**: version 3 does not require every component to have one (section 3.6 explains why), and the Engine accepts a file where some or all are missing. Clearing the field in the editor is nevertheless refused, because the interface has no command that removes a designator; a missing one is only ever the state of a part that was never named.

### 3.6 Projects from before this stage

A version 1 or 2 project has no designators. Opening it shows the automatic names of today, which are not saved. The documentation panel has a command **Assign missing designators** that gives every component without one its default letter and next number in file order, in one history entry. Bill of materials and export (sections 8 and 9) are offered only when every component has a stored designator, and say so otherwise. Placing a new part in such a project stores its designator at once; the other parts keep their automatic names until the command runs, and an automatic name skips any name equal to a stored designator (compared as the full name), so no name is shown twice.

## 4. Terminals

A **terminal** is a labelled marker on the drawing, `strip` + `label`, for example strip `X1`, label `3`, drawn as `X1:3`.

- It is placed on a pin or on a wire, with the same attachment rule as a reference marker (stage-1.md and stage-2-editor.md): its position must be a pin or a point of a wire, and one terminal per point. It is **not an electrical element**: it adds no resistance, no node and no diagnostic, and the Engine's connectivity ignores it. A terminal is a connection point of the drawing only.
- `strip`: `[A-Z][A-Z0-9]{0,7}` (advice, not enforced: the standard uses `X` for terminals); `label`: `[A-Za-z0-9+._-]{1,8}`. The pair is unique in the project; a duplicate is refused.
- Moving the pin or wire it sits on moves it; deleting the pin's component or the wire deletes it, in the same history entry. Terminals belong to no sheet explicitly: a terminal is on the sheet that contains its position (section 7.3).
- A terminal shows in the bill of materials only through the strip designators (section 8.2); it is not a catalog part in this stage.

## 5. Wire numbers

A **wire number** is a short label on a wire, `[A-Za-z0-9+._-]{1,16}`, stored in `wireNumbers` by wire ID, at most one per wire.

- Several wires of the same net may carry the same number (a conductor drawn in several segments). Numbers belong to the drawing, never to the Engine's nets: no net identity is saved.
- The Engine checks the documentation and returns **warnings**, not errors, for: one number on two different nets (`WireNumberOnDifferentNets`, with the wires it names); a net whose wires carry two different numbers (`NetHasTwoWireNumbers`); and a wire number on a wire that has no electrical connection at all. Warnings are listed by the documentation panel and never stop a run, an export or a save. The checks need the Engine's connectivity, so they are an Engine function over the schematic (`documentation_diagnostics`), reached through the bridge.
- **Number the wires** is an Engine function (`number_wires`) that gives every net whose wires have no number the next free decimal number, from 1 upward, skipping numbers in use. Nets are taken in a fixed order: by the smallest point of the net, by `y` and then `x`. Wires that already have a number are not changed. The result is one history entry, and the same drawing numbers the same way every time.
- Wire numbers are drawn beside the wire's longest segment, in the same way on the canvas and in the export.

## 6. Text notes

A **note** is free text on the drawing: an ID, a position on the grid and a text.

- The text is UTF-8, 1 to 500 characters, with line breaks allowed (at most ten lines) and no other control character. Any script is allowed, so Russian and German text work.
- A note is a documentation object: it does not take part in connectivity, in hit tests of electrical objects beyond selecting and moving itself, or in the bill of materials. It belongs to the sheet that contains its position.
- A note is placed with a new **Note** tool of the editor toolbar, edited by double-click like a part (the same non-modal panel, one text area), moved by dragging and deleted with Delete, each one history entry.

## 7. Title block and sheets

### 7.1 Title block

One title block per project: `title` (100), `drawingNumber` (32), `revision` (16), `author` (64), `company` (64), `date` (32) characters, all text the user types, UTF-8, no control characters. The date is text: the interface offers today's date as a suggestion and never fills it by itself, and nothing here reads a clock into the file. It is shown in the frame of every sheet together with the automatic sheet number "n of N".

### 7.2 Sheets

A **sheet** is a frame over the one drawing, not a separate drawing: `name` (1 to 32 characters), `size` (`a4` or `a3`, always landscape) and `origin`, the top-left corner in grid units. The scale is fixed: **one grid unit is 5 mm**. The frame is therefore 59 by 42 grid units for A4 and 84 by 59 for A3 (the page is 59.4 by 42 and 84 by 59.4, so the frame is centred on the page and the sliver left over, under half a grid unit, is a margin); the printable area is the frame inset by two units on every side, 55 by 38 for A4 and 80 by 55 for A3; the printable frame is inset by two units and carries the title block at the bottom right, 36 by 8 units. Sheets may not overlap (a validation error); they are numbered by their order in the list, and the order can be changed. There may be none.

### 7.3 Which objects are on a sheet

An object is on the sheet whose frame contains its origin (a component's position, a terminal's or a note's position). A wire is drawn on every sheet its path crosses, clipped at the printable frame; a wire that leaves a sheet simply ends at the edge (off-sheet connectors are deferred). The Engine reports two warnings: `ObjectOutsideSheets` for objects on no sheet when there is at least one sheet, and `ObjectCrossesSheetEdge` for a component whose symbol crosses the printable frame. They are advice, never errors.

### 7.4 No sheets defined

With no sheet, export and the bill of materials treat the whole drawing as one sheet: A4 if the drawing's bounds fit its printable area (55 by 38 grid units), otherwise A3 (80 by 55), otherwise export is refused with a message that says the drawing needs sheets.

## 8. Bill of materials

### 8.1 What it is

A **derived list**, never stored. It is computed by the Engine (`bill_of_materials`) from the schematic and the catalog, so a test can check it against the drawing without the interface.

### 8.2 Rows and order

- One row per distinct pair of component type and exact parameter values (all parameters, in the catalog's order; an unset optional parameter is part of the identity as "unset"). Columns: position number (1, 2, ...), quantity, designators, type (the catalog's display-name key, which the interface turns into text) and the parameters with their SI values and units.
- Designators in a row are listed in natural order (`R2` before `R10`). Rows are ordered by the identification letters in the order of section 3.1 and then by the first designator's number. The row order and the position numbers are therefore the same every time.
- Terminals are listed as one extra section: one row per strip with the count of terminals and their labels. Notes, wires and rails are not listed.
- The list is shown in a panel and is exported as an extra page after the sheets (section 9).

## 9. Export

### 9.1 Formats

- **SVG**: one file per sheet, UTF-8, the frame, the title block, the drawing, the labels and the notes. Text is real `<text>` and the document states a generic sans-serif family.
- **PNG**: one file per sheet at 300 dpi (A4 is 3508 by 2480 pixels, A3 is 4961 by 3508), on white.
- **PDF**: one file with one page per sheet, then the bill of materials page; vector content; page size A4 or A3 landscape as the sheet.

All three are produced from **one description of the page** (polylines, circles and text in millimetres) that the interface builds from the same symbol primitives as the canvas and the preview of the drawer, so the document equals the screen. They are black on white with fixed line widths, whatever the theme of the application. The files contain no user name, machine name or path; the date in them is the title block's text, and no clock is read.

### 9.2 Writing the files

The interface asks for the location with the native dialog; native code (`crates/app`) writes the files atomically, as it writes projects, and a refused or failed write leaves the document and any existing file as they were. The PDF writer is a small Rust module or a crate chosen by #193; **a new dependency needs the owner's approval before it is added** (section 11).

### 9.3 Fonts

Latin, German and Cyrillic text must all be legible in the three formats. SVG relies on the viewer's font. For PNG the application's own font renders the text. For PDF an embedded font is needed, which has a licence; the choice of font and its licence is left to #193 and the owner (section 11). Until it is made, PDF export may be limited and the record says so.

## 10. Deferred work

Off-sheet connectors and cross-references; contact mirrors under a coil; terminal-strip and cable diagrams; the single-pole overview drawing; zone grids on the frame; documentation in assignments; CSV export of the bill of materials; drawing templates and frames of other standards; a printing dialog. Each needs its own contract.

## 11. Decisions that need the owner's approval

1. **The designator letters** of the table in section 3.2, including the changes from today's letters (`V` to `G`, `D` and `L` to `R`, the meters to `P`, `QF` and `FR` to `Q` and `F`, loads to `R`), and the grammar of section 3.3 (one to three capital letters, first letter from the table, a number to 9999).
2. **The project format**: schema version 3 with `documentation` inside the schematic; the Engine ignoring it; documentation not available in assignments for now (sections 2.2 to 2.4).
3. **The sheet model**: frames over one drawing at 5 mm per grid unit, A4 and A3 landscape, no off-sheet connectors in this stage (section 7).
4. **Photographs of book pages are not committed** (copyright and quality). The owner keeps them; if a picture is to be a reference, the owner confirms that it may be published, or supplies one that is free to use, under `docs/symbols/` (stage-11-drawer-and-editing.md section 4.2).
5. **The PDF writer and the PDF font**: a new Rust dependency or a small own writer, and an embedded font with a licence that allows it (sections 9.2 and 9.3).
6. **The interface language and the symbol standard.** The pictures follow DIN EN 60617 and IEC 61346. The proposal is to keep the symbols as IEC 60617 drawings (as the existing ones) and to leave the interface in English until the owner decides the language; designator letters do not depend on the language, and every visible text of this stage is a message key.

## 12. Verification

- **Contract** (#188): both workstreams review this document; the open decisions of section 11 are answered or recorded as deferred.
- **Format** (#189): strict shapes and limits for every documentation field, the v1 and v2 migration, a v3 round trip, an unknown field rejected, an entry for an unknown ID rejected, duplicate designators, terminals, notes, sheets (overlap, size) and the limits, with golden files for each.
- **Bridge and persistence** (#190): documentation through the app bridge and the project file; a documentation edit does not restart a run and an electrical edit does; opening a documented project in assignment mode is refused before the path changes.
- **Designators and notes** (#191): the catalog letters (a part without one fails a test), numbering, `setDesignator` and its refusals, the editor field, undo and redo, the Note tool.
- **Terminals and wire numbers** (#192): attachment rules, the three warnings, `number_wires` determinism on the same drawing, and no change to any electrical result.
- **Bill of materials, title block and export** (#193): the bill of materials of drawn circuits against hand-counted answers, order and natural sorting, the page description against golden SVG, PNG size and PDF page count, non-Latin text, and a refused write leaving everything unchanged.
- **Sheets** (#194): overlap, which sheet an object is on, the two warnings, no sheet defined.
- **Acceptance** (#195): `docs/acceptance/stage-12-cabinet-documentation.md` with the commands, results, native desktop observations including an exported file opened in another program, the candidate's CI on three platforms, and the earlier workflows (assignments, rails, operating devices, drawer and editor) intact.
