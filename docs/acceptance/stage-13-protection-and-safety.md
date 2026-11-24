# Stage 13 acceptance record: protection and safety

**Issue:** [#206](https://github.com/SWOFART/open-circuit/issues/206). **Parent:** [#158](https://github.com/SWOFART/open-circuit/issues/158).

**Recorded:** 2026-11-24.

**Base:** `main` at `e4c5241` (the merge of #303). **Candidate:** the revision that adds this record and the acceptance test `crates/app/tests/stage13_acceptance.rs`; the pull request that carries it holds its own CI run, which a record inside it cannot cite.

**Contract:** [Stage 13 protection and safety](../contracts/stage-13-protection-and-safety.md). **Reference data:** [Stage 13 references](../solver/stage-13-references.md), with the notes on [earth](../solver/stage-13-earth.md), [RCD](../solver/stage-13-rcd.md) and [breaker](../solver/stage-13-breaker.md).

**Status:** local automated checks on Windows, an end-to-end earth-fault test through the application core, and the CI of the pull requests are recorded below. **Not done and not claimed:** a native desktop walkthrough on any platform, and any run of the test suites on Ubuntu or macOS (the CI of this repository does not run them, see "Cross-platform CI"). The acceptance of both workstreams is **pending**. This record does not declare Stage 13 accepted; #206 and the parent #158 must stay open until the owner decides.

> No installation standard is claimed. Everything here describes a simulation for teaching (contract, first paragraph). A circuit that passes these checks is not thereby safe.

## What was delivered

| Issue | Pull request                                             | What                                                                                                                          |
| ----- | -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| #196  | [#286](https://github.com/SWOFART/open-circuit/pull/286) | The contract                                                                                                                  |
| #197  | [#287](https://github.com/SWOFART/open-circuit/pull/287) | The earth electrode, the PE net and the `leak` fault                                                                          |
| #198  | [#288](https://github.com/SWOFART/open-circuit/pull/288) | The residual-current device                                                                                                   |
| #199  | [#289](https://github.com/SWOFART/open-circuit/pull/289) | Breaker trip classes B, C and D, and the short circuit through the existing solver                                            |
| #201  | [#294](https://github.com/SWOFART/open-circuit/pull/294) | The exposed part, the touch voltage and the hazard result                                                                     |
| #202  | [#295](https://github.com/SWOFART/open-circuit/pull/295) | The reproducible reference fixtures (analytical; no ngspice, by the contract)                                                 |
| #203  | [#297](https://github.com/SWOFART/open-circuit/pull/297) | The bridge: the new devices, faults, results and diagnostics through the application                                          |
| #204  | [#299](https://github.com/SWOFART/open-circuit/pull/299) | Symbols, readouts and the RCD control                                                                                         |
| #205  | [#303](https://github.com/SWOFART/open-circuit/pull/303) | Fault-finding assignments with the `leak` fault and its diagnosis; the Stage 9, 10 and 13 devices open to fault-finding tasks |
| #293  | [#293](https://github.com/SWOFART/open-circuit/pull/293) | The rev-4 wait of a Stage 8 session test (a flaky test, issue #272), found while landing #289                                 |

Every pull request of the stage passed an independent Gate A review by a read-only subagent started by the implementing agent, not by a human reviewer; their non-blocking findings are listed in the pull requests.

## An earth fault trips the protective device as the references say

`crates/app/tests/stage13_acceptance.rs` draws one circuit and runs it with the real Engine behind `SimulationCore`, one frame per public step: a 230 V, 50 Hz supply with its return pole earthed, a 23 Ω load behind a two-pole RCD of 1 A rated residual current, and a `leak` fault of 65 Ω from the load's live pin to an exposed part whose electrode is 50 Ω. The leak current is 230 / (65 + 50) = 2 A, a residual current of twice the rated one, which is the independent reference `rcd-trip-ac-2` of `tests/golden/stage-13/analytical.json` (read by the test from the committed file).

| Check                                                                                                                                                                                                                                                                                                                                                                                                  | Result |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| The device trips at step 43 (43 ms); the reference gives 43.164 ms, within the reference's own `max(3 ms, 2 %)`                                                                                                                                                                                                                                                                                        | Pass   |
| Five steps before the trip the detector is above the rated 1 A and below 2 A (it is still rising: 1.92 A), the frame is valid, its touch voltage is 99.994 V RMS (peak 141.41 V, `√2` times it) against the 50 V limit, and the Engine's `hazard` is 1                                                                                                                                                 | Pass   |
| After the trip both pole currents are exactly 0, the frame's touch voltage is below 1 µV, `hazard` is 0 and the reading stays valid                                                                                                                                                                                                                                                                    | Pass   |
| The latched device refuses a close; an open and a close are accepted; it does not trip in the reply of its own close, and the fault, still there, trips it again                                                                                                                                                                                                                                       | Pass   |
| **The wrong wiring is not repaired:** with the electrode not on the frame's net the compile reply carries `ExposedPartNotBonded`, the RCD never trips in 400 ms (the residual is below 1 µA), the frame floats up to 229.99 V RMS and `hazard` is 1                                                                                                                                                    | Pass   |
| **Assignment:** the same drawing is written as a `findFault` task in the assignment file format and loaded by the Engine's decoder; the faults of the loaded task, applied to a run, trip the device at the same reference time; the diagnosis `leak` on the load's pin 1 passes, on pin 2 is `wrongKind` (partial), on another part `incorrect` (failed); a diagnosis that names the frame is refused | Pass   |

The rest of the evidence is in the tests of the earlier issues, which this record does not repeat: the closed-form fixtures (`crates/engine/tests/stage_13_references.rs`: earth loops, touch voltage, RCD trip times at DC and 50 Hz, breaker classes, short circuit; each correct fixture with wrong variants whose different results the Engine reproduces), the model tests (`stage_13_earth.rs`, `stage_13_rcd.rs`, `stage_13_breaker.rs`, `stage_13_touch.rs`), the bridge tests (`crates/app/tests/stage13_session.rs`), the assignment tests (`stage_13_assignment.rs`, `src/ui/assignment/stage13Assignment.test.tsx`, the golden file `tests/golden/stage-13-earth-fault.ocassignment`) and the interface tests (`src/symbols/stage13.test.ts`, `src/ui/stage13Panels.test.tsx`).

## Local checks

Run on Windows 11 Pro 10.0.26100 on the base `e4c5241` with the candidate's files.

| Command                                                             | Result                                          |
| ------------------------------------------------------------------- | ----------------------------------------------- |
| `cargo fmt --all --check`; prettier over `src/**`                   | Pass                                            |
| `cargo clippy --workspace --all-targets -- -D warnings`; `eslint .` | Pass                                            |
| `tsc --noEmit`                                                      | Pass                                            |
| `cargo test --workspace --no-fail-fast`                             | Pass: 681 tests, 0 failed                       |
| `vitest run`                                                        | Pass: 106 files, 1784 tests                     |
| `node scripts/check-types.mjs`; `node scripts/check-boundaries.mjs` | Pass                                            |
| `vite build`; `node scripts/check-production-bundle.mjs`            | Pass: seven production scripts, no mock adapter |
| `node scripts/stage13-references.mjs --check`                       | Pass (also run by vitest)                       |

Existing non-fatal messages: ts-rs does not parse `deny_unknown_fields` and `deserialize_with`; Vite reports a bundle chunk over 500 kB. No lint, compiler or CI setting was weakened.

## Cross-platform CI

Each job is "Checks and Tauri build". **The workflow does not run the test suites:** the step "Test (cargo test, Vitest)" is commented out since [#296](https://github.com/SWOFART/open-circuit/pull/296) ("Skip the full test suite in CI"). What the three jobs prove is formatting, lint, type-check, that the generated types are current, the architecture boundaries, the Tauri build and the production-bundle check. **They do not show that the Stage 13 tests pass on Ubuntu or macOS**; the tests were run on Windows only (the table above).

| Pull request                                           | Revision  | Run                                                                             | Windows | Ubuntu | macOS |
| ------------------------------------------------------ | --------- | ------------------------------------------------------------------------------- | ------- | ------ | ----- |
| #299 symbols and readouts                              | `8f65816` | [45490003008](https://github.com/SWOFART/open-circuit/actions/runs/45490003008) | Pass    | Pass   | Pass  |
| #303 assignments (last of Stage 13 before this record) | `005c4dc` | [45502802015](https://github.com/SWOFART/open-circuit/actions/runs/45502802015) | Pass    | Pass   | Pass  |

Runs on `main`:

| Run                                                                                                                                         | Revision  | Windows | Ubuntu | macOS |
| ------------------------------------------------------------------------------------------------------------------------------------------- | --------- | ------- | ------ | ----- |
| [45498348935](https://github.com/SWOFART/open-circuit/actions/runs/45498348935), push of the merge of #299                                  | `232ece4` | Pass    | Pass   | Pass  |
| [45502819590](https://github.com/SWOFART/open-circuit/actions/runs/45502819590), push of #301, the last completed run when this was written | `393fef9` | Pass    | Pass   | Pass  |

The run on the merge of #303 (`e4c5241`, the base of this record) was still in progress when this was written and is not claimed. Earlier in the stage the Stage 8 session test flake of issue [#272](https://github.com/SWOFART/open-circuit/issues/272) failed one Windows job of #289; it was rerun and fixed by #293 (48 concurrent runs of the bounded wait passed).

## Deviations from the contract that the owner should know

These are recorded in the contract and in [Stage 13 references](../solver/stage-13-references.md) and are listed here so that acceptance is an informed decision:

- Tolerances: the 50 Hz RMS tolerance is the Stage 9 `2e-3`, and the unbonded-frame current uses an absolute `1e-10 A`; both amend rows of contract section 11.
- The RCD **test button** is deferred by the contract (section 12). The interface has no test button.
- The exposed part has no discrete state, so the canvas does not colour it; the hazard is read from the devices panel with the teaching note.
- **Rails in assignments stay deferred** (contract section 3.4): three-phase fault-finding tasks are drawn with wires. The text of #205 mentions rails.
- The assignment file stays at version 1 (an additive change); an older application refuses a file with a `leak` through its existing unknown-field and unsupported-type refusals.
- The references are analytical only; the contract allows no ngspice reference for the RCD and breaker detectors, which are the Engine's own law.

## Pending

- **Acceptance by both workstreams** and the closing of #206 and #158.
- **A native desktop walkthrough** (build with `tauri build --no-bundle`, open an earth-fault circuit, watch the RCD trip, read the touch voltage and the teaching note, open a fault-finding assignment). None was done for this record on any platform.
- **The test suites on Ubuntu and macOS**, which the CI does not run; they pass here on Windows only.
- The designator letters and symbol standards of the new parts follow the editor's names of today and the owner's later decision (contract section 13, item 6).
- No protective-device coordination, breaking capacity, selectivity, TN/TT/IT classification, arc flash, lightning or manufacturer data is advertised; the interface says nothing of them.
