# Stage 12 project format (schema version 3)

Contract: [`docs/contracts/stage-12-cabinet-documentation.md`](../contracts/stage-12-cabinet-documentation.md)
sections 2 and 3. This file records what issue #189 implemented.

## Inputs

`Schematic` is v3. It gains one required field, `documentation` (`ProjectDocumentation`,
owned by Rust and generated for TypeScript): `designators`, `wireNumbers`, `terminals`,
`notes`, `titleBlock` and `sheets`. The designator and wire-number lists are arrays of
entries that name the stable ID they belong to. Every nested object denies unknown fields.

Projects of version 1 and 2 are decoded by their frozen shapes (`LegacySchematicV1`,
`LegacySchematicV2`) and normalized to v3 with empty documentation. A v2 file that holds a
`documentation` field is malformed, not accepted. Opening a legacy file does not rewrite it;
saving writes v3, which an older reader rejects.

The Engine validates `documentation` with the document and otherwise ignores it during circuit
solving. Wire numbering and its warnings use derived conductor connectivity on demand.

In the editor, the Terminal tool places a marker on a pin or wire. Select its label to edit the
strip and terminal name. At a pin, the terminal follows that pin. On a wire, it follows its
distance along the route, limited to the new end if the route becomes shorter. Moving or
deleting the attachment and updating the terminal share one undo entry.

Select a wire to edit its number, or use **Number the wires** to number fully unnumbered
conductors. The same number is assigned to each wire on that conductor. Saved numbers on
other conductors are kept. Undo reverses the whole numbering operation.

## Validation

- Limits are checked first and reject the whole document: 4,096 designators, wire numbers and
  terminals each, 200 notes, 32 sheets (`DocumentationLimitExceeded`).
- A designator or wire number for an unknown component or wire is
  `UnknownDocumentationTarget`; an ID listed twice is `DuplicateId`.
- Designators follow `[ABCEFGHKMPQRSTUVWX][A-HJ-NP-Z]{0,2}[1-9][0-9]{0,3}` and are unique
  (`InvalidDesignator`, `DuplicateDesignator`).
- Wire numbers and terminal labels use letters, digits and `+ . _ -`; a strip is a capital
  letter and up to seven capitals or digits; strip and label together are unique.
- Notes are 1 to 500 characters on at most 10 lines; title-block fields have the lengths of
  contract section 7.1; sheet names are 1 to 32 characters; sheets may not overlap
  (A4 59 by 42 grid units, A3 84 by 59).
- Terminal, note and sheet IDs share the document's one ID space.

## Assignments

Assignment files stay v1. An assignment whose schematic holds documentation is refused
(`UnsupportedAssignmentDocumentation`, or `TaskMismatch` for a live-shaped schematic inside a
file). `evaluate_checks` refuses a request carrying documentation.

## Catalog

Every catalog entry carries `designatorPrefix`, the letters of contract section 3.2.

## Fixtures

`tests/golden/stage-12-legacy-v2-*.ocircuit` are v2 files kept as they are, for the migration
tests. Every other live fixture was moved to v3 with empty documentation by
`scripts/documentation-defaults.mjs`.

## Bridge and persistence (issue #190)

- `open_project` takes `assignmentOpen` (it replaces `forbidRails`). With an assignment open, a
  project with DC rails or documentation beyond designators is refused after reading and before
  the project path changes; a project whose documentation is only designators opens with the
  documentation emptied, and the file is not rewritten. The message that designators are not
  kept comes with the designator interface (#191).
- Save writes the documentation as drawn and nothing derived; a save the Engine refuses leaves
  the file and the path as they were.
- `SimulationController.syncDocument` sends nothing when only `documentation` changed
  (`hasSameElectricalPart`: version and the component, wire, junction, marker and rail lists by
  identity): the run, traces and operator positions stay. Any other change recompiles as before.

## Designators (issue #191)

- Placing a part stores its designator in the same history entry: the catalog's
  `designatorPrefix` and the next number (the highest in use with exactly those letters plus
  one). Deleting the part removes the entry in the same entry; undo and redo restore both.
  While an assignment is open nothing is stored and the field is not offered.
- `SchematicDocumentStore.setDesignator` is one history entry and refuses, leaving the document
  unchanged, a text outside the grammar of contract section 3.3 or one another part has. The
  Designator field of the double-click editor and of the Inspector call it; the Engine's
  validation remains the authority.
- A part without a stored designator keeps its automatic name, which skips every stored
  designator, so no name is shown twice. `src/editor/designators.ts` mirrors the grammar and the
  numbering for the immediate check.
- Not in this change: the **Note** tool (see below) and the commands of the documentation panel (Assign
  missing designators); both follow in their own changes under #191.

## Notes (issue #191, second part)

- The **Note** tool of the toolbar places a note at the clicked grid point (its top-left
  corner) with the text "Note", selects it, and stays active until Escape. A note is
  selected, moved by dragging, edited by a double press or Enter (a non-modal panel with one
  text area: Enter adds a line, Ctrl+Enter or leaving the field stores, Escape restores, a
  refused text stays with its reason) and deleted with Delete or the panel's button. Each is
  one history entry (`addNote`, `moveNote`, `setNoteText`, `removeNote` of the document store).
- A note is drawn on top of the circuit and is picked only where no electrical object is, so it
  never shadows one. It takes no part in connectivity. `src/editor/noteRules.ts` mirrors the
  text rules (1 to 500 characters, at most ten lines, no control character but the line break,
  counted as the Engine counts) and the limit of 200 notes.
- While an assignment is open the Note tool is disabled and note commands are refused.
- Not in this change: the documentation panel's **Assign missing designators**.
