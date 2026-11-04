# Stage 6 control-circuit acceptance record

**Issue:** [#100](https://github.com/SWOFART/open-circuit/issues/100).

**Recorded:** 2026-11-04.

**Base:** `b9719c486da451605e3888d59ecc8db89a07e6fc`.

**Contract:** [Stage 6 control circuits](../contracts/stage-6-control-circuits.md).

**Status:** local automated checks and Windows desktop verification are recorded below.
Independent Gate A and the candidate's Windows/Linux/macOS CI remain pending. This record
does not declare Stage 6 accepted and issue #100 must remain open until those gates pass.

## Editor-to-Engine evidence

`src/store/stage6Drawing.test.ts` builds four circuits from an empty document using the
editor's placement, wiring and ground commands, inspector parameter edits, and real symbol
geometry. It pins `tests/golden/stage-6-drawn-*.request.json`; missing files fail rather than
being silently regenerated. Deliberate regeneration in PowerShell:

```powershell
$env:UPDATE_GOLDEN = '1'
pnpm exec vitest run src/store/stage6Drawing.test.ts
Remove-Item Env:UPDATE_GOLDEN
```

`crates/app/tests/stage6_acceptance.rs` runs those exact requests on the real Engine behind
`SimulationCore`. The canonical expected files are analytical references, not recorded
Engine output. Both executions of each complete timeline must have identical floating-point
bits and discrete states, excluding run IDs and transport sequences.

| Drawn fixture | Operator events (newest public step) | Known answer and tolerance |
|---|---|---|
| `switch` | Close at 3, open at 6 | Ideal 10 V, 0.01 Ω contact, two 1 kΩ resistors: open 0 V/0 A; closed 4.999975000125 V and 0.004999975000125 A. Closed voltage tolerance 1e-9 V, current 1e-12 A; zero voltage 1e-12 V. All rows of `stage-6-switch-divider.expected.json` checked. |
| `button` | Press NC at 50, release at 100 | Ideal 10 V, 0.01 Ω contact, 1 kΩ, 100 µF from 0 V: τ = 0.100001 s; 3.873706039689 V at 49 ms and held through 99 ms; 3.934663076568 V at 100 ms, 7.768664929075 V at 200 ms. Tolerance 5 mV (initial zero 1e-9 V); open-contact current 1e-12 A. All rows of `stage-6-button-capacitor-hold.expected.json` checked. |
| `latching` | START press/release 5/10, STOP press/release 15/20 | Ideal 5 V, default contactor, NC STOP and NO START: initially released/0 A; picked with 49.9904563674 mA while START is held; still picked with 49.9450604335 mA after START release; released/0 A after STOP and still off afterwards. Current tolerance 0.1 mA. All rows of `stage-6-start-stop-latching.expected.json` checked. |
| `unlatched` | Same START/STOP events | Same circuit with neither NO terminal wired: picked/49.9900019996 mA at step 5, released/0 A at step 12. Same 0.1 mA tolerance; distinguished from the correct circuit. |

The Engine applies an event by recomputing the newest 1 ms sample, from the start of that
step. The history reader is checked against the resulting values at every step, including
each replaced sample. Earlier samples remain unchanged. The tests then activate each
device again before saving and recompiling: persistence/reset is tested from a changed
position, including a picked contactor, rather than from an already idle run.

Project write/read must return exactly the drawn version 1 schematic, with no operator
position, calculated state, trace or run metadata. Translating the complete drawing one
grid unit starts a new run with the same initial states and values. An operator request
for the replaced run returns `StaleRun`. The existing `stage6_session.rs` additionally
covers `initialClosed`, invalid requests, stale revisions and the simulation-thread path.

The Rust acceptance test writes a local recording to `target/tmp/stage6-session.json`.
`src/store/stage6EndToEnd.test.tsx` replays it through the production Tauri adapter, binary
decoder, simulation controller and Devices panel. Only the Tauri transport is replaced;
every outgoing compile/operator request is matched to its recording. It checks the
reported positions and values, accessible controls, no document/history/dirty-state
changes, rejection of old-revision and replaced-run frames, and reset after the drawing
edit. A deferred compile also verifies that old values/states disappear while the new
document is still being compiled, including a drawing edit waiting for an in-flight
operator reply: that reply cannot restore the old run. CI requires the recording;
`pnpm test` creates it by running Rust before Vitest. No production mock or dependency was added.

## Integration fixes found during desktop verification

- While a changed document compiled, the controller retained the old run and readings,
  which the panels labelled with the new drawing's component names. Compilation now
  withdraws the run/snapshot and stops its stream before waiting for the reply. The
  deferred-compile replay is the regression test; lightweight operator changes retain
  their existing acknowledgement and refusal behavior.
- On the canonical contactor circuit, the debug application's catch-up batch could run
  up to 1000 stiff public steps without receiving operator commands. START timed out and
  its release was delayed. Catch-up now yields between completed public steps after an
  8 ms computation budget (or the existing step-count cap), dropping excess wall-clock
  debt as the previous cap did. A single step may exceed that budget; it is never split
  or interrupted. Physics remains fixed at 1 ms with the Engine's existing internal
  substeps and event rules, and frames remain independently capped at 60 Hz. A regression
  test gives the drawn contactor a one-second scheduling stall and checks that it yields
  before a thousand steps, then accepts press/release. Existing cadence, backpressure,
  lifecycle and real-thread tests also pass. A slow machine may run below real time.

These fixes change scheduling/presentation only; models, fixture values, public DTOs,
project and binary formats are unchanged.

## Checks on Windows

| Command | Result |
|---|---|
| `pnpm format:check` | Pass |
| `pnpm lint` | Pass: workspace Clippy with warnings denied, ESLint |
| `pnpm typecheck` | Pass |
| `pnpm test` | Pass: workspace Rust tests and 66 Vitest files / 1196 tests |
| `pnpm types:check` | Pass: generated bindings unchanged |
| `pnpm boundaries:check` | Pass |
| `pnpm build` | Pass: Windows Tauri release executable |
| `pnpm bundle:check` | Pass: seven production scripts, no mock adapter |
| `git diff --check`; unchanged lockfiles and generated types | Pass |

Existing non-fatal build messages: ts-rs does not parse `deny_unknown_fields`; Vite reports
a bundle chunk over 500 kB. No lint/compiler/CI setting was weakened.

## Live Windows desktop

The agent controlled the native Open Circuit window with the computer-use plugin.
The input projects were extracted from the editor-drawn requests into ignored
`target/tmp/stage6-live/*.ocircuit`, then opened through the native Open dialog. The
canvas rendering, operator gestures and displayed numbers used the real Tauri/Engine
connection. Automated runs above establish the exact millisecond timelines; desktop
observations use human-scale press/release times instead.

| Check | Observed result |
|---|---|
| Switch divider | Open: 0 V, 0 A. Clicking the lever: Closed/Conducting Yes, R2 4.999975 V, 4.999975 mA; the saved initial-position inspector remains Open and the project stays clean. |
| Correct START/STOP, rebuilt release executable | Initially Q1 Released, coil 0 A. Click START: START returns Released, Q1 stays Picked, coil and NO-contact current 49.94506 mA. Click STOP: Q1 Released, coil 0 A; STOP returns Released and the circuit stays off. No timeout in this corrected release run. |
| Without seal-in contact | Click START: after release S2 is Released, Q1 is Released, coil current 0 A, and the recorded contactor last-change time confirms a transition. Unlike the correct circuit it does not stay picked. |
| NC button and capacitor | At open C1 is 0 V and 9.9999 mA; later C1 is 10 V and its current 0 A. Click NC actuator: Pressed/Conducting No, branch and capacitor currents 0 A while C1 remains 10 V; after release the button returns Released/Conducting Yes. The precise 50/100-step hold at a partial charge is verified by the automated fixture, not claimed as a live timed measurement. |
| Native Save after operation | Save of the operated `unlatched` project matches the original drawn schematic by deep JSON equality; no position or last-transition/result metadata is written. |

The native canvas was fitted to show the complete contactor, its isolated contacts and
the seal-in wiring. Full pointer/keyboard cancellation and refusal coverage is automated
in the existing Stage 6 interaction, gesture, panel, controller and bridge tests; it is
not claimed as a separate live keyboard accessibility audit.

## Cross-platform CI and remaining gate

The base `main` revision passed [CI run 41722344248](https://github.com/SWOFART/open-circuit/actions/runs/41722344248).
That is baseline evidence only. No CI run for this uncommitted candidate exists yet.
After independent Gate A passes for the exact reviewed tree, create a draft PR, wait for
its Windows/Ubuntu/macOS checks and Tauri builds, and add that run's link/results here.
Any subsequent candidate change needs a fresh Gate A result under `AGENTS.md`.
