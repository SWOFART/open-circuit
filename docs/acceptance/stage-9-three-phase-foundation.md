# Stage 9 three-phase foundation acceptance record

[Issue #171](https://github.com/SWOFART/open-circuit/issues/171) ·
[Stage 9 contract](../contracts/stage-9-three-phase-foundation.md) ·
[Reference methods and provenance](../solver/stage-9-references.md)

Recorded on 2026-11-19. The base is `ad59562596a1deefdb6a5490e71b381839ce363d`
(editor PR #258 on top of bridge PR #257). The candidate is the revision
that adds this record and the six pinned editor requests. The review and
CI identify it by commit and tree. The candidate adds test and acceptance
evidence for both workstreams. The human stage acceptance remains pending.

## Automated editor-to-Engine replay

`src/store/stage9Drawing.test.ts` builds six circuits from blank documents
using the component placement, parameter editing, wire and reference marker
commands. It resolves pins from the editor symbols and checks the exact
compile request against `tests/golden/stage-9-drawn-*.request.json`. A changed
drawing fails the check. To deliberately regenerate the pinned inputs in
PowerShell:

```powershell
$env:UPDATE_GOLDEN = '1'
pnpm exec vitest run src/store/stage9Drawing.test.ts
Remove-Item Env:UPDATE_GOLDEN
```

`crates/app/tests/stage9_acceptance.rs` compiles those requests through the
application's `SimulationCore`, samples OCF2 frames every 1 ms and compares
with both stored oracles. The analytical oracle uses closed-form startup
and sinusoidal responses and an independent complex nodal solve. The
ngspice oracle uses version 47 and the committed netlists. The fixture
manifest records simulator settings, archive hash, signs and tolerance.
The offline oracle check passed all 27 circuits and 1,460 stored
observations, including wrong variants, KCL and power balance.

```text
node scripts/stage9-references.mjs --check
pnpm exec vitest run src/store/stage9Drawing.test.ts
cargo test -p open-circuit-app --test stage9_acceptance -- --nocapture
```

### Capacitor and three-phase samples

The RC case uses 10 V RMS at 50 Hz, 0.1 Ω source resistance, 100 Ω
and 100 µF. The check compares all public-step observations in both
oracles up to step 220. The three-phase cases use 50 Hz and 0.1 Ω source
resistance. Star uses 230 V phase RMS and delta uses 400 V line RMS.
Unbalanced star loads are 100, 220 and 470 Ω. Other loads are 100 Ω.
Three-phase observations span 100–120 ms. The three numbers in each
current row denote the three load branches in the order drawn.

| Circuit and observation | Analytical | Engine on Windows | Largest difference against either oracle | Absolute sample bound |
|---|---:|---:|---:|---:|
| RC capacitor voltage at 5 ms | 3.777084419 V | 3.767814013 V | 0.011792546 V | 0.042856229 V |
| Balanced star with neutral at 105 ms | 3.249441752, −1.624720876, −1.624720876 A | 3.249441752, −1.624720876, −1.624720876 A | 4.171e-8 A | 0.016263456 A |
| Unbalanced star with neutral at 105 ms | 3.249441752, −0.738912129, −0.345957370 A | 3.249441752, −0.738912129, −0.345957370 A | 4.171e-8 A | 0.016263456 A |
| Unbalanced star without neutral at 105 ms | 1.951523898, −1.329196442, −0.622327456 A | 1.951523898, −1.329196442, −0.622327456 A | 2.488e-8 A | 0.016263456 A |
| Balanced delta load on delta source at 105 ms | 5.651203046, −2.825601523, −2.825601523 A | 5.651203046, −2.825601523, −2.825601523 A | 7.114e-8 A | 0.028284271 A |

The sample bounds use the RC start-up tolerance and 0.5% of the peak
current scale for the resistive three-phase samples. The replay also
checks delta line voltages and star load-neutral shift against both
oracles with peak-scaled voltage bounds. It checks source neutral current
against the sum of the star load currents within 1 nA. Removing the
unbalanced star's neutral wire shifts the load star point and reduces
the first branch current from about 3.249 A to 1.952 A at the same step.
The Engine model tests retain the full amplitude, phase, frequency and
wrong-connection checks specified in the contract.

### Wattmeter on the RL load

The drawing uses the 10 V RMS source at 50 Hz with 0.1 Ω internal
resistance, 100 Ω and 100 mH in series, and a wattmeter with 0.1 Ω
burden and 10 MΩ input resistance. The window ends at 300 ms. The
table shows the analytical values and the measured values. The same
bounds apply to the stored ngspice values.

| Quantity | Analytical | Engine on Windows | Absolute bound |
|---|---:|---:|---:|
| RMS voltage | 9.981827854 V | 9.980545585 V | 0.019963656 V |
| RMS current | 0.095230407 A | 0.095217074 A | 0.000190461 A |
| Real power | 0.906874859 W | 0.906620943 W | 0.001813750 W |
| Reactive power | 0.284900009 var | 0.284856848 var | 0.000569800 var |
| Apparent power | 0.950573524 VA | 0.950318345 VA | 0.001901147 VA |
| Power factor | 0.954029158 | 0.954018143 | 0.002 |
| Phase angle | 17.440414492° | 17.442520164° | 0.3° |

## Persistence, determinism and failure paths

- The real project session saves and reopens every pinned drawing. The
  serialized value equals the drawn input. The new run starts with a fresh
  identity and has identical value bits at every public step in the same
  environment. The selected OCT1 trace also agrees sample for sample.
- The wattmeter is invalid at step 1 and valid after the window fills.
  A 500 Hz live edit returns `InvalidParameter` and preserves the
  previous trace. A successful 60 Hz source edit invalidates the newest
  sample while earlier history remains intact. The flag is valid again
  after 20 steps on the 50 Hz meter setting. Recompiling withdraws the
  old run and its trace request returns `StaleRun`.
- The Stage 9 app bridge suite checks every new slot in OCF2 and OCT1,
  the run and revision metadata, rejected updates, and a real failed
  step that reports a stop notice without publishing a partial frame.
  The frontend suites cover delayed compile replies, stale frames and
  stale traces, catalog ranges, keyboard placement, and trace gaps.
- Issue [#256](https://github.com/SWOFART/open-circuit/issues/256) recorded
  a resistive 50 Hz voltmeter reading of 9.949610 V against the analytical
  9.970089 V. The Engine regression now checks the loaded divider at
  step 60. Integrating squared samples with the trapezoidal rule removes
  the downward bias from squaring the piecewise-linear waveform. Real
  power uses the same weights as RMS so the two readings stay consistent.
  The replay tables above record the original acceptance run.
  Stage 9 also leaves AC-driven relay and contactor coils undefined
  until Stage 10 and keeps AC devices outside assignment checks.

## Local checks and candidate CI

Windows local checks passed for `pnpm format:check`, `pnpm lint`,
`pnpm typecheck`, `pnpm test`, `pnpm types:generate`, `pnpm types:check`,
`pnpm boundaries:check`, `pnpm build`, `pnpm bundle:check`, and the
candidate whitespace and lockfile checks. Existing ts-rs and Vite
build messages did not affect results.

| CI evidence | Windows | Ubuntu | macOS |
|---|---|---|---|
| [Bridge PR #257](https://github.com/SWOFART/open-circuit/actions/runs/44529091597) | Pass | Pass | Pass |
| [Editor base PR #258](https://github.com/SWOFART/open-circuit/actions/runs/44533247765) | Pass | Pass | Pass |
| [Acceptance candidate CI](https://github.com/SWOFART/open-circuit/actions/workflows/ci.yml?query=branch%3Acodex%2Fissue-171-stage9-acceptance) | See matching PR head | See matching PR head | See matching PR head |

Each job checks formatting, lint, types, tests, architecture boundaries,
the production bundle and the Tauri build. The candidate PR records the
run for its exact reviewed head after CI completes.

## Native Windows observations

The native walkthrough used Windows 11 Home 10.0.26200 and the local
Tauri release executable from the editor base at `ad59562`. These
acceptance changes add no production inputs to that executable. The three
input projects used the schematics from the pinned requests, exported
to the untracked build directory and opened from the native file dialog.
The replay above supplies the exact simulation steps.

| Desktop action | Observed result |
|---|---|
| Open the RL wattmeter project | The wattmeter appears on the drawing. The inspector shows 10 MΩ, 100 mΩ and 50 Hz. Devices shows 9.980546 V RMS, 95.21707 mA RMS, 906.6209 mW, 284.8568 mvar, 950.3183 mVA, 0.9540181 PF and 17.44252°. |
| Select PW1 Voltage in the oscilloscope | The 100 ms window shows five 50 Hz periods and reports one sample every 1 ms. |
| Save As and reopen the wattmeter project with Ctrl+O | The saved drawing equals the input JSON by value. The run restarts near zero and recomputes the same settled meter readings. |
| Open the unbalanced star with neutral | The canvas shows the star winding and marked N, L1, L2 and L3 pins. The inspector shows 230 V, 50 Hz and Standard sequence. The neutral current changes with the simulation and a sampled display read 721.6724 mA. |
| Open the unbalanced star without the neutral wire | The preceding circuit is replaced. The sampled neutral-current display reads −142.1085 fA, consistent with zero within the numerical balance tolerance. |

Native work was observed on Windows. Ubuntu and macOS evidence comes
from CI and Tauri builds. No native walkthrough was performed there.
