# Stage 16 acceptance record

Issue [#232](https://github.com/SWOFART/open-circuit/issues/232), parent
[#161](https://github.com/SWOFART/open-circuit/issues/161). Contract:
[stage-16-editor-improvements.md](../contracts/stage-16-editor-improvements.md). Observed on 2026-11-28.
The candidate is `main` at `530e645f`, which holds every Stage 16 pull request (merged by a human).

## What was delivered

| Issue                                                      | Feature                                 | Pull request                                             | Reviewed tree (Gate A, by a subagent) |
| ---------------------------------------------------------- | --------------------------------------- | -------------------------------------------------------- | ------------------------------------- |
| [#224](https://github.com/SWOFART/open-circuit/issues/224) | The contract                            | [#317](https://github.com/SWOFART/open-circuit/pull/317) | passed on the second run              |
| [#225](https://github.com/SWOFART/open-circuit/issues/225) | Multi-selection, group move and delete  | [#323](https://github.com/SWOFART/open-circuit/pull/323) | `b3f9ae33`, after two failed rounds   |
| [#226](https://github.com/SWOFART/open-circuit/issues/226) | Copy, cut, paste, duplicate             | [#325](https://github.com/SWOFART/open-circuit/pull/325) | `9735ba61`                            |
| [#227](https://github.com/SWOFART/open-circuit/issues/227) | Half turn, mirror, the `mirrored` field | [#332](https://github.com/SWOFART/open-circuit/pull/332) | `9e85f4d0`                            |
| [#228](https://github.com/SWOFART/open-circuit/issues/228) | Canvas search, history panel            | [#333](https://github.com/SWOFART/open-circuit/pull/333) | `11b2bcfd`                            |
| [#229](https://github.com/SWOFART/open-circuit/issues/229) | Light and dark themes                   | [#334](https://github.com/SWOFART/open-circuit/pull/334) | `7bb5031c`                            |
| [#230](https://github.com/SWOFART/open-circuit/issues/230) | Autosave and crash recovery             | [#335](https://github.com/SWOFART/open-circuit/pull/335) | `6b411263`                            |
| [#231](https://github.com/SWOFART/open-circuit/issues/231) | Keyboard map and shortcut window        | [#336](https://github.com/SWOFART/open-circuit/pull/336) | `71c0586b`                            |

Every pull request states that its Gate A review was run by a subagent started by the implementing
agent, not by a human reviewer, and lists the non-blocking findings.

## Commands and results

Run on `main` at `530e645f` (the working tree equals it apart from this record), Windows 11:

| Command                                                      | Result                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo fmt --all -- --check`                                 | pass                                                                                                                                                                                                                                                                                        |
| `cargo clippy --workspace --all-targets -- -D warnings`      | pass                                                                                                                                                                                                                                                                                        |
| `cargo test --workspace`                                     | 795 passed, 0 failed                                                                                                                                                                                                                                                                        |
| `npx tsc --noEmit`                                           | pass                                                                                                                                                                                                                                                                                        |
| `npx eslint .`                                               | pass                                                                                                                                                                                                                                                                                        |
| `npx prettier --check "src/**/*.{css,html,ts,tsx}"`          | pass                                                                                                                                                                                                                                                                                        |
| `npx vitest run`                                             | 2523 of 2524 passed; the one failure is `scripts/stage14-references.test.mjs` ("checks committed Stage 14 references offline"), which needs the repository's pinned Node version and fails the same way on a clean checkout (it is the known environment difference, not a Stage 16 change) |
| `node scripts/check-types.mjs`                               | generated TypeScript bindings are up to date                                                                                                                                                                                                                                                |
| `node scripts/check-boundaries.mjs`                          | boundaries hold                                                                                                                                                                                                                                                                             |
| `npx vite build`, `node scripts/check-production-bundle.mjs` | pass; the production bundle holds no mock adapter                                                                                                                                                                                                                                           |

The Engine part of #227 has its own test, `crates/engine/tests/stage_16_mirrored.rs`: a file with and
without `mirrored` round-trips, an unmirrored file stays byte-identical, a misspelled or mistyped field is
refused, the flag is accepted on every catalog type, and the Engine result depends on the geometry sent,
not on the flag. The native service of #230 has 16 unit tests and 3 integration tests
(`crates/app/src/recovery.rs`, `crates/app/tests/recovery_session.rs`).

## Candidate CI on three platforms

The CI workflow (`Checks and Tauri build`) runs on `windows-latest`, `ubuntu-latest` and `macos-latest`.
The tests are not part of it (they were taken out of the workflow to keep the matrix fast), so CI shows
that the formatting, lint, type, boundary and generated-types checks pass and that the Tauri application
builds on all three; the tests above were run locally on Windows only.

| Candidate                          | Windows | Ubuntu  | macOS   |
| ---------------------------------- | ------- | ------- | ------- |
| #317 head `f505b676`               | success | success | success |
| #323 head `0534ae33`               | success | success | success |
| #325 head `80240749`               | success | success | success |
| #332 head `5ee13af4`               | success | success | success |
| #333 head `23228929`               | success | success | success |
| #334 head `7b953ead`               | success | success | success |
| #335 head `b422c536`               | success | success | success |
| #336 head `e2ecb3f9`               | success | success | success |
| `main` `530e645f` (the last merge) | success | success | success |

The runs of `main` after each earlier merge also completed with success.

## Native Windows observations

A Windows 11 release build of `main` (`cargo build --release -p open-circuit-app --features
tauri/custom-protocol`), the window at 1280 x 800, the application's data folder
`%LOCALAPPDATA%\org.swofart.opencircuit`. The keys were sent as hardware key presses by scan code
(see the note on the test harness below), to the window of this build only.

| Action                                                                      | Observed result                                                                                                                                                                                                                                                                           |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Start the build                                                             | The workspace showed the Voltage divider with the **Keyboard shortcuts** and **Theme** controls; no recovery offer (nothing was there).                                                                                                                                                   |
| `F1`                                                                        | The **Keyboard shortcuts** dialog opened, listing File, Edit and the other groups with `Ctrl` keys (the native platform is not a Mac). The Close button had the focus.                                                                                                                    |
| `Escape` (twice)                                                            | The first closed the dialog; the second closed the floating parts drawer.                                                                                                                                                                                                                 |
| Click the canvas, `Ctrl+A`                                                  | "3 parts, 3 wires, 1 ground marker selected" appeared in the side column; every object was highlighted.                                                                                                                                                                                   |
| `Down`, `Down`                                                              | The selection moved down by two grid units; the hint changed to "Unsaved changes" and **Undo** became available.                                                                                                                                                                          |
| `Ctrl+D`, `M`, `Shift+R`, `R`                                               | The group was duplicated and the copy became the selection; the new parts were named R3, R4 and G1 by the automatic numbering; the copy was then mirrored, turned a half and then a quarter turn.                                                                                         |
| Wait a few seconds                                                          | `recovery\recovery.json` appeared in the application's data folder, 2,387 bytes, with the envelope fields (`format`, `version`, `writtenAtMs`, `sourcePath` none, `appVersion`) and a six-part schematic. No project file was written.                                                    |
| End the process (a crash), start the build again                            | Before anything else, the dialog **Recover unsaved work?** showed the time written (28 Nov 2026, 00:35) and "a document that was never saved", with **Restore** (focused) and **Discard**.                                                                                                |
| `Escape` on the offer                                                       | The dialog stayed: Escape is not a choice.                                                                                                                                                                                                                                                |
| `Enter` (Restore)                                                           | The six-part document opened as an unsaved document: "Unsaved changes", **Undo** not available (fresh history), the duplicated parts R3, R4 and G1 present. The recovery file was still there (rewritten).                                                                                |
| **New**, then **Discard** in the unsaved-changes dialog                     | A blank "Untitled" document opened and the recovery folder was empty.                                                                                                                                                                                                                     |
| **Open Example**, **Voltage divider**, click the canvas, `Ctrl+A`, `Ctrl+C` | The system clipboard held the `open-circuit-clipboard` JSON text (version 1, 995 characters), read back with `Get-Clipboard`.                                                                                                                                                             |
| `Ctrl+V`                                                                    | **Not verified.** WebView2 showed a permission prompt, "tauri.localhost wants to view text and images copied to the clipboard" (Block / Allow), and no placement started. The prompt was dismissed without granting it. Issue [#337](https://github.com/SWOFART/open-circuit/issues/337). |

## Earlier workflows intact

- The whole suite passes on `main`: 795 Rust tests (including every earlier stage's acceptance and session
  tests) and 2523 interface tests, with the one environment failure named above.
- Natively: **Open Example** listed the six examples and opened the Voltage divider with its circuit drawn;
  **New** with **Discard** gave a blank document; the unsaved-changes dialog still has Save, Discard and
  Cancel; the Pause, Step and speed controls were present and enabled with a document open.
- Saved projects: an unmirrored project file is byte-identical to the earlier format (a Rust test), the schema
  version stays 3, and the Engine's result does not depend on the new flag.

## Decisions and deviations

- **A saved `mirrored` field** (contract section 13, decision 1) was implemented as written: optional, omitted
  when false, schema version 3, on every component variant. It was raised as an owner decision in
  [#332](https://github.com/SWOFART/open-circuit/pull/332), which was merged.
- **Contract amendment in #332:** a part mirrored together with other objects also has its angle negated,
  and the pivot's rounding means a repeated turn is exact only when the center of the bounds is a grid point.
- **`Ctrl/Cmd+F` in a text field** is left to the field, as the contract says; it used to open the search from
  anywhere. The file shortcuts now match the physical key.
- The DTOs of the native recovery commands are typed by hand in TypeScript, as the other `crates/app` DTOs
  are; generation with `ts-rs` covers the Engine crate only.
- Widget-local keys (a field's Enter and Escape, the drawer's list navigation) are not application bindings and
  are not in the shortcut table.

## Not verified, and what follows

- **macOS and Ubuntu:** no native run; CI built the application on both. The Mac key names (`Cmd`) are covered
  by unit tests only.
- **Native, not observed:** the Light theme and the System mode following a change of the operating system's
  appearance; the History panel and the search field with the keyboard; a native Save and Open after a
  restore; autosave being off while an assignment is open; the close-window-with-Discard path. They are
  covered by unit tests and, for the themes, by web screenshots ([stage-16-themes.md](stage-16-themes.md)).
- **Paste** in the native build, because of the permission prompt ([#337](https://github.com/SWOFART/open-circuit/issues/337)).
- **Color-only states** were not audited panel by panel (the contract's accessibility rule).
- **Non-blocking findings** of the pull-request reviews are collected in
  [#338](https://github.com/SWOFART/open-circuit/issues/338), so none is left as hidden work.
- **Test harness note.** Keys sent with `SendKeys` carry no key code, so the letter shortcuts (matched by
  physical key) did not react to them while `F1`, `Escape` and the arrows (matched by the key's value) did;
  hardware-style scan codes reached every shortcut. A keyboard that reports no code would lose the letter
  shortcuts; the earlier shortcuts matched the same way.
