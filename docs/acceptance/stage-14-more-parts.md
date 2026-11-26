# Stage 14 acceptance record

Issue [#216](https://github.com/SWOFART/open-circuit/issues/216), parent
[#159](https://github.com/SWOFART/open-circuit/issues/159). Observed on
2026-11-26. The runtime candidate is the editor PR
[#310](https://github.com/SWOFART/open-circuit/pull/310) at
`a6ec92024491f40f2bfff2d5cf00bb76b33df600`, stacked on the
[reference fixtures](https://github.com/SWOFART/open-circuit/pull/308)
and [app bridge checks](https://github.com/SWOFART/open-circuit/pull/309).

The [Stage 14 contract](../contracts/stage-14-more-parts.md) specifies the
devices, result signs, and tolerances. The editor tests start with a blank
document and use the normal editor commands to place parts, edit parameters,
route orthogonal wires, and mark a reference. The pinned requests in
[tests/golden/stage-14](../../tests/golden/stage-14) contain the drawings
and resolved pin geometry. The Rust acceptance suite compiles those exact
drawings and checks frames from the real Engine through the app core.

| Circuit | Automated comparison |
| --- | --- |
| Timed star–delta starter | Uses the Stage 10 induction motor and interlocked contactors with two Stage 14 on-delay relays. Closing start at public step 20 starts the 90 ms and 120 ms delays. The timer outputs first change at steps 110 and 140 by the reference deadline rule. The star contactor releases before delta picks. The test checks every public frame from start through step 180 for the order and lack of overlap. |
| Emergency-stop chain | A 24 V supply, latching emergency stop, 3 ms relay and 120 Ω lamp. The relay picks exactly at the reference deadline. Lamp power matches both observations in the `lamp-24` reference within the contract tolerance, including the small drop across the closed contact. Latch opens the circuit, release preserves the latch, and reset starts a new full delay. |
| LED and op-amp | A 3 V follower drives the LED through 220 Ω. The independent load line combines the Shockley equation with the closed-loop Thevenin source. The test checks amplifier output voltage and negative output current, LED forward voltage and current, and the lit state. |

The [reference guide](../solver/stage-14-references.md) records the individual
device oracles, provenance, and regeneration commands. The compound
LED/follower comparison uses the analytic load line. The starter comparison
covers timing and contact states.

## Native desktop observations

The Windows Tauri debug build ran with its Vite frontend and actual Rust
backend. Each pinned request's `schematic` was opened as a local `.ocircuit`
file through the desktop project picker. The programmatic editor construction
and the interactive desktop observations exercise different entry points.

| Circuit | Observed in the Windows desktop app |
| --- | --- |
| Timed star–delta starter | **Fit to circuit** showed the wired starter. Clicking S1 on the canvas closed the switch. Devices showed both timers counting, Q1 and Q2 picked, and Q3 released in star. Later Q2 was released at 313.9844 ms and Q3 picked at 333.9688 ms with motor current present. |
| Emergency-stop chain | Devices initially showed S1 armed, K1 picked and H1 lit at 23.998 V, 199.9833 mA and 4.7992 W. **Latch** changed the state to latched, released K1 and reduced lamp power to 0 W. **Reset** restored armed, picked and lit. |
| LED and op-amp | Devices showed the amplifier linear with 2.996996 V output and −7.20282 mA output current. The LED reported lit, 1.412375 V and 7.20282 mA. |

The general Results panel marked lamp power “Not yet valid” while Devices
showed the correct power. This display defect is tracked in
[#311](https://github.com/SWOFART/open-circuit/issues/311).

## Verification

- `pnpm exec vitest run src/store/stage10Drawing.test.ts` verifies the exact
  drawings from editor commands.
- `cargo test -p open-circuit-app --test stage14_acceptance --test stage14_session`
  verifies the three comparisons and the Stage 14 project and operator bridge.
- `pnpm --filter @open-circuit/app tauri build --debug --no-bundle` built
  the Windows executable used for the desktop observations. The debug WebView
  used `pnpm dev:web` on the same editor candidate.
- The [runtime candidate CI](https://github.com/SWOFART/open-circuit/actions/runs/45819740916)
  passed on Windows, Ubuntu, and macOS. It runs the repository's checks and
  Tauri builds. The test step in the existing workflow is disabled, so the
  automated circuit comparisons and interactive desktop observations above
  were performed locally on Windows.

Stage acceptance and merge remain with the project owner.
