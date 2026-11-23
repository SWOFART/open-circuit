# Stage 12 acceptance record

- **Recorded** 2026-11-22
- **Issue** [#195](https://github.com/SWOFART/open-circuit/issues/195)
- **Contract** [Stage 12 cabinet documentation](../contracts/stage-12-cabinet-documentation.md)
- **Application candidate** `fa4ec9ae64c064e0a14fff1a779444c4c1c2cdec`, tree `30ba6fe571cdc662031aaa4c4eca017e7ed990e4`. [Draft PR #290](https://github.com/SWOFART/open-circuit/pull/290)

The candidate passed automated checks and Tauri builds on Windows, Ubuntu, and macOS. Native desktop observations were collected on Windows. The approval and merge of the stacked draft pull requests remain with the project owner.

## Native Windows walkthrough

The test project used the saved circuit from
[`stage-1-divider.schematic.json`](../../tests/golden/stage-1-divider.schematic.json).
Its 5 V source and two 1 kΩ resistors were documented as `G1`, `R2`, and
`R10`. The first wire had number `12` and a terminal `X1:3` at `(-1, 2)`.
The note at `(10, 6)` and the title both read `Über Щит`. The drawing
number was `D-12`, revision `A`. A separate horizontal conductor from
`(49, 8)` to `(65, 8)` crossed the boundary between the sheets.

| Sheet   | Landscape size | Origin in grid units |
| ------- | -------------- | -------------------- |
| Power   | A4             | `(-5, -9)`           |
| Control | A3             | `(54, -9)`           |

1. Opened the project through the native file picker. Opened Drawing documentation and moved the default active sheet, **Power**, later. The selector changed from **1. Power** to **2. Power** while the canvas view stayed in place.
2. Used **Save As** to create a separate `.ocircuit` copy. Chose **New** to clear the editor and reopened the saved copy through the native picker. The editor showed `G1`, `R2`, `R10`, terminal `X1:3`, wire number `12`, and title `Über Щит`. The saved v3 document matched the input field for field after reversing the expected sheet array to **Control**, **Power**.
3. Exported the reopened copy from the native PDF save dialog. Poppler's `pdfinfo -f 1 -l 3` read three landscape pages in order: **Control** (A3), **Power** (A4), and the bill of materials (A4). Rendered all three with `pdftoppm -f 1 -l 3 -r 100 -png`. The Power page showed the terminal and wire number, the note and title in German and Cyrillic, and sheet number **2 of 2**. The parts list showed one DC source, two resistors grouped under `R2, R10`, and terminal strip `X1` with one terminal labelled `3`.
4. In an earlier Windows run on `9c7cbb4425862cd974d29ab4a69584c19562d139`, exported the same circuit with **Power** before **Control**. The native folder picker produced one numbered SVG and PNG per sheet. SVG dimensions read from the file were `297 × 210 mm` and `420 × 297 mm`. PNG headers showed `3508 × 2480` and `4961 × 3508` pixels, each with a `pHYs` value of `11811` pixels per metre in both axes (300 dpi). The final follow-up commit changes active-sheet selection during reorder and adds its regression test. It leaves the format writers unchanged.

## Automated verification

The workspace test run used the first #290 candidate. The final UI follow-up
passed 1,704 Vitest tests locally, and the complete Rust and frontend suites
ran again against the final head in the three CI jobs below.

| Check                                                                       | Result and coverage                                                                                                                                                                                                  |
| --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm test`                                                                 | Rust workspace and Vitest passed. Stage 12 coverage includes strict v3 inputs, v1/v2 migration, document history, connectivity-derived wire diagnostics, natural BOM order, printable clipping, and export rollback. |
| `pnpm exec vitest run`                                                      | 101 files, 1,704 tests passed after the reorder fix, including the default-active-sheet interaction regression.                                                                                                      |
| `pnpm lint`, `pnpm format:check`, `pnpm typecheck`, `pnpm boundaries:check` | Passed locally.                                                                                                                                                                                                      |
| `pnpm types:generate`, `pnpm types:check`                                   | Rust-derived bindings regenerated and verified. The reorder follow-up changes TypeScript UI code and its test.                                                                                                       |
| `pnpm build`, `pnpm bundle:check`                                           | Windows release executable built. The production bundle contained no mock adapter.                                                                                                                                   |

The Rust app bridge tests compared compile and frame bytes with and without
documentation. The Engine divider test compared electrical results with
terminal and wire labels. Existing assignment load/refusal, DC rail
connectivity and editing, operator controls, parts drawer, and parameter editor
regressions ran in the full suites. Desktop testing for this record covered
the documented circuit, sheet navigation and reorder, native project
persistence, and export. Earlier workflow coverage in this record comes from
the automated suites. Native Ubuntu and macOS walkthroughs remain deferred
under the platform policy in [ARCHITECTURE.md](../../ARCHITECTURE.md).

## Candidate CI

All three jobs passed for `fa4ec9ae64c064e0a14fff1a779444c4c1c2cdec` in
[run 45073357158](https://github.com/SWOFART/open-circuit/actions/runs/45073357158).

| Platform | Result                                                                                    |
| -------- | ----------------------------------------------------------------------------------------- |
| Windows  | [Pass](https://github.com/SWOFART/open-circuit/actions/runs/45073357158/job/137365165771) |
| Ubuntu   | [Pass](https://github.com/SWOFART/open-circuit/actions/runs/45073357158/job/137365165905) |
| macOS    | [Pass](https://github.com/SWOFART/open-circuit/actions/runs/45073357158/job/137365165420) |
