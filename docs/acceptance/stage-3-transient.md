# Stage 3 transient workflow acceptance record

**Issue:** [#50 — Verify the Stage 3 transient workflow end to end](https://github.com/SWOFART/open-circuit/issues/50)
**Recorded:** 2026-10-26.
**Scope:** Capacitors, inductors, explicit initial conditions, and the oscilloscope, on the real
Rust Engine. Contract: [stage-3-dynamics.md](../contracts/stage-3-dynamics.md). Fixtures and
tolerances: [models.md](../physics/models.md), "Stage 3 acceptance".

**Status:** the automated part of this record passes. The live desktop check was **not
performed** by the agent that prepared this record; see "Live desktop check" below. The issue
should stay open until a person has run it and added the result.

## What is checked, and against what

| Fixture | Circuit | Quantity | Reference | Tolerance |
|---|---|---|---|---|
| RC charge | V1 10 V ideal, R1 1 kΩ, C1 10 µF, C1 starts at 0 V | C1 voltage | `10 (1 − e^(−t/10 ms))` | 10 mV (0.1% of 10 V) |
| RC discharge | C1 10 µF and R1 1 kΩ, C1 starts at 10 V | C1 voltage | `10 e^(−t/10 ms)` | 10 mV |
| RL rise | V1 10 V ideal, R1 10 Ω, L1 100 mH, L1 starts at 0 A | L1 current | `1 − e^(−t/10 ms)` | 1 mA (0.1% of 1 A) |
| RL decay | R1 10 Ω and L1 100 mH, L1 starts at 1 A | L1 current | `e^(−t/10 ms)` | 1 mA |
| DC initialization | V1 5 V, R1 1 kΩ, L1 1 H, no initial current; and the RC charge circuit with no initial voltage | L1 current and voltage; C1 voltage and current | 5 mA and 0 V; 10 V and 0 A, held for 1000 steps | 1 nA, 1 µV |
| Lossless LC | C1 100 µF at 10 V and L1 100 mH at 0 A | stored energy; period | 5 mJ; 19.8691765 ms | relative `1e-6`; 0.25% |

Values the oscilloscope shows for the RC charge circuit at the times the issue names (from
`tests/golden/stage-3-rc-charge.expected.json`; the accepted error is 10 mV):

| Time | C1 voltage, analytical | C1 voltage, the Engine's trace |
|---:|---:|---|
| 10 ms | 6.321205588 V | within 10 mV, checked in `stage3_session.rs` and `stage3EndToEnd.test.tsx` |
| 20 ms | 8.646647168 V | within 10 mV |
| 30 ms | 9.502129317 V | within 10 mV |

The trapezoidal rule at the 1 ms step gives 6.324275 V, 8.648904 V, and 9.503374 V at those
times (models.md), at most 3.07 mV from the analytical curve.

## Automated evidence

| Acceptance item | Evidence | Result |
|---|---|---|
| Draw the RC charge circuit from a blank document with the editor | `src/store/stage3Drawing.test.ts` starts from `blankSchematic` and draws a 10 V ideal source, a 1 kΩ resistor, a 10 µF capacitor with an explicit 0 V start, three wires, and a ground marker, using the editor commands the canvas issues (`placeComponent`, `addWire`, `addReferenceMarker`) and the inspector's parameter edits. It checks that every pin ends exactly one wire, that no wire passes through a foreign pin, and that the resulting compile request equals `tests/golden/stage-3-drawn-rc.request.json`. Pointer gestures themselves are covered by the editor interaction tests. | Pass |
| Run it on the real Engine and read the oscilloscope values at 10, 20, and 30 ms | `crates/app/tests/stage3_session.rs` compiles that same request file with the real Engine through `SimulationCore`, runs it against a fake clock, reads C1's voltage and current through the real trace encoder, and compares every public step to 50 ms and the 10, 20, and 30 ms table with the tolerance above. `src/store/stage3EndToEnd.test.tsx` replays the recorded exchange through the TypeScript adapter, decoder, `OscilloscopeController`, and the oscilloscope panel, and checks the same values on what the panel draws. | Pass |
| RC discharge, RL rise and decay, DC initialization, LC period and energy | `crates/app/tests/stage3_fixtures.rs` runs the documented fixture files on the real Engine and checks them through the bridge's trace path (ring of samples, `OCT1` encoder, bounded history reads). The Engine's own `crates/engine/tests/stage_3_capacitor.rs` and `stage_3_inductor.rs` check the same fixtures against ngspice as well. A wrong time constant makes `stage3_fixtures.rs` fail (checked by hand). | Pass |
| Explicit initial conditions and the diagnostic for an incompatible combined state | The editor draws a second capacitor on the first one's pins with an explicit 5 V against the first one's 0 V (`stage3Drawing.test.ts`, `tests/golden/stage-3-drawn-rc-conflict.request.json`). The real Engine refuses it with `InconsistentState` that references both capacitors (`stage3_session.rs`). The replay shows the diagnostic in the result panel, says there is no active run on the oscilloscope, and leaves the drawing unchanged and not dirty (`stage3EndToEnd.test.tsx`). Setting and clearing an initial condition, and the Engine's refusal to change one as a live parameter, are covered by `src/store/capacitorInductor.test.ts` and `crates/engine/tests/stage_3_*.rs`. | Pass |
| Save and reopen with initial conditions | `stage3EndToEnd.test.tsx` saves the drawn circuit through `ProjectWorkflow`, checks that the file equals the schematic with its explicit `initialVoltageVolts`, and that no run, layout, snapshot, trace, history, window, or step appears in it. The reopened document is equal and clean, and compiling it starts a new run with an empty oscilloscope selection and no data. The real Rust file path is checked in `crates/app/src/project.rs`: every Stage 3 golden schematic (RC charge and discharge, RL rise and decay, lossless LC, DC inductor) is written and read back equal and without computed state; an explicit 0 V or 0 A is kept; an unset initial condition is absent, not `null`; and a `null` or non-numeric initial condition, a zero or negative capacitance, and an inductor's key on a capacitor are refused on open. `src/store/capacitorInductor.test.ts` covers set and cleared conditions in the document. | Pass |
| A refused or queued parameter edit never leaves a trace that looks current | `stage3EndToEnd.test.tsx` edits R1 while the Engine has not answered: the panel says the trace is from the last circuit the Engine accepted. The Engine then refuses the update; the application recompiles, a new run replaces the old one, and no trace of the old run remains. Failed and queued updates of the simulation store are covered by `src/store/simulation.test.ts` and `documentSync.test.ts` (issue #38, closed). | Pass |
| A read in flight when the run is replaced | `src/store/traces.test.ts` answers a pending read after a new run has replaced the old one and shows nothing; `stage3EndToEnd.test.tsx` checks that the recorded samples of the first run are refused for the run that replaced it. | Pass |
| Stage 1 and 2 guarantees | Full Rust and Vitest suites, including `crates/app/tests/bridge_session.rs`, `src/store/endToEnd.test.tsx`, the project workflow tests, and the stale-frame tests. | Pass |
| No mocks in production | `scripts/check-production-bundle.mjs` on the built bundle: no mock adapter or mock trace marker, and the production trace error text is present. The tests above use the real Engine; the only replaced part is the Tauri JavaScript API. | Pass |

## Checks run on Windows

Collected on 2026-10-26 on the candidate of the pull request that adds this record.

| Command | Result |
|---|---|
| `prettier --check` (the `format:check` globs) and `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` and `eslint .` | Pass |
| `tsc --noEmit` | Pass |
| `cargo test --workspace` | Pass, including `trace_contract`, `stage3_session`, and `stage3_fixtures` |
| `vitest run` | Pass; 42 files, 799 tests |
| `vite build`, then `scripts/check-production-bundle.mjs` | Pass; the bundle has no mock adapter |
| `scripts/check-types.mjs`, `scripts/check-boundaries.mjs`, `git diff --check` | Pass |

`pnpm build` (the Tauri release build) and the macOS and Linux runs were not run locally. The
pipeline runs them for every pull request on Windows, Ubuntu, and macOS (`.github/workflows/ci.yml`).
The implementation this record verifies, `main` at `43c0436070bc6b86c6c48e67f28a68094814e9fa`,
passed all three jobs in
[GitHub Actions run 40246386313](https://github.com/SWOFART/open-circuit/actions/runs/40246386313).
The run for the pull request that adds this record is linked from that pull request.

## Live desktop check

**Not performed.** The agent that prepared this record tried to drive the real desktop window
through a temporary local script server and was not allowed to, because that server would
have let any local process run code inside the application. Nothing from that attempt remains
in the repository. No result below is claimed.

A person should run the following in the desktop app (`pnpm dev`) and record the readings in
this section, then close the issue:

1. **New**, then draw the RC charge circuit: a DC voltage source set to 10 V with 0 Ω internal
   resistance, a 1 kΩ resistor, a 10 µF capacitor with **Set C1 initial voltage** checked at
   0 V, three wires closing the loop, and a ground marker on the source's negative pin.
2. In the oscilloscope, tick **C1 voltage** and **C1 current** and choose the 50 ms window.
   Expect a curve rising to about 9.5 V, and a current falling from 10 mA. Record the values
   at 10, 20, and 30 ms (6.32 V, 8.65 V, 9.50 V, within 10 mV).
3. Change R1 to 2 kΩ: the trace continues (a live update keeps the run). Change the capacitor's
   initial voltage: the oscilloscope clears and a new run starts at time zero.
4. Place a second capacitor on the first one's pins with an initial voltage of 5 V: the result
   panel shows an `InconsistentState` error naming C1 and C2, the oscilloscope says there is no
   active run, and the drawing stays editable. Undo removes the conflict.
5. **Save**, close, reopen: the circuit and the 0 V initial voltage are back, and the
   oscilloscope starts empty.

| Manual check | Observed result | Result |
|---|---|---|
| Steps 1 to 5 above | Not yet recorded | Pending |
