# Stage 4 components acceptance record

**Issue:** [#69 — Stage 4: Verify switching, protection, motor, and measurement workflows end to end](https://github.com/SWOFART/open-circuit/issues/69)
**Recorded:** 2026-10-27.
**Scope:** The SPDT relay, I²t fuse, PMDC motor, voltmeter, and ammeter, on the real Rust Engine and
through the application's bridge formats. Contract:
[stage-4-components.md](../contracts/stage-4-components.md). Fixtures: `tests/golden/stage-4-*.expected.json`.

**Status:** the automated part of this record passes. The live desktop check was **not performed**
by the agent that prepared this record; see "Live desktop check" below. The issue should stay open
until a person has run it and added the result, and the pull request's own CI run is linked.

## Circuits and expected values

Each circuit is drawn from a blank document with the editor's own commands
(`src/store/stage4Drawing.test.ts`) and pinned as `tests/golden/stage-4-drawn-*.request.json`.

| Fixture | Circuit as drawn | Expected | Tolerance |
|---|---|---|---|
| Self-latching relay | V1 ideal 5 V; K1 with the catalog defaults (coil 100 Ω and 10 mH, pickup 40 mA, dropout 20 mA, contact 0.1 Ω); R1 1 Ω start path from the coil's return to ground; K1's normally open contact from the coil's return to ground; the normally closed contact unconnected | Picked at time zero; at 5 ms picked with coil current 49.95005 mA; released by 7 ms with coil current 0 | 0.1 mA |
| I²t fuse | V1 ideal 2.02 V; F1 0.01 Ω with trip threshold 0.004 A²·s; R1 1 Ω in series | Intact at 2 A at time zero; tripped at 1 ms with accumulated I²t 0.004 A²·s; after that fuse current 0 A, 2.02 V across the fuse | 1e-12 s, 1e-12 A²·s, 1e-12 A, 1e-9 V |
| PMDC motor | V1 ideal 10 V; M1 with R 2 Ω, L 0.1 H, k 0.1 N·m/A, J 0.01 kg·m², b 0.01 N·m·s/rad, load torque 0, explicit initial speed 0 and current 0 | At 8 s: 33.33349 rad/s and 3.333325 A (closed form of the coupled equations) | 0.01 rad/s, 1 mA |
| Voltmeter loading | V1 10 V behind 1 MΩ; VM1 10 MΩ across it | Reads 9.090909 V and draws 0.909091 µA (an ideal meter would read 10 V) | 1 nV, 1 pA |
| Ammeter loading | V1 ideal 10 V; R1 10 Ω; AM1 1 Ω in series | Reads 0.909091 A with 0.909091 V across it (an ideal meter would read 1 A) | 1 nA, 1 nV |

## Automated evidence

| Acceptance item | Evidence | Result |
|---|---|---|
| Self-latching relay: deterministic pickup, hold, and release | `crates/app/tests/stage4_session.rs`: the real Engine through `SimulationCore`, a frame at every public step, the fixture's two live updates. The state of every step to 10 ms is checked, the whole timeline is identical on a second run (including the bits of the coil current), and a new compile starts a fresh picked run whatever happened before. `src/store/stage4EndToEnd.test.tsx` replays the recorded OCF2 frames and update replies through the adapter, store, and Devices panel: Picked at 0 s, held at 5 ms with 49.95 mA, Released afterwards. | Pass |
| Voltmeter and ammeter: values, polarity, units, loading | `stage4_session.rs`: both circuits against the fixture, and each wired the other way round (the two pins swapped in the geometry) reads the negative of the same value, within the fixture's tolerance. Loading is asserted against the ideal value. The replay shows the readings with their units (V, nA, mA, mV). | Pass |
| Fuse trips at the agreed condition and opens the intended path, then after reset | `stage4_session.rs`: intact with 2 A at time zero; tripped at 1 ms with I²t 0.004 A²·s; from then on zero current in the fuse and the load, 2.02 V across the fuse, still tripped (one-shot); a new compile gives a new run, intact again, and it trips again at the same time. The replay shows Intact, Tripped (fault badge, "since 1 ms"), the open path in the result list, and Intact after the document changes. | Pass |
| Motor startup and a load case against the known-answer reference | `stage4_session.rs`: step 0 is rest; at 10, 100, 500, 1000, 2000, 4000, and 8000 ms the Engine's speed and current are compared with the closed-form solution (computed in the test from `x' = A x + c`), and at 8 s with the fixture's numbers. A second case, a −0.05 N·m shaft torque, reaches the analytical operating point 30 rad/s and 3.5 A. The trace carries the frame's value to the bit. | Pass |
| UI readouts are Engine-produced and do not diverge after run or document changes | `stage4EndToEnd.test.tsx`: a frame from before a live update is refused after it; after a structural edit the panel shows the new run's own initial state, not the previous run's released relay or tripped fuse, and late frames of the old run are refused; readings and states come from the same frame. | Pass |
| Oscilloscope traces of the new signals | `stage4EndToEnd.test.tsx` decodes the real Engine's OCT1 responses for the fuse's accumulated I²t (`ampere_squared_second`) and the motor's angular velocity (`radian_per_second`) and draws their lanes. This is the path that failed before the trace decoder accepted every unit (#68). | Pass |
| Diagnostics | `stage4_session.rs`: a relay whose dropout current is not below its pickup current is refused as `InvalidParameter`, both as a live update (the run goes on) and as a compile. | Pass |
| Rust, TypeScript, contract, generated-type, and architecture-boundary checks | See "Checks run on Windows". | Pass |

## Findings

- **The motor fixture's speed is 3.2e-4 rad/s too high.** `stage-4-pmdc-motor.expected.json` gives
  33.333492588830005 rad/s at 8 s. The closed form of the same equations, computed in
  `stage4_session.rs`, gives 33.33317 rad/s: the speed approaches the steady 33.33333 rad/s from below
  (slow mode `e^(−1.54 t)`, about 4.4e-6 at 8 s), so a value above the steady state cannot be right. The
  armature current in the fixture agrees. The Engine matches the closed form, and both are far inside
  the fixture's 0.01 rad/s tolerance, so nothing fails, but the golden value should be corrected by
  its owners. This record does not change an accepted value.
- **The relay fixture's "at 6 ms" is the newest step when the update is sent.** Contract
  [stage-3-dynamics.md](../contracts/stage-3-dynamics.md) section 3.4 says a live update takes effect
  from the start of the newest step, so removing the supply when the newest step is 6 removes it from
  5 ms and the relay lets go at about 5.1 ms. The fixture's expectations (picked at 5 ms, released by
  7 ms) hold; the test asserts the release lies between 5 ms and 7 ms and that the replaced sample of
  step 6 already shows it.

## Checks run on Windows

Collected on 2026-10-27 on the candidate of the pull request that adds this record.

| Command | Result |
|---|---|
| `prettier --check` (the `format:check` globs) and `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` and `eslint .` | Pass |
| `tsc --noEmit` | Pass |
| `cargo test --workspace` | Pass, including `stage4_session` and the Engine's `stage_4_*` tests |
| `vitest run` | Pass; 47 files, 920 tests |
| `vite build`, then `scripts/check-production-bundle.mjs` | Pass; the bundle has no mock adapter |
| `scripts/check-types.mjs`, `scripts/check-boundaries.mjs`, `git diff --check` | Pass |

`pnpm build` (the Tauri release build) and the macOS and Linux runs were not run locally. The pipeline
runs them for every pull request on Windows, Ubuntu, and macOS (`.github/workflows/ci.yml`). The
implementation this record verifies, `main` at `7ab7b00` (merge of #76), passed all three jobs in
[GitHub Actions run 40542308130](https://github.com/SWOFART/open-circuit/actions/runs/40542308130).
The run for the pull request that adds this record is linked from that pull request.

## Live desktop check

**Not performed.** The agent that prepared this record cannot drive the desktop window (an earlier
attempt to do so through a temporary local script server was refused, and nothing from it was kept).
No result below is claimed.

A person should run the following in the desktop app (`pnpm dev`) and record the readings in this
section, then close the issue:

1. **New**. Draw the relay circuit: a 5 V source with 0 Ω internal resistance, the SPDT relay with its
   defaults, a 1 Ω resistor from the coil's return to ground, the relay's normally open pin to ground,
   and a ground marker. Expect **Picked** in the Devices panel at once, with the coil current near
   49.95 mA.
2. Raise the 1 Ω resistor to 1 GΩ: the relay stays **Picked**. Set the source to 0 V: the relay becomes
   **Released** and its last change time is shown.
3. Draw the fuse circuit (source 2.02 V, fuse, 1 Ω load, ground). Expect **Intact** and 2 A, then
   **Tripped** within a moment, with 0 A in the load and 2.02 V across the fuse. Change any value: a
   new run starts and the fuse is **Intact** again.
4. Draw the motor circuit (source 10 V, motor with its defaults, ground, and 0 for both initial
   conditions). Expect the angular velocity to rise to about 33.3 rad/s over several seconds and the
   current to peak and settle near 3.33 A. Select the motor's angular velocity in the oscilloscope and
   choose the 10 s window.
5. Voltmeter: 10 V source with 1 MΩ internal resistance and the voltmeter across it should read about
   9.09 V. Ammeter: 10 V source, 10 Ω resistor, ammeter should read about 909 mA. Turn either meter
   around: the sign flips.
6. Place a relay and set its dropout current above its pickup current: the Results panel reports a
   parameter error naming the relay, and the drawing stays editable.

| Manual check | Observed result | Result |
|---|---|---|
| Steps 1 to 6 above | Not yet recorded | Pending |
