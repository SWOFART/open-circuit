# Stage 15 acceptance record

Issue [#223](https://github.com/SWOFART/open-circuit/issues/223), parent
[#160](https://github.com/SWOFART/open-circuit/issues/160). Observed on
2026-11-26. The runtime candidate is
[PR #321](https://github.com/SWOFART/open-circuit/pull/321) at
`1d860d6464dda096d3923e355e511a4d97db712c`, stacked on the
[multimeter](https://github.com/SWOFART/open-circuit/pull/316),
[playback](https://github.com/SWOFART/open-circuit/pull/318) and
[example library](https://github.com/SWOFART/open-circuit/pull/320).

## Independent expectations

The [scored divider assignment](../../tests/golden/stage-15-divider-scored.ocassignment)
starts with the [Stage 1 drawn divider](../../tests/golden/stage-1-divider.schematic.json).
Its source is 5 V with 0.1 ohm source resistance and two 1,000 ohm resistors
in series. The positive probe at grid point (6, 0) is on the midpoint.
The negative probe at (9, 0) is on the return. Ohm's law gives
`5 * 1000 / (1000 + 1000 + 0.1) = 2.49987500625 V` across R2.
The reading is constant. While paused, elapsed wall time does not advance
model time, and one Step advances it by 0.001 s.

R2 voltage is worth 7 points and must match this result at 1 ms within
1 microvolt. A separate check requires three resistors and is worth 3
points. The supplied circuit has two resistors, so it should earn 7/10.
Only the count check should show its hint.

## Native Windows observations

In a Windows release build from the grading branch before commit,
**Open Example** listed six circuits and
opening **Voltage divider** created an **Untitled** document. In
**Student assignment**, the scored divider was opened through the
native assignment picker. The circuit was paused and the voltage
probes were placed at the two coordinates above.

| Action | Observed result |
| --- | --- |
| Read the paused meter | 2.499875 V at model time 3.264 s |
| Click Step once | Playback stayed paused. The meter showed 2.499875 V at model time 3.265 s. |
| Click Check my circuit | 7/10 points. R2 voltage passed with 7/7. The count check found two resistors, earned 0/3 and showed “This starter circuit has two resistors. The exercise calls for three.” |

## Automated checks

- `pnpm exec vitest run src/store/stage15Drawing.test.ts` constructs
  the divider from a blank document using editor commands and checks
  the drawing, the assignment's circuit, and resolved pin geometry.
- `cargo test -p open-circuit-app --test stage15_acceptance` opens that
  assignment, measures through the real Engine, holds a paused run
  over three simulated wall-clock seconds, advances one 1 ms step, and
  compares the grade and hint with the independent expectations.

## Runtime candidate verification

- The exact runtime commit built locally with `pnpm build` on Windows
  and passed `pnpm bundle:check`. The interactive observations above
  were from the earlier release build.
- The [runtime candidate CI](https://github.com/SWOFART/open-circuit/actions/runs/45886821402)
  passed on Windows, Ubuntu and macOS for
  `1d860d6464dda096d3923e355e511a4d97db712c`. The jobs ran the configured
  repository checks and Tauri builds. The CI test step is disabled.
  The circuit tests and the Windows desktop observations were performed
  locally.

Stage acceptance and merge remain with the project owner.
