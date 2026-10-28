# Stage 5 assignments and fault finding acceptance record

**Issue:** [#85 — Stage 5: Verify the assignment and fault-finding workflow end to end](https://github.com/SWOFART/open-circuit/issues/85)
**Recorded:** 2026-10-28.
**Scope:** A teacher authors and saves an assignment, a student opens it and is checked, and a student
finds a fault, on the real Rust Engine and through the application's bridge formats. Contract:
[stage-5-assignments.md](../contracts/stage-5-assignments.md). Fixtures:
`tests/golden/stage-5-*.expected.json`, and the files drawn and authored for this record in
`tests/golden/stage-5-drawn-*`.

**Status:** the automated part of this record passes. The live desktop check was **not performed** by
the agent that prepared this record; see "Live desktop check" below. The issue should stay open until a
person has run it and added the result, and the pull request's own CI run is linked.

## Circuits and expected values

The circuits are drawn from a blank document with the editor's own commands, so the pin geometry is the
editor's, and the assignments are authored through the draft store of the authoring panel and its
conversion to the assignment document (`src/store/stage5Drawing.test.ts`). The student side's requests
are built by the real functions (`checkStudentCircuit`, the simulation controller) and pinned as golden
files. `crates/app/tests/stage5_session.rs` runs those files with the real Engine and the bridge code
(`AssignmentSession`, `CheckService`, `SimulationCore`) and checks them against the fixtures.

| Fixture | Circuit and attempts as drawn | Expected | Tolerance |
|---|---|---|---|
| Divider to specification (`build`) | Locked ideal 10 V V1 and locked 10 MΩ VM1 from the output node to ground; checks: reading 2.5 V ± 25 mV at 0 s, reading in [2.4, 2.6] V from 0 to 10 ms, exactly two resistors | 30 kΩ over 10 kΩ reads 2.498126405 V: `passed`. Same reading with three resistors: `partial`. Two 10 kΩ resistors read 4.997501249 V: `partial`. No resistors read 0 V: `failed`. A changed locked source (20 V): `failed` (locked check). A wire across the source: `notEvaluated` with `ShortCircuit`, the part-count criterion still evaluated | 1e-9 V (1e-12 V for 0 V) |
| Fault finding (`findFault`, reveal after a check) | V1 ideal 10 V, R1 = R2 = 1 kΩ in series, 10 MΩ VM1 across R2, all four parts locked; the fault is R2 open | Healthy reading 4.99975001 V; faulted 9.99900010 V with R1 carrying 0.9999 µA, R2 0 A and 9.999 V across it; R2 short 9.99999e-6 V, R1 open 0 V, R1 short 9.99998999 V. Diagnoses: R2 open `passed`; R2 short `partial` (wrong kind); R2 open plus R1 short `partial`; R1 open `failed`; none `failed` | 1e-9 V, 1e-12 A, 1e-12 V (R2 short, R1 open) |

All reference values are direct circuit equations from the fixtures, not Engine output.

The fuse-state fixture (a `deviceState` criterion) is verified by the Engine's and the bridge's tests
(`crates/app/tests/assignment_bridge.rs`, `crates/engine/tests/stage_5_checks.rs`) on the fixture's own
pin geometry, not drawn in the editor.

## Automated evidence

| Acceptance item | Evidence | Result |
|---|---|---|
| A teacher creates and saves the canonical build assignment; a student opens it | `stage5Drawing.test.ts` authors the divider assignment (title, instructions, starting circuit, reference solution, locks, allowed part types, three checks with the signal chosen from the editor's model of the Engine's result slots, which the Engine's reading of the voltmeter confirms) through the draft store and pins the file. `stage5_session.rs`: the Engine accepts it, `AssignmentSession` saves and reopens it unchanged (no result, trace, or verdict in the file), and the student's copy, as the application writes and reads it, is the same task without `reference`. `stage5EndToEnd.test.tsx`: the Engine-written student copy equals the student side's own copy of the authored file, and the draft conversion round-trips the file. | Pass |
| A correct, an incorrect, and a partial attempt each get the agreed structured result | `stage5_session.rs` runs the six pinned check requests through `CheckService`: outcome, the verdict of each criterion, the locked result, the readings and their units, and the diagnostics match the fixture. The same request gives the same result, and no result number is lost. `stage5EndToEnd.test.tsx` rebuilds each request with the student side's functions (a request that differs from the recorded one is not answered) and shows each result in the panel: measured and expected values with units, tolerance, and the message of each criterion, and the problems of a circuit that does not compile. | Pass |
| The faulty circuit behaves as the fixture says | `stage5_session.rs`: the drawn circuit compiled with the assignment's fault reads the faulted value at step 0 and at step 5, R2 carries exactly 0 A, R1 the meter's current, and R2 9.999 V across it; the other three hypotheses read the fixture's values; the same document without the faults is the healthy circuit. `stage5EndToEnd.test.tsx` replays the real compile reply and OCF2 frames through the adapter, the controller, and the Results panel: the controller sends the pinned request with the faults, and the panel shows 9.999 V. | Pass |
| A correct and an incorrect diagnosis are checked | `stage5_session.rs`: the five diagnoses of the fixture, through `check_diagnosis` on the student's copy, give the fixture's outcomes, entry verdicts, and fault verdicts. `stage5EndToEnd.test.tsx`: through `checkStudentDiagnosis` and the result view; with the policy `never` only the outcome is rendered, with `afterCheck` each finding, each fault, and the faulty part. | Pass |
| The fault is never written into any saved file | `stage5_session.rs`: the student's progress file (`write_project_file` of the circuit the faulted run was compiled from) holds no fault, kind, fault ID, instruction, or check, and reads back as the starting circuit. `stage5Drawing.test.ts`: the pinned request holds the faults beside a schematic that has none. `stage5EndToEnd.test.tsx`: the schematic JSON contains no fault. (The assignment file itself holds the faults by design; contract section 2.4.) | Pass |
| Assignment files round-trip; computed state is recalculated, not persisted | `stage5_session.rs` and `assignment_bridge.rs`: the saved teacher file and the student's copy contain no snapshot, layout, run ID, trace, measured value, or verdict, and reopen to the same assignment. Every check and every run is computed again from the request. | Pass |
| Stale or mismatched results are never shown after a run or document change | `stage5EndToEnd.test.tsx`: a check result is shown only for the circuit object it was asked for (another circuit, and an edited copy, get none), and a result whose revision is not the request's is dropped; after the faults go the controller starts a new run with its own reading, and a late frame of the faulted run is refused and counted. `stage5_session.rs`: after the same document is compiled again without the faults on the same core, an update that names the faulted run is refused as `StaleRun`. The authoring and student panels key their answers the same way (`answerFor`, `answerForKey`; `studentAssignment.test.ts`, `assignmentView.test.ts`). | Pass |
| Real Engine, no production mocks | The Rust session uses the real Engine through the bridge code; the replay replaces only the Tauri API. No mock adapter or mock assignment data is added, and the production bundle check passes. | Pass |
| Rust, TypeScript, contract, generated-type, and architecture-boundary checks | See "Checks run on Windows". | Pass |

## Findings

- **The reference result is shown on request, not on save.** Contract section 2.3 says the application
  shows the Engine's check of the reference "when the teacher saves". The authoring panel (#83) has a
  "Preview the checks on the reference solution" button and does not run it on save. The Engine and the
  bridge support both; the owners should decide whether Save should run the preview.
- **"Hidden parameters" is not in the contract.** Issue #84 lists it among the editor restrictions;
  contract section 3 defines only `allowedComponentTypes` and `lockedComponents`. A locked part's
  parameters are shown read only. If hiding is wanted it needs a contract change first.
- **A student's file must not become the teacher's save target.** Found in the review of #84: the bridge
  keeps one active assignment path, so opening an assignment as a student made it the target of the
  teacher's Save. The path is now cleared after a student open (`openStudentAssignment`).
- **The answer key is in the file, by design.** A student can read the faults, and the expected values of
  the checks, in the assignment file, and can check as often as they like; the reveal policy is a
  presentation policy only (contract sections 2.4 and 5.7). This record does not change that limit.
- No fixture value was found to disagree with the Engine: every value in the table above matched within
  its tolerance on the first run of the drawn circuits.

## Checks run on Windows

Collected on 2026-10-28 on the candidate of the pull request that adds this record.

| Command | Result |
|---|---|
| `prettier --check` (the `format:check` globs) and `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` and `eslint .` | Pass |
| `tsc --noEmit` | Pass |
| `cargo test --workspace` | Pass; 323 tests, including `stage5_session`, `assignment_bridge`, and the Engine's `stage_5_*` tests |
| `vitest run` | Pass; 58 files, 1110 tests, including the replay of the recorded session |
| `vite build`, then `scripts/check-production-bundle.mjs` | Pass; the bundle has no mock adapter |
| `scripts/check-types.mjs`, `scripts/check-boundaries.mjs`, `git diff --check` | Pass |

`pnpm build` (the Tauri release build) and the macOS and Linux runs were not run locally. The pipeline
runs them for every pull request on Windows, Ubuntu, and macOS (`.github/workflows/ci.yml`). The
implementation this record verifies, `main` at `e105f9a` (merge of #91, the last Stage 5 pull request),
passed all three jobs in
[GitHub Actions run 40744412716](https://github.com/SWOFART/open-circuit/actions/runs/40744412716).
The run for the pull request that adds this record is linked from that pull request.

## Live desktop check

**Not performed.** The agent that prepared this record cannot drive the desktop window, so the file
dialogs and the bridge in the real application are covered only by the tests above. No result below is
claimed.

A person should run the following in the desktop app (`pnpm dev`) and record the observations in this
section, then close the issue:

1. **Teacher.** New. Draw a 10 V source with 0 Ω internal resistance (it will be locked) and a 10 MΩ voltmeter
   from the output to ground, with a ground marker. Choose **Author assignment**, set a title and
   instructions, use the circuit as the starting circuit, add a 30 kΩ and a 10 kΩ resistor as the divider,
   use that as the reference solution, lock the source and the voltmeter, allow resistors only, and add
   a value check (VM1 voltage, 2.5 V, 25 mV), a range check (2.4 to 2.6 V, 0 to 10 ms), and a
   count check (two resistors). **Preview the checks**: every check passes. **Save assignment** and
   **Export student copy**.
2. **Student.** Choose **Student assignment**, **Open an assignment**, pick the student copy. The
   starting circuit appears as a new unsaved document, the palette offers resistors only, and the
   source and the voltmeter cannot be edited or deleted. Draw 10 kΩ over 10 kΩ and **Check my
   circuit**: partial, reading about 4.9975 V. Change to 30 kΩ over 10 kΩ and check again: passed,
   about 2.498 V. Edit the circuit: the old result disappears. Save the document as a project.
3. **Fault finding.** Author a fault task with the 10 V source, two 1 kΩ resistors in series, and a
   voltmeter across the second, all locked; add the fault "R2 open" and show the faulted circuit in the
   workspace: the voltmeter reads about 9.999 V. Save, export the student copy, and open it as a student.
   The voltmeter reads about 9.999 V, nothing names a fault, add a finding "R2, open" and **Check my
   diagnosis**: passed, and with the reveal policy "after a check" the finding and the fault are shown.
   Try "R1, open": failed. Save the progress as a project and open it in a text editor: no fault.
4. **Refusals.** Open a text file renamed to `.ocassignment` as a student and as a teacher: the file is
   refused with the Engine's errors, and the circuit and the draft are unchanged.

| Manual check | Observed result | Result |
|---|---|---|
| Steps 1 to 4 above | Not yet recorded | Pending |
