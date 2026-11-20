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

The Engine validates `documentation` with the document and otherwise ignores it: it changes no
compile, run or result. Terminal attachment to a pin or wire is checked with the drawing in
issue #192.

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
