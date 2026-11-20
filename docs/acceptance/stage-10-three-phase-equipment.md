# Stage 10 acceptance record: three-phase equipment

**Issue:** [#181](https://github.com/SWOFART/open-circuit/issues/181). **Parent:** [#155](https://github.com/SWOFART/open-circuit/issues/155).

**Recorded:** 2026-11-20.

**Base:** `main` at `3a8d142f2a74b1cb6f1172405cb3e3f7e86b46ff` (the merge of #278), tree `99a631d390fb37968be44731cbee8f063fd9c901`. **Candidate:** the revision that adds this record, the editor drawing test with its four pinned requests, the Rust acceptance test and the shared test helpers; the pull request that carries it holds its own CI run, which a record inside it cannot cite.

**Contract:** [Stage 10 three-phase equipment](../contracts/stage-10-three-phase-equipment.md). **Reference data:** [Stage 10 references](../solver/stage-10-references.md) and [solver notes](../solver/stage-10-equipment.md).

**Status:** local automated checks, the CI of the pull requests and of `main`, an editor-to-Engine replay of four drawn starters, and a native Windows desktop walkthrough are recorded below. The acceptance of both workstreams is **pending**, as is any native observation on macOS or Ubuntu (only CI ran there). This record does not declare Stage 10 accepted; #181 and the parent #155 must stay open until both workstreams accept it.

## What was delivered

| Issue | Pull request                                             | What                                                                                                                     |
| ----- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| #172  | [#262](https://github.com/SWOFART/open-circuit/pull/262) | The contract                                                                                                             |
| #173  | [#263](https://github.com/SWOFART/open-circuit/pull/263) | The linked pin-group framework and the catalog `pinGroups`                                                               |
| #174  | [#264](https://github.com/SWOFART/open-circuit/pull/264) | The three-pole contactor and the breaker                                                                                 |
| #175  | [#267](https://github.com/SWOFART/open-circuit/pull/267) | The thermal overload relay                                                                                               |
| #176  | [#270](https://github.com/SWOFART/open-circuit/pull/270) | The three-phase induction motor                                                                                          |
| #177  | [#271](https://github.com/SWOFART/open-circuit/pull/271) | The transformer and the three-phase loads                                                                                |
| #178  | [#273](https://github.com/SWOFART/open-circuit/pull/273) | The reproducible reference fixtures (analytical and ngspice)                                                             |
| #179  | [#276](https://github.com/SWOFART/open-circuit/pull/276) | The bridge tests with the real Engine, and the fix of the live-parameter classification of the contactor and the breaker |
| #180  | [#278](https://github.com/SWOFART/open-circuit/pull/278) | Symbols, readouts and the breaker and reset controls                                                                     |
| #268  | [#277](https://github.com/SWOFART/open-circuit/pull/277) | The tick tests no longer depend on the speed of the machine                                                              |

Each pull request of the application workstream (#276 to #278) passed an independent Gate A review by a read-only subagent started by the implementing agent, not by a human reviewer; their non-blocking findings are listed in the pull requests.

## Editor-to-Engine evidence

`src/store/stage10Drawing.test.ts` draws four starters from blank documents with the editor's own commands (`placeComponent`, parameter edits, `addWire`, `addReferenceMarker`) and the real symbols: the **direct-on-line** starter and the **star-delta** starter, each with the correct wiring and with **two phases swapped** (the wrong wiring). The parts and their parameters are those of the independent fixture `tests/golden/stage-10/starters.json`, placed in the same order so that the editor numbers them with the fixture's IDs. The wiring is drawn from connection lists, one bus per net above or below the row of parts, with the pins of the new symbols. The test checks that the drawn parts, parameters and IDs equal the fixture's, and pins each compile request as `tests/golden/stage-10-drawn-*.request.json`; a missing or different file fails and nothing regenerates silently. Deliberate regeneration in PowerShell:

```powershell
$env:UPDATE_GOLDEN = '1'
pnpm exec vitest run src/store/stage10Drawing.test.ts
Remove-Item Env:UPDATE_GOLDEN
```

`crates/app/tests/stage10_acceptance.rs` takes those pinned requests and checks them with the Engine, not with the TypeScript that drew them:

| Check                                                                                                                                                                                                                                                                                                                                                   | Result |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| The pins that each net joins, by the Engine's own connectivity (`derive_netlist`), equal the fixture's net for net, for all four drawings                                                                                                                                                                                                               | Pass   |
| The parts, IDs, parameters and the one reference marker equal the fixture's; no junction is drawn                                                                                                                                                                                                                                                       | Pass   |
| The two wrong wirings join other pins than the correct starters                                                                                                                                                                                                                                                                                         | Pass   |
| Each drawing runs through the application core (`SimulationCore`) with the fixture's operator events at their steps and is held to the independent **analytical** and **ngspice** rows (more than 20 rows each): winding currents within max(0.12 A, 2%), speed within max(0.5 rad/s, 2%), torque within max(0.05 N·m, 3%), the Engine's own tolerances | Pass   |
| The overload relay stays intact, and in the star-delta starter star and delta never overlap, main and star are picked from step 25 to 110, only main from 125 to 140, main and delta from step 145                                                                                                                                                      | Pass   |
| The wrong wiring is visible and not repaired: the correct starters end at a positive speed above 100 rad/s, those with two phases swapped at one below −100 rad/s                                                                                                                                                                                       | Pass   |
| Two runs of the same drawing are bit-identical in this environment (direct-on-line and star-delta; the contract promises no bit identity across operating systems, and CI holds results to the tolerances)                                                                                                                                              | Pass   |
| A drawing saved and reopened keeps every drawn input, the file holds no derived word, and a run of the reopened document starts at step 0 and repeats the same frames                                                                                                                                                                                   | Pass   |
| **Failure handling:** the direct-on-line starter without the three shunts across the motor windings, when the control circuit is opened, runs until the contactor releases, then **stops** with its last frame finite and its revision as it was, instead of inventing a result                                                                         | Pass   |

`crates/app/tests/stage10_session.rs` (issue #179) runs the same fixtures, a breaker and an overload relay through the bridge: values and discrete states equal, bit for bit, the same events sent straight to an Engine; the new result keys, units and records; a breaker closed, tripped, refused and reset; an overload relay tripped, refused an early reset and reset after cooling; refused updates changing nothing; stale run, stale revision and replaced-run replies; project round trips; and the live-parameter list pinned against the Engine.

At the last observation step (0.243 s) both oracles give, for the direct-on-line starter, a speed of 150.10 rad/s and a torque of 1.604 N·m, for the starter with two phases swapped −151.03 rad/s and −1.412 N·m, and for the star-delta starter 157.88 rad/s and 2.939 N·m.

## Local checks

Run on the base `3a8d142` with the candidate's files.

| Command                                                             | Result                                                                                                      |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `cargo fmt --all --check`; prettier over the `format:check` globs   | Pass                                                                                                        |
| `cargo clippy --workspace --all-targets -- -D warnings`; `eslint .` | Pass                                                                                                        |
| `tsc --noEmit`                                                      | Pass                                                                                                        |
| `cargo test --workspace --no-fail-fast`                             | Pass: 566 tests, 0 failed (the two starter binaries take about 70 s and 100 s, their tests run in parallel) |
| `vitest run`                                                        | Pass: 1646 tests                                                                                            |
| `node scripts/check-types.mjs`; `node scripts/check-boundaries.mjs` | Pass                                                                                                        |
| `vite build`; `node scripts/check-production-bundle.mjs`            | Pass: seven production scripts, no mock adapter                                                             |
| Windows Tauri release build                                         | `tauri build --no-bundle` of the base, used for the walkthrough below                                       |

Existing non-fatal messages: ts-rs does not parse `deny_unknown_fields`; Vite reports a bundle chunk over 500 kB. No lint, compiler or CI setting was weakened.

## Cross-platform CI and Tauri builds

Each job is "Checks and Tauri build", so it includes the checks above and the Tauri build.

| Pull request                                                | Revision  | Run                                                                             | Windows | Ubuntu | macOS |
| ----------------------------------------------------------- | --------- | ------------------------------------------------------------------------------- | ------- | ------ | ----- |
| #277 tick tests                                             | `c617fb8` | [44789361413](https://github.com/SWOFART/open-circuit/actions/runs/44789361413) | Pass    | Pass   | Pass  |
| #276 bridge                                                 | `dfefb7c` | [44795480523](https://github.com/SWOFART/open-circuit/actions/runs/44795480523) | Pass    | Pass   | Pass  |
| #278 symbols and readouts (**the last change of the base**) | `7ad5fc2` | [44797056055](https://github.com/SWOFART/open-circuit/actions/runs/44797056055) | Pass    | Pass   | Pass  |

Runs on `main`:

| Run                                                                                                                                     | Revision  | Windows  | Ubuntu | macOS    |
| --------------------------------------------------------------------------------------------------------------------------------------- | --------- | -------- | ------ | -------- |
| [44750886764](https://github.com/SWOFART/open-circuit/actions/runs/44750886764), push of the merge of the Stage 10 engine pull requests | `141e213` | **Fail** | Pass   | Pass     |
| [44772296786](https://github.com/SWOFART/open-circuit/actions/runs/44772296786), push of the merge of #275                              | `0c3be6c` | **Fail** | Pass   | **Fail** |
| [44808461012](https://github.com/SWOFART/open-circuit/actions/runs/44808461012), push of the merge of #278                              | `3a8d142` | Pass     | Pass   | Pass     |

Both red runs on `main` are the same test of an earlier stage, not of Stage 10: `a_fast_ui_does_not_speed_physics_up_or_slow_it_down` (`crates/app/tests/simulation_core.rs`). It used a fake clock but the tick loop gave up after a real 8 ms, so on a slow runner the step count fell behind. It was found to fail consistently on Windows and macOS (issue [#268](https://github.com/SWOFART/open-circuit/issues/268), after the earlier #149 was closed), and #277 made it independent of machine speed; the runs of #277, #276 and #278 are green on all three platforms. The push run of the base, the revision all the checks above refer to, passed on all three platforms.

## Native Windows desktop walkthrough

**Platform and build.** Windows 11 Pro 10.0.26100, one monitor in use. The executable was built on 2026-11-20 with `tauri build --no-bundle` from the base `3a8d142` (frontend embedded, no dev server). The agent controlled the native window with the computer-use plugin after the user granted access. The four drawn starters were written from the `schematic` of the pinned requests into ignored `target/tmp/stage10-live/*.ocircuit` and opened through the native Open dialog. The Engine, the bridge and the numbers were the real ones. Times are human-scale (the control switch was clicked, not scheduled at a step), so the exact timelines are established by the automated replay above, not by this walkthrough.

| Check                                        | Observed result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Open `direct-on-line`                        | The Inspector lists G1 (the three-phase source), V1, FR1 (the overload relay), S1, Q1 (the contactor), R1, M1 (the motor) and R2 to R4 with the parameters of the fixture. After "Fit to circuit" the canvas draws the new symbols: the source, the three sensing elements of FR1, the three-pole contactor Q1 with its coil and auxiliary contacts, the motor with its M and ~, joined by the drawn buses. At rest the Devices panel shows FR1 intact, S1 open, Q1 released and all currents 0 A. |
| Rest speed                                   | The motor reads about −10 rad/s with 0 A and 0 N·m: the signed load torque (0.1 N·m) with the friction (10 mN·m·s/rad) turns an unpowered motor, because the contract adds no shaft lock. This is the model, not a defect.                                                                                                                                                                                                                                                                         |
| Close S1 (start)                             | The S1 control reads Closed, Q1 picked (last change 2.999969 s), coil current 2.394732 A, the three pole currents 16.69 A, −6.14 A and −10.55 A, and the motor at 114.8 rad/s one second later. After four more seconds the motor reads **150.1073 rad/s** with **1.601073 N·m**, the oracles' 150.10 rad/s and 1.604 N·m at the last observation step.                                                                                                                                            |
| Open S1 (stop)                               | Q1 released, the pole currents 0 A, the contactor's detector decays to about 1e-14 A², and the motor coasts down (45 rad/s two seconds later).                                                                                                                                                                                                                                                                                                                                                     |
| Open `direct-on-line-swapped` (wrong wiring) | A fresh run starts (S1 open, Q1 released, motor at rest speed). After S1 is closed, the motor runs at **−151.0264 rad/s** with **−1.410265 N·m**, the oracles' −151.03 rad/s and −1.412 N·m: the distinct result, nothing repaired.                                                                                                                                                                                                                                                                |
| Open `star-delta`                            | The three contactors Q1 to Q3 and the three switches are drawn and the run starts with live results. The start sequence by hand was not carried out in the window (see below).                                                                                                                                                                                                                                                                                                                     |
| Engine warning                               | The Results panel shows the Engine's warning "The circuit is too fast for the available internal step limit; simulation accuracy may be limited." for these starters, whose contactor coil has an inductance of 10 µH. It is the Engine's own diagnostic, shown as it came.                                                                                                                                                                                                                        |

Not done on the desktop and not claimed: pressing the star-delta start sequence by hand (its interlocks and times are covered by the automated replay), the breaker and the overload relay's trip and reset in the window (covered by the bridge tests; no drawn starter contains a breaker), placing the new parts one by one from the drawer and drawing a starter by hand with the mouse (the drawings were produced by the editor's commands and opened as project files), the keyboard-only operation of the Devices panel's controls and any screen-reader audit, and any macOS or Ubuntu desktop run (CI built and tested there only). Reading the Devices panel needs a long scroll in the right column; that is a layout matter, not a Stage 10 item.

## Pending

- **Acceptance by both workstreams** and the closing of #181 and #155.
- **Native observations on macOS and Ubuntu**, which this record does not have.
- The designator letters of these parts (`QF`, `FR`, `Q`, `M`, `T`) will change with the Stage 12 contract; the tests that name parts follow the editor's names of today.
- Issue [#272](https://github.com/SWOFART/open-circuit/issues/272) (a rail service test that can stop before the final revision frame) is open and is not a Stage 10 item.
- No protective earth, fault-current, frequency converter, harmonic, saturation or detailed thermal behavior is advertised; the interface says nothing of them.
