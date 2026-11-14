# Stage 8 supply-rail acceptance record

**Issue:** [#135](https://github.com/SWOFART/open-circuit/issues/135).

**Recorded:** 2026-11-14.

**Base:** `7ae6bac99340af830c00c90469e4936c190e61e8` (`main` after #134).

**Contract:** [Stage 8 supply rails](../contracts/stage-8-supply-rails.md). **Reference data:**
[Stage 8 references](../solver/stage-8-references.md) and
[connectivity verification](../solver/stage-8-connectivity.md).

**Status:** local automated checks, the candidate's Windows/Ubuntu/macOS CI and Tauri builds, and a native
Windows desktop walkthrough are recorded below. The acceptance of both workstreams is **pending**, as is any
native observation on macOS or Ubuntu (only CI ran there). This record does not declare Stage 8 accepted;
issue #135 and the parent #127 must stay open until both workstreams accept it.

## Editor-to-Engine evidence

`src/store/stage8Drawing.test.ts` draws six circuits from an empty document with the editor's commands
(`placeComponent`, parameter edits, `addWire`, `addReferenceMarker`, `addRail`) and the real symbol
geometry, asserts that each drawing equals, object for object and ignoring IDs, the independent fixture in
`tests/golden/stage-8-circuits.json`, and pins the compile request as
`tests/golden/stage-8-drawn-*.request.json`. A missing or different file fails; nothing regenerates
silently. Deliberate regeneration in PowerShell:

```powershell
$env:UPDATE_GOLDEN = '1'
pnpm exec vitest run src/store/stage8Drawing.test.ts
Remove-Item Env:UPDATE_GOLDEN
```

`crates/app/tests/stage8_session.rs` runs those exact requests on the real Engine behind
`SimulationCore`, with a frame at every public step, applies the fixture's parameter and operator events at
their newest steps as the application sends them, and checks every observation. The expected numbers are
the fixture's independent ones (Ohm and Kirchhoff, closed-form RC, Stage 6 answers; no Engine output). It
then replays the explicitly wired twin (`wireSchematic`, read through the versioned decoder) and compares
frame by frame within the manifest's equivalence tolerance (1e-9 absolute and relative). No production
mock or dependency was added.

| Drawn circuit | Fixture observations (steps) | Tolerance | Largest deviation | Wired twin |
|---|---|---|---|---|
| `divider` (5 V, 0.1 Ω, 2 × 1 kΩ; R1 edited to 3 kΩ at step 10) | 24 values (0, 10, 20) | 1e-9 / 1e-6 | 4.2e-15 | equal, 1e-9 |
| `start-stop` (seal-in; START press/release 5/10, STOP press/release 15/20) | 7 values + contactor state (0, 3, 5, 8, 12, 15, 25) | 1e-4 A | 2.3e-6 A | equal, discrete states identical |
| `start-stop-no-seal-in` (wrong wiring) | 2 values + state (5, 12) | 1e-4 A | 2.3e-6 A | equal |
| `rc-hold` (NC button held 50–100, 100 µF through 1 kΩ) | 10 values (0, 40, 49, 50, 75, 99, 100, 150, 200) | 1e-9 / 5e-3 / 1e-12 | 2.8e-4 V | equal |
| `two-islands` (two supplies, no wire, no marker) | 8 values (0, 10) | 1e-6 / 1e-9 | 0 | equal |
| `two-islands-N-PE` (the rails named `N`, `PE`, `L1`, `L2`) | 8 values (0, 10) | 1e-6 / 1e-9 | 0 | equal |

Both island circuits compile with exactly the two `NoGroundReference` warnings the fixture expects, and
the names `N` and `PE` change nothing: the results equal those of the same circuit with other names.

## START/STOP, wrong wiring and references

The seal-in circuit picks up with coil current 49.99 mA at step 5, stays picked with 49.94 mA after START
is released (step 12), and drops out after STOP (step 25 observation). The drawing without the seal-in
contact is accepted as drawn and is visibly different: the contactor is released at step 12 while the
correct one is picked (coil 49.9 mA against 0 A), so a wrong wiring is shown, not repaired. The explicit
reference marker on the source's negative terminal sets the global reference in the connected circuits; the
two islands, with none, each keep their own automatic reference and warning.

## Determinism

Two runs of the same drawing with the same events (a new compile each) report identical floating-point bits
in every frame, for all six circuits (`every_circuit_drawn_between_rails_matches_its_answers_its_wired_twin_and_itself`).
This is a same-environment guarantee. The contract promises no bit identity across CPU architectures or
operating systems, and the three-platform CI holds results to the tolerances above, never to stored bits.

## Workflows

Where each acceptance item is exercised (all on the real Engine or the real stores; none by a production
mock):

| Item | Evidence |
|---|---|
| Create, rename, move, resize, delete a rail, each a complete compile and a fresh run at step 0 with the operator and discrete state reset | `creating_renaming_moving_resizing_and_deleting_a_rail_each_start_a_fresh_run` (`stage8_session.rs`); `stage8Session.test.ts`; `railDocument.test.ts` and `railAuthoring.test.ts` (store, history, tool, drag) |
| Cancel and undo/redo | `railAuthoring.test.ts` (Escape, pointer cancel, Undo during a preview, drag released where it began); `railDocument.test.ts` (undo/redo restore IDs and values, redo cleared, saved baseline) |
| Wire attachment and crossings | `rails.test.ts` (all 27 contract cases of `stage-8-connectivity.json`, junction spots, dots, attachments, the wire, junction and reference tools on rails); `stage_8_references.rs` (Engine truth table and permutations) |
| Save and reopen | `a_saved_and_opened_rail_project_keeps_inputs_and_geometry_and_stores_nothing_derived`, `a_saved_and_reopened_rail_drawing_recomputes_a_fresh_run_without_the_operator_positions`, `a_legacy_project_opens_as_version_two_and_the_file_is_not_rewritten` (`stage8_session.rs`): names, IDs and geometry round-trip, nothing derived is stored, results recompute from step 0 |
| Malformed and unsupported inputs, diagnostics, highlighted rails, failed compiles, refused updates | `rail_diagnostics_carry_typed_references_and_leave_the_previous_run_alone`, `an_isolated_rail_is_a_floating_node_that_names_the_rail`, `a_refused_open_or_save_never_touches_the_current_file`; `railPanel.test.tsx` (the Engine's text under the rail); `rails.test.ts` (flagged rails drawn as warnings) |
| Rapid structural edits, late old-run and old-revision replies, frames and traces, editable failure | `stage8Session.test.ts` (compile coalescing, withdrawn values, dropped frames, editable after a diagnostic); `a_replaced_rail_document_leaves_no_old_run_value_frame_or_trace_to_read` |
| Live updates, operator events, stop notice, service responsiveness, finite frames, ordering and backpressure | `the_service_runs_a_rail_circuit_with_live_operator_and_parameter_updates`, `a_rail_circuit_that_fails_while_running_stops_with_a_notice_and_keeps_the_document` |
| No new binary envelope, slot, state or signal for rails | `rails_add_no_slot_state_trace_signal_or_frame_field` (the layout equals the wired drawing's) |
| Assignments keep their restrictions; rails are off in an assignment | `railAuthoring.test.ts` (`commandVerdict`), `railPanel.test.tsx`; the open gap is #146 |

## Checks on Windows

| Command (there is no `pnpm` on the recording machine; the same tools run from `node_modules/.bin`) | Result |
|---|---|
| `cargo fmt --all --check`; prettier over the `format:check` globs | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings`; `eslint .` | Pass |
| `tsc --noEmit` | Pass |
| `cargo test --workspace` | Pass |
| `vitest run` | Pass |
| `node scripts/check-types.mjs` | Pass: generated bindings unchanged |
| `node scripts/check-boundaries.mjs` | Pass |
| `vite build`; `node scripts/check-production-bundle.mjs` | Pass: seven production scripts, no mock adapter |
| Windows Tauri release build | Built locally with `tauri build --no-bundle` (frontend embedded) for the walkthrough below; the CI builds are listed next |

Existing non-fatal messages: ts-rs does not parse `deny_unknown_fields`; Vite reports a bundle chunk over
500 kB. No lint, compiler or CI setting was weakened.

## Cross-platform CI and Tauri builds

| Run | Revision | Windows | Ubuntu | macOS |
|---|---|---|---|---|
| [43634831275](https://github.com/SWOFART/open-circuit/actions/runs/43634831275), pull request #147 (**the candidate**) | `ba0f7b9`, tree `ad9e42bc601b3c7e23bf5bcae600a58424b05a58` | Pass | Pass | Pass |
| [43640067212](https://github.com/SWOFART/open-circuit/actions/runs/43640067212), pull request #148 (adds #146, same Stage 8 tests) | `cbce9d9` | Pass | Pass | Pass |
| [43637193497](https://github.com/SWOFART/open-circuit/actions/runs/43637193497), push of the merge commit to `main` | `093ed70` | Pass | Pass | **Fail** |

Each job is "Checks and Tauri build", so it includes the checks of the table above and the Tauri build. The
base `main` run [43632821665](https://github.com/SWOFART/open-circuit/actions/runs/43632821665) is baseline
evidence only.

The failure on the merge commit is a test of an earlier stage, not of Stage 8:
`a_fast_ui_does_not_speed_physics_up_or_slow_it_down` (`crates/app/tests/simulation_core.rs`) reached step 1956
where it expects 1980 to 2000, on the macOS runner only; the same tree passed on all three platforms in the run
of the pull request above. It is a timing dependence and is tracked as
[#149](https://github.com/SWOFART/open-circuit/issues/149). It is recorded here rather than hidden: macOS
passed for the candidate's own run, but `main` at the merge commit was red there once.

## Native Windows desktop walkthrough

**Platform and build.** Windows 11 Pro 10.0.26100, one monitor in use. The executable was built on 2026-11-14 at
21:59 with `tauri build --no-bundle` from the tree of pull request #148 (`main` at `093ed70` plus the #146
change, commit `cbce9d9`), so the frontend is embedded and no dev server was involved. The agent controlled the
native window with the computer-use plugin after the user granted access. The input projects were written from
the `schematic` of the pinned drawn requests (`tests/golden/stage-8-drawn-*.request.json`) into ignored
`target/tmp/stage8-live/*.ocircuit` and opened through the native Open dialog. The Engine, the bridge and the
numbers were the real ones. Press and release times were human-scale, so the exact millisecond timelines are
established by the automated replay above, not by this walkthrough.

| Check | Observed result |
|---|---|
| Open `start-stop` | The native canvas draws the rails `L+` and `0V` as green lines with end dots and their names, the STOP and START buttons, the contactor and the seal-in wires. The panel lists `L+ (x 0, y 0 to 4)` and `0V (x 14, y 7 to 10)`. At rest: V1 5 V, every current 0 A, S1 (NC) released and conducting, S2 (NO) released and not conducting, Q1 released. |
| Press and release START | Q1 picked, coil current 49.94506 mA (the independent answer is 49.94506 mA), S2 released and not conducting, S1 carrying 49.94506 mA: the contactor stays picked through the seal-in contact. |
| Press and release STOP | Q1 released. |
| Open `start-stop-no-seal-in` (wrong wiring) | The drawing visibly lacks the two seal-in wires. A fresh run starts (simulation time back to about 3.8 s, Q1 released). After START is pressed and released, Q1 is released with coil current 0 A, and its last change shows it did switch and drop: the distinct result, nothing repaired. |
| Open the `duplicate-name` case saved as a project | Refused: "The project operation failed: Project schematic failed validation: DuplicateRailName." The current document and run stay. |
| DC rail tool: two clicks at a place no conductor touches | A rail `RAIL1` is drawn and drawn in red; Results reads "The engine could not calculate this circuit. There is no result, and nothing below is a reading of it. ERROR A node is not connected to any reference. (DC rail RAIL1)"; Devices says there is no active run. |
| Undo | The rail disappears, Undo is disabled and Redo enabled, and the red marking is gone. |
| Rename, save, reopen | From `start-stop`, the rail `L+` was selected on the canvas and renamed to `SUPPLY` in the panel (the canvas label and the list followed). Save As wrote `saved-supply.ocircuit` and the title returned from "Unsaved changes" to "Project". The file is schema 2 with the rails `SUPPLY` and `0V`, identical to the original except the name, with no state, result or trace word. Reopened: the name `SUPPLY` is back, history is empty, a fresh run starts (simulation time about 3.8 s, Q1 released, all currents 0 A), and START then gives Q1 picked with 49.94506 mA. |
| #146 in the native window | With the Student assignment panel open the DC rail tool is disabled; Open on `start-stop` is refused with "The project operation failed: This project contains DC rails, which assignments do not support. Leave assignment mode to open it." and the document and the picked contactor are unchanged. (This confirms Tauri maps `forbidRails` to `forbid_rails`.) |

Not done on the desktop and not claimed: the keyboard-only **Create rail** form, dragging a rail or an end handle,
rail deletion and Redo in the native window (they are covered by the Vitest tests of #134), diagnostics for
the rail-name and limit errors (they are covered by the real-Engine tests), keyboard accessibility audit, and any
macOS or Ubuntu desktop run. One defect was seen: a project-operation banner stays after later successful edits
([#150](https://github.com/SWOFART/open-circuit/issues/150)).

## Pending

- **Acceptance by both workstreams** and the closing of #135 and #127.
- **Native observations on macOS and Ubuntu**, which this record does not have; CI built and tested there only.
- #149 (the timing-dependent test) and #150 (the stale banner) are open and are not Stage 8 acceptance items.
- No three-phase, AC or protective-earth behavior is advertised: the interface says rails are DC conductors and
  that `L1`, `N` and `PE` are labels.
