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

These command results were collected on 2026-10-25 during the automated Stage 2 acceptance
run against implementation revision `35aa24d1f3378d4f6bd650730aadbb8b745ba799`, before this
documentation-only update. They are carried-forward evidence for that implementation; they
were not rerun for the current acceptance-record candidate. No automated test, formatting,
lint, type-check, or build command was rerun for this documentation-only change.

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

The previous PR head at commit `b5662a6666bf591c70ba4031376b40b06d184248` passed the
cross-platform CI matrix in
[GitHub Actions run 40190470648](https://github.com/SWOFART/open-circuit/actions/runs/40190470648).
The desktop-acceptance update committed as `35f6060ff19a16109467cb154b783c88b14289a8`
(tree `dcd3a1cde8a53564240b55445aa865453ad9f868`) subsequently passed the Windows,
Ubuntu, and macOS jobs in
[GitHub Actions run 40199537906](https://github.com/SWOFART/open-circuit/actions/runs/40199537906).
That run is evidence for that exact tree. The local Windows command results above remain
carried-forward evidence from the implementation revision; they were not rerun locally for
this documentation-only update.

## Manual desktop interaction

**Operator:** Codex, in a user-authorized live desktop session.
**Environment:** Windows desktop; Tauri development build started with `pnpm dev` from
source revision `35aa24d1f3378d4f6bd650730aadbb8b745ba799`.

| Manual check | Observed result | Result |
|---|---|---|
| New document, component placement, wires, and reference | Started from **New**, placed a 5 V source with 0.1 Ω internal resistance and two 1 kΩ resistors, drew the divider connections, and placed an explicit ground marker. The live results showed `Vout = 2.499875 V` and `2.499875 mA` through each resistor. | Pass |
| Parameter edit and undo/redo | Changed R1 to `3 kΩ`; the live result changed to `1.249969 V`. Undo restored `1 kΩ` and `2.499875 V`; redo restored `3 kΩ` and `1.249969 V`. | Pass |
| Save, close, and reopen | Saved the test project as an `.ocircuit` file, closed the window, restarted the development server after the local page was unavailable, and reopened the saved project. The component values, positions, wires, explicit ground, and `2.499875 V` divider result were visible after reopening. | Pass |
| Unsaved-change prompt | Changed R1 to `2 kΩ` (`Vout = 1.666611 V`) and selected **New**. The app displayed **Save / Discard / Cancel**. **Cancel** retained the modified circuit and result; Undo returned to the saved `1 kΩ` state and `2.499875 V`. | Pass |
| Malformed and unsupported projects | Opened a malformed JSON test file; the app reported a parse error. Then opened a valid JSON project with `schemaVersion = 2`; the app reported `InvalidSchemaVersion`. In both cases, the active 1 kΩ divider remained visible with `Vout = 2.499875 V`. Automated coverage in `crates/app/src/project.rs` also checks unknown fields and invalid IDs/rotations/parameters. | Pass |

The user-visible editing, recalculation, undo/redo, save/reopen, malformed/unsupported file,
and cancel-without-data-loss flows passed.
