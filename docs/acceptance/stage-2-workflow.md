# Stage 2 workflow acceptance record

**Issue:** [#33 — Verify the complete Stage 2 editing workflow](https://github.com/SWOFART/open-circuit/issues/33)
**Recorded:** 2026-10-25.
**Scope:** Stage 2 editor and project persistence, using the Stage 1 divider and the real Rust Engine.

## Expected circuit and readings

The canonical divider contains a 5 V source with 0.1 Ω internal resistance, two 1 kΩ
resistors, three wires, and an explicit reference marker. Its expected output is the voltage
across R2.

| State | Exact Vout | UI display | Stage 1 tolerance |
|---|---:|---:|---:|
| R1 = 1 kΩ, R2 = 1 kΩ | 2.4998750062496877 V | 2.499875 V | 1 µV |
| R1 = 3 kΩ, R2 = 1 kΩ | 1.2499687507812304 V | 1.249969 V | 1 µV |

The values in this table are in `tests/golden/stage-1-divider.expected.json`. The Rust
Engine integration tests check the unrounded values. The bridge session is produced by the
real Engine and replayed through the TypeScript adapter, controller, and result UI.

## Automated evidence

| Acceptance area | Evidence | Result |
|---|---|---|
| Blank document lifecycle | `src/store/projectWorkflow.test.ts`: New clears the project path and produces a clean blank schematic. | Pass |
| Component placement and wire/reference editing | `src/ui/editorCommands.test.ts`, `src/editor/interaction.drawing.test.ts`, and `src/store/document.test.ts` cover catalog placement, drawing operations, stable IDs, and document history. `crates/engine/tests/stage_2_connectivity.rs` checks the Engine's connectivity for edited drawings. | Pass |
| Divider topology and 1 kΩ readings | `crates/app/tests/bridge_session.rs`, `crates/engine/tests/resistive_engine.rs`, and `src/store/endToEnd.test.tsx`. | Pass |
| 3 kΩ result and recalculation | The real Engine bridge session checks the 3 kΩ operating point; `src/store/endToEnd.test.tsx` checks the displayed result and restored 1 kΩ value. | Pass |
| Undo/redo state | `src/store/document.test.ts` and `src/ui/editorCommands.test.ts` check undo/redo document values, IDs, and wire state. | Pass |
| Project save/open round trip | `crates/app/src/project.rs` tests exact `Schematic` round trip, including component IDs, parameters, positions, wires, junctions, and reference markers; computed state is excluded. | Pass |
| Malformed/unsupported projects | `crates/app/src/project.rs` checks malformed JSON, unknown fields, unsupported versions, invalid IDs/rotations/parameters, and preservation of the active project after failed open. | Pass |
| Unsaved-change handling | `src/store/projectWorkflow.test.ts` checks Save, Discard, Cancel, canceled/failed Save, failed Open, and that valid work remains active. | Pass |
| Stage 1 physics, transport, stale frames, and production mocks | Full Rust/Vitest suites, including `src/store/endToEnd.test.tsx`, stale-frame tests, and `scripts/check-production-bundle.mjs`. | Pass |

## Checks run on Windows

| Command | Result |
|---|---|
| `pnpm format:check` | Pass |
| `pnpm lint` | Pass; Clippy emitted existing `ts-rs` notices that it ignores the `deny_unknown_fields` serde attribute. |
| `pnpm typecheck` | Pass |
| `pnpm types:generate` + `git diff --exit-code -- src/generated` | Pass; generated bindings are current. |
| `pnpm boundaries:check` | Pass |
| `pnpm test` | Pass; Rust tests and all 584 Vitest tests passed. |
| `pnpm build` | Pass; Windows Tauri release executable built at `target/release/open-circuit-app.exe`. |
| `pnpm bundle:check` | Pass; production bundle contains no mock adapter. |
| Lockfile diff check | Pass; `Cargo.lock` and `pnpm-lock.yaml` were unchanged. |

Linux/macOS Tauri builds and the associated CI run are pending the draft PR. No CI run is
linked yet.

## Manual desktop interaction still required

The application process built and launched locally, but the available computer-use and CUA
runtimes both failed to initialize in this session. As a result, the following user-visible
steps have **not** been observed in the running window and this issue is not yet fully
accepted:

- Start from **New**, place the three components from the palette, enter their parameters,
  draw the three wires, and place the explicit reference marker.
- Observe `Vout = 2.499875 V`; set R1 to `3 kΩ` and observe `Vout = 1.249969 V`.
- Undo and redo the parameter change and observe the readings follow the document.
- Save the project, close the application, reopen the saved file, and verify the schematic
  round-trips while results are recalculated.
- Confirm in the live window that malformed/unsupported files and unsaved-change choices
  leave valid work intact.

Record the operator, OS, app/tool versions, outcomes, and any deviations here after the
manual run. Do not mark the GUI steps passed based only on automated tests.
