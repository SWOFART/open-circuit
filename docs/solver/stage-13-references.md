# Stage 13 reference data

Issue #202, contract [`docs/contracts/stage-13-protection-and-safety.md`](../contracts/stage-13-protection-and-safety.md)
section 11. The data are in `tests/golden/stage-13/` and are written by one script that has no
dependency on the application or the Engine, only on Node.

```bash
node scripts/stage13-references.mjs --write   # regenerate analytical.json and manifest.json
node scripts/stage13-references.mjs --check   # verify the committed files (also run by vitest)
```

`manifest.json` records the generator command, the tools, the SHA-256 of `analytical.json` and the
fixture counts. The only tool is Node (the data were generated with 22.20.0; 20 or later is
supported) and the data use no package and only IEEE-754 arithmetic with `Math.exp`, `Math.sqrt`
and `Math.sin`: closed forms and a 1 microsecond integration. The manifest records a constant, not
the running version, so `--check` does not depend on the machine.

## What is in it

Each fixture is a complete compile request (schematic, pin geometry, faults) and a list of checks:
`value` (read a layout key after `step` public steps, within a relative and an absolute
tolerance) and `tripStep` (step until a `tripped` key is 1 and compare the time, or require that it
never trips within a horizon). Fixtures are `correct` or `wrong`: a wrong variant builds a
different circuit (a missing bond, a poor electrode, a rated current changed, a class swapped, a
wrong short resistance, a breaker set too high) and expects the different result that circuit
really gives, so the Engine is held to reproducing a wrong circuit as it is and not repairing it.

| Family                                 | Closed form                                                                                                                                                                                                                                        |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| earth fault, DC (four resistance sets) | `I = E / (Rs + Rf + Re)`, frame `Re·I`; unbonded: `E·1G / (Rs + Rf + 1G)` and `E / (Rs + Rf + 1G)` (to 1e-10 A)                                                                                                                                    |
| earth fault, three-phase TN style      | a leak on `l1` of a star source feeding a balanced star load with the neutral at the source: the node of phase 1 is `E / (1 + Rs (1/Rl + 1/(Rf + Re)))`, the touch voltage `Re · node / (Rf + Re)`; unbonded: the frame floats                     |
| touch voltage (50 Hz)                  | RMS `Re·E / (Rs + Rf + Re)`, peak `√2` times it (Stage 9 measured-RMS tolerance, 2e-3); no earth node: not valid, not hazardous                                                                                                                    |
| residual-current device, DC            | trip time of the detector `I²(1 − e^(−t/τ))` with the dose `(I/In)²/T` while the RMS is at least `In`, at 1.05, 2 and 5 × `In`; 0.5 × `In` and a balanced load never trip                                                                          |
| residual-current device, 50 Hz         | the same law with the oracle integrating the squared sinusoid itself, at 1.25, 2 and 5 × `In` (tolerance `max(3 ms, 2 %)`); 0.5 × `In` never trips                                                                                                 |
| breaker classes                        | magnetic at 1.02 and 0.98 × 5, 10, 20 × `In`; thermal time `K/(q − 1)` with the detector, at (D, 5.5), (C, 3), (B, 2), (B, 1.2) × `In`; 0.9 × `In` never trips                                                                                     |
| short circuit                          | DC `E / (Rs + Rshort)`; cleared by a class C breaker and by a class 0 breaker set below the current (tripped, then zero current); the three-phase prospective peak `√2 · √3 · E / (2 Rs + Rshort + burden)` read by an AC ammeter (tolerance 5e-3) |

Trip times are compared within `max(2 ms, 1 %)` (DC) or `max(3 ms, 2 %)` (50 Hz; see the list of differences below).
`crates/engine/tests/stage_13_references.rs` runs every fixture against the Engine.

## What is not in the fixtures

These rows of contract section 11 are not fixtures here; each is covered by a test of the Engine
crate, which is a test of the Engine against itself and not an independent reference:

- the RCD sequence (close on a residual, delayed trip, refused close while latched, open clearing the
  latch, the dose and the detector): `crates/engine/tests/stage_13_rcd.rs`;
- the breaker rule that a close is refused only while latched, and the class 0 breaker being the
  Stage 10 breaker: `crates/engine/tests/stage_13_breaker.rs` and the unchanged Stage 10 tests;
- the breaker sweep of 0.9, 1.2, 2, 5.5, 10.5 and 21 times the rated current for every class: the
  fixtures use the ad hoc set listed above, which covers each class's magnetic edge and four
  thermal points.

There is no ngspice reference: the contract allows one only where it can be built without
reproducing the Engine's own formula, and the RCD and breaker detectors are the Engine's own law.

## Where the fixtures differ from contract section 11

The fixtures relax or change these rows of the section 11 table; each is a deliberate choice and
none is hidden:

1. **Unbonded frame current.** A current of the order of `E / 1 GΩ` is held to 1e-10 A absolute,
   not 1e-12 A: the 1 GΩ exposed-part resistor beside ohm-scale branches limits double precision
   (the Engine reads 4.000196e-7 A for 4.0e-7 A). The contract row is amended to match.
2. **Three-phase TN row.** The fixture checks only the frame's touch voltage and the hazard flag,
   at 5e-3 relative; the contract also names the line currents and the earth-fault current at
   1e-6. They are not checked here (the touch voltage is the Engine's measured RMS over a 1 ms
   sampled window, which cannot reach 1e-6).
3. **Prospective peak.** The tolerance is 5e-3 relative, not 1e-6, and is looser than the Stage 9
   measured-peak tolerance of 2e-3: the peak is read from a 1 ms sampled window by an AC ammeter
   with a burden, at 20 samples per period.
4. **Clearing row.** The class 0 clearing fixture is the DC 100 V short through a breaker set to
   50 A (tripped at the first sample, current then zero), not the contract's three-phase circuit
   with pole 1 in the `l1` line; the three-phase peak is a separate fixture without a breaker.
5. **50 Hz RCD trip time.** The tolerance is `max(3 ms, 2 %)`, not `max(2 ms, 1 %)`: the Engine
   integrates the squared sinusoid with held 1 ms inputs while the oracle uses a continuous
   waveform. The DC rows keep `max(2 ms, 1 %)`.

Rows 2 to 5 are tolerances the data need today, to be tightened if the Engine's sampling is
refined, or amended in the contract by the acceptance record (#206).
