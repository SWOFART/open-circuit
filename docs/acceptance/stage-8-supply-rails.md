# Stage 8 supply-rail acceptance record

**Issue:** [#135](https://github.com/SWOFART/open-circuit/issues/135).

**Recorded:** 2026-11-14.

**Base:** `7ae6bac99340af830c00c90469e4936c190e61e8` (`main` after #134).

**Contract:** [Stage 8 supply rails](../contracts/stage-8-supply-rails.md). **Reference data:**
[Stage 8 references](../solver/stage-8-references.md) and
[connectivity verification](../solver/stage-8-connectivity.md).

**Status:** local automated checks are recorded below. Independent Gate A, the candidate's
Windows/Ubuntu/macOS CI and Tauri builds, the **native desktop walkthrough** and the acceptance of both
workstreams are **pending**. This record does not declare Stage 8 accepted; issue #135 and the parent
#127 must stay open until those items are done.

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
| Windows Tauri release build | Built locally as `target/release/open-circuit-app.exe`; the candidate's CI build is pending |

Existing non-fatal messages: ts-rs does not parse `deny_unknown_fields`; Vite reports a bundle chunk over
500 kB. No lint, compiler or CI setting was weakened.

## Pending

- **Independent Gate A** for the exact candidate (AGENTS.md).
- **Candidate CI.** The base `main` revision passed
  [CI run 43632821665](https://github.com/SWOFART/open-circuit/actions/runs/43632821665); that is baseline
  evidence only. After Gate A passes, a draft PR is opened and its Windows, Ubuntu and macOS checks and Tauri
  builds must be added here.
- **Native desktop walkthrough.** **Not performed.** A request to control the native window was declined, so
  nothing below was done on the desktop: opening the rail projects (`target/tmp/stage8-live`, written from the
  pinned requests), the canvas rendering of the rails and their names, drawing a circuit with the DC rail tool,
  pressing and releasing START and STOP and reading the coil current and contactor state, the highlighted rail
  for a diagnostic (`duplicate-name`), and a save and reopen. The automated results above run the real Engine
  through the application's simulation core and its binary and JSON transports; they are a transport replay,
  not desktop evidence. A browser run of the real interface (no engine) showed the rail tool, the panel and the
  validation in #134, which is also not native evidence.
- **Acceptance by both workstreams** and the closing of #135 and #127.
- **#146** (refuse opening a rail-bearing project while an assignment is open) is a known gap in the contract's
  assignment section, not part of this record's evidence.
- No three-phase, AC or protective-earth behavior is advertised: the interface says rails are DC conductors and
  that `L1`, `N` and `PE` are labels.
