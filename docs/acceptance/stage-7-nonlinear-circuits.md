# Stage 7 nonlinear-circuit acceptance record

**Issue:** [#115](https://github.com/SWOFART/open-circuit/issues/115).

**Recorded:** 2026-11-08.

**Base:** `0d5635b4a3d9d478c219e3b805c386debf080e89` (`main` after #114).

**Contract:** [Stage 7 nonlinear circuits](../contracts/stage-7-nonlinear-circuits.md). **Reference
data:** [Stage 7 references](../solver/stage-7-references.md).

**Status:** local automated checks, the candidate's Windows/Ubuntu/macOS CI and Tauri builds, and a live
Windows desktop walkthrough are recorded below. Independent Gate A for the final update of this record is
the remaining gate. This record does not itself close Stage 7: issue #115 and the Stage 7 parent (#107)
are closed by a human.

## Editor-to-Engine evidence

`src/store/stage7Drawing.test.ts` builds fourteen circuits from an empty document with the
editor's placement, wiring and reference-marker commands, inspector parameter edits, and the real
symbol geometry, and pins them as `tests/golden/stage-7-drawn-*.request.json`. A missing or
different file fails; nothing is regenerated silently. Deliberate regeneration in PowerShell:

```powershell
$env:UPDATE_GOLDEN = '1'
pnpm exec vitest run src/store/stage7Drawing.test.ts
Remove-Item Env:UPDATE_GOLDEN
```

`crates/app/tests/stage7_drawn.rs` runs those exact requests on the real Engine behind
`SimulationCore`, with a frame at every public step. The expected numbers are the answers of the
model equations in `tests/golden/stage-7-analytical.expected.json` (60-digit decimal Newton, no
Engine, no simulator), and the tolerances are the manifest's `engineTolerance`; the file contains
no number of its own. No production mock or dependency was added. The ngspice 47 cross-check of the
same fixtures was made in #109 and is not repeated here.

## Reference results

Measured at public step 5 (direct current) unless stated; `Δ` is measured minus answer. All
quantities are the Engine's, read by layout key; the tests infer no region.

| Drawn fixture | Quantity | Answer of the equations | Δ | Tolerance |
|---|---|---|---|---|
| `diode-forward` (5 V, 1 kΩ) | V | 0.6928878323780558 V | 1.1e-16 | 1e-6 V |
| | I | 4.307112167621944 mA | 2.2e-17 | 1e-6 relative + 1e-12 A |
| `diode-reverse` (−5 V) | V | −4.99999999499 V | −8.9e-16 | 1e-6 V |
| | I | −5.00999999499 pA | 0 | 1e-14 A |
| `npn-cutoff` (base 0 V) | V_BE / V_CE | 4.3042962868e-6 V / 9.99999998998 V | 0 / 1.8e-15 | 1e-6 V |
| | i_C / i_B | 1.0019997e-11 A / −1.0009991e-11 A | 1.6e-27 | 1e-13 A |
| `npn-active` (5 V, 430 kΩ) | V_BE / V_CE | 0.6553851180 V / 8.9896236688 V | 1.1e-16 / −1.8e-15 | 1e-6 V |
| | i_C / i_B / i_E | 1.01037633 mA / 10.1037555 µA / −1.02048009 mA | 3.7e-18 / 3.7e-20 / −3.7e-18 | 1e-6 relative + 1e-12 A |
| `npn-saturation` (5 V, 4.3 kΩ) | V_BE / V_CE | 0.7187142909 V / 66.9395638 mV | 0 / 2.8e-17 | 1e-6 V |
| | i_C / i_B | 9.93306044 mA / 995.647839 µA | 1.2e-17 / 1.1e-18 | 1e-6 relative + 1e-12 A |
| `npn-reverse-active` (collector and emitter exchanged) | V_BE / V_CE | −9.45265256 V / −9.98962084 V (measured from the load node) | 0 / −1.8e-15 | 1e-6 V |
| | i_C / i_B / i_E | −20.7583055 µA / 10.3791435 µA / 10.3791619 µA | −1.4e-18 / 7e-19 / 7e-19 | 1e-6 relative + 1e-12 A |
| `pnp-cutoff`, `pnp-active`, `pnp-saturation` | all five | the NPN values with every sign changed | ≤ 1.8e-15 V, ≤ 5.4e-17 A | as the NPN rows |
| `rc-diode-charging` (switch closes after step 4) | C at 54 / 104 / 204 ms | 4.43286502 / 4.52489018 / 4.56192038 V | 4.73e-3 / 7.1e-4 / 2.1e-4 V | 5e-3 V |
| `rc-diode-reversed` (anode on the capacitor) | C at 54 / 104 / 204 ms | 0.2505 / 0.5010 / 1.0020 µV | 5.0e-9 V at each time | 1e-5 V |

The transient is observed at the contract's exact times: the switch is closed with
`set_operator_state` when the newest public step is 4, and the capacitor voltage is read in the
frames of steps 54, 104 and 204. The charging deviation at 54 ms (4.73 mV of the 5 mV allowed) is
the 1 ms alignment of the Engine's step with the reference's switching instant, as in the Engine's
own test; it is a documented tolerance, not a loosened one.

Distinguishing the wrong variants: the exchanged-pin NPN carries a reverse collector current
(−20.8 µA against +1.01 mA, a factor of about 50 smaller, opposite sign) and a V_BE that differs by
more than 0.1 V from the correct stage; the reversed-diode RC ends at about 1 µV where the correct one
reaches 4.56 V. The bias regions differ in what the Engine reports: V_CE > 9.99 V and
|i_C| < 1 nA (cutoff), 5 V < V_CE < 9.5 V and i_C/i_B = 100 within 1 (active), V_CE < 0.3 V and
i_C/i_B < 20 (saturation). The PNP rows equal the NPN rows with every sign changed.

## Determinism

Two independent runs of the same events (new `SimulationCore`, new compile) report identical
floating-point bits and discrete states at every step, for both transient fixtures through the
switch event (80 steps after closing) and for the diode, the NPN active stage and the PNP saturated
stage (20 steps). Run IDs and transport sequences are not compared. This is a same-environment
guarantee. The contract promises no bit identity across CPU architectures or operating systems, and
the tests across platforms (CI) hold the results to the tolerances above, never to stored bits.

## Live edits, operators and rollback

- **Parameter change during a run.** Changing the 1 kΩ resistor of `diode-forward` to 2 kΩ at step 5
  returns a snapshot of step 5; the history read shows steps 0–4 unchanged and step 5 replaced. The
  recomputed step equals a fresh compile of the edited drawing within 1e-9 relative.
- **Capacitor and Stage 6 switch.** In `rc-diode-charging`, changing the resistor 30 steps after the
  switch closed keeps the capacitor charge (no jump larger than 50 mV, nothing larger is expected
  from a changed resistor) and the switch position (the capacitor keeps charging).
- **Inductor.** `diode-inductor` (5 V, 1 kΩ, 10 H, the default diode, from zero current): the current
  rises with no step larger than 1 mA per public step, and the diode carries the inductor's current at every
  step (1e-9 relative). Changing the inductance to 5 H at step 5 leaves the current continuous (within
  1.5 mA across the edited step, against 0.9 mA of natural change), and after 400 further steps the
  diode's voltage and current equal the `diode-forward` direct-current answer within its tolerance.
- **No state is committed by a failed Newton iterate.** An edit that makes the solve leave the model
  range (the 1 kΩ resistor of the diode loop, or the 430 kΩ base resistor, set to 1e-21 Ω) is refused
  with `ModelRangeExceeded`. The run then produces, over the next 50 steps, exactly the values and
  states (bit for bit) of an untouched twin, and the next valid edit is accepted. The same is checked
  on the running switched capacitor circuit (refused with a contract code), where the capacitor's
  history is part of what must stay identical.

## Failures and diagnostics

The `diode-ideal-source` and `npn-ideal-source` drawings (an ideal source across a forward junction,
and across the base-emitter junction) fail to compile with `ModelRangeExceeded`, localization key
`diagnostic.modelRangeExceeded`, and a typed reference to the diode or the transistor, as JSON the
interface receives. A failed compile leaves the core able to compile the corrected drawing at once.
Revision, run and layout staleness, refused out-of-range edits, a stopped run's notice, and the
responsiveness of the service thread under a ten-transistor workload are tested on the real Engine in
`crates/app/tests/stage7_session.rs` (#113); the interface's handling (a failed compile with no
result, a refused edit, a stopped run, values withdrawn during recompilation, old in-flight replies,
and a late stop notice) is tested in `src/store/stage7Session.test.ts`, `src/store/runStopped.test.tsx`
and `src/ui/stage7Panels.test.tsx` (#113, #114).

## Save and reopen

The drawn schematic of the transient, the NPN and the diode fixtures is saved after the circuit ran
for 20 steps and read back: the schematic (parameters, positions, wires, reference markers) is equal
to the drawn one, and the file contains no solver, iteration, region, operating-point, snapshot, trace,
run or result word. Compiling the reopened document with the same geometry starts a new run at step 0
and reports, at every step, the bits of the first run: results are recalculated, not restored.
`stage7_session.rs` additionally refuses unknown types, unknown fields, out-of-range values and missing
parameters on open, and reopens every earlier-stage golden file unchanged.

## Checks on Windows

| Command (this repository has no `pnpm` on the recording machine; the same tools run from `node_modules/.bin`) | Result |
|---|---|
| `cargo fmt --all --check`; prettier over the `format:check` globs | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings`; `eslint .` | Pass |
| `tsc --noEmit` | Pass |
| `cargo test --workspace` | Pass: 436 tests (11 in `stage7_drawn.rs`) |
| `vitest run` | Pass: 71 files, 1274 tests |
| `node scripts/check-types.mjs` | Pass: generated bindings unchanged |
| `node scripts/check-boundaries.mjs` | Pass |
| `vite build`; `node scripts/check-production-bundle.mjs` | Pass: seven production scripts, no mock adapter |
| Windows Tauri release build | Not run when the checks above were recorded; built later (`tauri build --no-bundle` after `vite build`) for the desktop walkthrough, and part of the candidate's CI |

Existing non-fatal messages: ts-rs does not parse `deny_unknown_fields`; Vite reports a bundle chunk over
500 kB. No lint, compiler or CI setting was weakened.

## Candidate CI

The candidate (the commit of pull request #126, tree `b5ad905e0539eb40e17911166ec4f9ed12a80158`) passed
[CI run 42520866999](https://github.com/SWOFART/open-circuit/actions/runs/42520866999) on Windows, Ubuntu
and macOS, including the Tauri builds. The merge commit on `main` (`96be206`) passed
[CI run 42522603856](https://github.com/SWOFART/open-circuit/actions/runs/42522603856) on the same three
platforms. Results are held to the tolerances above on all three; no stored bit pattern is compared across
platforms.

## Live Windows desktop

Recorded 2026-11-13 by the agent controlling the native Open Circuit window with the computer-use plugin,
on `main` at `fa3e032` (which includes this stage and later merged work). The release executable was built
with `tauri build --no-bundle` after `vite build`, and started from `target/release`. The input projects
were extracted from the pinned editor-drawn requests into ignored `target/tmp/stage7-live/*.ocircuit` and
opened through the native Open dialog; the canvas, inspector, panels, oscilloscope and numbers are the real
Tauri and Engine connection. The automated sections above establish the exact millisecond timelines; the
desktop observations use human-scale times.

| Check | Observed result |
|---|---|
| Palette | The toolbar offers Diode, NPN transistor and PNP transistor beside the earlier parts. |
| Diode forward (5 V, 1 kΩ) | The symbol is a triangle from anode to cathode with the bar at the cathode. D1 voltage 692.8878 mV and current 4.307112 mA, equal to the reference 0.6928878 V and 4.307112 mA. The Devices panel reads "Voltage (Anode − Cathode)" and "Current (Into The Anode)" with the same numbers. The oscilloscope offers D1 voltage and current, and tracing D1 voltage draws a flat line at about 693 mV. |
| Inspector, diode | "D1 saturation current" shows 10 fA with "Allowed range: 0.001 fA to 1 nA." and the emission coefficient shows 1 with "Allowed range: 1 to 2." Typing 2 nA shows "That value is outside the range the circuit model allows: 0.001 fA to 1 nA." and is not applied (the readings stay 692.8878 mV). Typing 20 fA is applied: D1 675.0664 mV, 4.324934 mA, as the diode law gives. |
| NPN active (10 V, 5 V, 430 kΩ) | The NPN symbol has its emitter arrow pointing out of the base. T1 base-emitter 655.3851 mV, collector-emitter 8.989624 V, collector 1.010376 mA, base 10.10376 µA, emitter −1.02048 mA, all equal to the reference. The Devices panel names each quantity by its terminals and shows no operating region. |
| NPN with collector and emitter exchanged | The transistor is drawn turned half a turn. The load R1 carries 10.37916 mV instead of 1.010 V, the reference's reverse-active answer, so the wrong wiring is visibly different from the correct stage. |
| PNP active | The PNP arrow points toward the base. The load voltage is −1.010376 V, the NPN's with the sign changed. |
| Switched nonlinear RC | With S1 open the circuit is idle. Clicking the lever closes S1 (Closed, Conducting Yes); C1 charges through R1 and D1 to 4.6585 V while the diode current falls to 5.4 nA and its voltage to 341 mV. The diode, capacitor and switch readings agree with each other (the same current in S1, R1, D1 and C1). |
| Compile failure | Opening the ideal-source-across-diode drawing shows, in the Results panel, "The engine could not calculate this circuit. There is no result, and nothing below is a reading of it." and "ERROR A junction exceeds the supported model range. (D1)", with D1 highlighted. The Devices panel and the oscilloscope say that there is no active run. The document stays editable: setting V1 to 0.5 V gives a result (D1 500 mV, 2.485608 µA) without reopening. |
| Save and reopen | Saving the diode with 20 fA wrote a file holding only the version, components, wires, junctions, reference markers and rails, and no computed word. Reopening it shows 20 fA and 675.0664 mV, and the simulation time restarts from zero: the result is recalculated. The unsaved-changes prompt appeared before Open and Discard returned to a clean state. |

Not covered by the desktop walkthrough and not claimed: a stop notice from an advancing run (it has no short
deterministic example; the automated test uses an overflowing circuit, see PR #124), keyboard-only
editing and screen-reader names (covered by the interface tests, not audited live), and the macOS and Linux
desktops (only their CI builds and tests). Two small presentation points were seen and are not defects of
this stage: the device panel capitalizes "Into The Anode" in each word, as it does for the earlier devices'
labels, and the readings are not annotated with their sign convention beyond the row names.

## Remaining

- **Independent Gate A** for the change that adds this section (AGENTS.md).
- **Stage closure.** All acceptance items of #115 now have recorded evidence. #115 and the Stage 7 parent
  (#107) are closed by a human after reading this record.
