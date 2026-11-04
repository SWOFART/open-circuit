# Stage 7 reference data

This note describes the reference circuits and expected data that the Stage 7 tasks compare
the Engine with: what they are, where each number comes from, how ngspice is aligned with the
[Stage 7 contract](../contracts/stage-7-nonlinear-circuits.md), which tolerance means what, and
how to regenerate everything without the Rust Engine. The model equations are the contract's
section 3; nothing here changes them.

## Files

All files are in `tests/golden/`.

| File | What it is |
|---|---|
| `stage-7-manifest.json` | The manifest: sources and versions, the alignment decisions, the sign and time conventions, the two kinds of tolerance, and one entry per fixture (circuit, netlist, Engine tolerance, and the contract file that records the same answer) |
| `stage-7-reference.py` | The model equations in 60-digit decimal arithmetic, and the transient integrator. Standard library only |
| `stage-7-references.py` | The case table, the netlist writer, the ngspice runner and parser, the comparisons, and the offline check. Standard library only |
| `stage-7-ngspice/*.cir` | 18 minimal executable netlists, one per fixture |
| `stage-7-analytical.expected.json` | The answers of the equations for every fixture, twice: `contract` (the exact SI 2019 constants of the contract) and `ngspiceConstants` (the same equations with the constants ngspice uses) |
| `stage-7-ngspice.expected.json` | The numbers ngspice 47 produced for the netlists, as it wrote them |
| `stage-7-diode-dc`, `stage-7-bjt-dc`, `stage-7-diode-rc-transient` `.expected.json` | The fixture files of the contract (section 9). Their numbers are the `contract` answers, and a test holds them to it |

## The fixtures

| Group | Cases |
|---|---|
| Diode, direct current | forward and reverse through 1 kΩ; a sweep from −5 V to 5 V in 1 V steps (the polarity change); and the supported but hard cases: forward at about 0.2 µA (0.6 V, 1 MΩ), forward at about 0.4 A (5 V, 10 Ω), and a non-default model (`I_S` = 1e-12 A, `n` = 1.8) |
| Transistor, direct current | NPN cutoff, forward active, and saturation; the same three for the PNP with every voltage negated; the NPN with its collector and emitter exchanged (reverse active, the wrong wiring); and the hard cases: the edge of saturation (`V_CE` ≈ 0.2 V), saturation at about 100 mA, and a non-default model (`I_S` = 1e-15 A, `β_F` = 30, `β_R` = 5) |
| Transient | a diode charging a capacitor through 10 kΩ and a Stage 6 switch that closes at 4 ms, and the diode reversed. No alternating source and no new waveform: the only time-varying input is the switch the Engine already has |
| Invalid | an ideal source directly across a forward diode, and across the base-emitter junction of the transistor stage. The expected diagnostic is the contract's `ModelRangeExceeded`, from the contract files. No ngspice comparison: ngspice has no such range rule, and no failure is invented for a circuit that the model supports |

## Where the numbers come from

**The equations (`stage-7-analytical.expected.json`).** `stage-7-reference.py` solves the model
equations of the contract's section 3 by Newton's method in 60-digit decimal arithmetic. Every
solution is checked by its residual in the same arithmetic (below 1e-40). The transient is
integrated by a fourth-order Runge–Kutta rule at steps of 20 µs and 10 µs, and the two must agree
to 1e-9 V. The script does not use the Engine, a simulator, or any package. Besides, a test of
Kirchhoff's current and voltage laws and of the balance of power, which do not depend on the
model, is run on every stored answer (`crates/engine/tests/stage_7_references.rs`).

**ngspice (`stage-7-ngspice.expected.json`).** The netlists are run by ngspice 47, the Windows
release `ngspice-47_64.7z` from <https://sourceforge.net/projects/ngspice/files/ng-spice-rework/47/>
(SHA-256 `59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a`). ngspice is not a
dependency of any build or normal test and is not stored in the repository. Its license is the
Modified BSD license of its `COPYING`. No manufacturer model data is used: every model card is
written here, with generic parameters.

## Aligning ngspice with the contract

A comparison is only meaningful for like models, so the netlists are written to remove the
differences that were found. Each one was measured, not assumed.

- **Model level.** ngspice's level-1 diode and its Gummel–Poon transistor, with every effect the
  contract defers left at its default of zero or infinity (series resistances, capacitances,
  transit times, breakdown, Early voltage, knee currents, recombination currents). The
  transistor then reduces to the transport form of Ebers–Moll of the contract's section 3.3.
- **Temperature.** `temp=27 tnom=27`, which is 300.15 K.
- **Junction conductance.** The contract's `g_min` is a conductance of 1e-12 S directly across
  each junction. ngspice's own `gmin` is set to 0, and a 1e12 Ω resistor is placed across each
  junction in the netlist. With ngspice's `gmin = 1e-12` the NPN cutoff collector current was
  2.0e-11 A against the contract model's 1.0e-11 A, because ngspice adds its `gmin` to the
  transport currents before dividing by the betas. The diode agrees either way; the explicit
  resistor is used for both so the netlists are alike.
- **Thermal voltage.** ngspice 47 computes `V_T` from the CODATA 2014 values `k` =
  1.38064852e-23 J/K and `q` = 1.6021766208e-19 C, not from the exact SI 2019 values of the
  contract. That is a relative difference of 3.4e-7 in `V_T` and changes a result by at most
  3.4e-7 V. The analytical file therefore holds a second set of answers with ngspice's
  constants, and ngspice agrees with those to its own convergence tolerance (5.5e-9 V at worst).
  That agreement is the evidence that the equations and the netlists describe the same model.
- **Convergence.** `reltol=1e-9 vntol=1e-12 abstol=1e-15` for direct current, so ngspice's own
  error is far below the quantities compared. The transient uses `method=trap reltol=1e-6` with a
  1 µs maximum step and is linearized to a 1 ms grid so the samples fall on the Engine's public
  steps; it agrees with the integrator to 5.5e-9 V.
- **Switch.** A voltage-controlled switch with 10 mΩ closed and 1e15 Ω open and a 1 ns control
  ramp at 4 ms. The Engine's open switch does not conduct; 1e15 Ω leaks about 2e-11 V over 4 ms.
- **Signs.** The diode current is positive entering the anode, which is minus ngspice's source
  current. The transistor currents are read through zero-volt sources wired so that each one
  enters the device terminal. All results use the contract's conventions.

## Two tolerances that must not be confused

- **Reference error** (`referenceError` in the manifest) is how far ngspice may be from the
  equations when both use the same constants and model: 30 nV, 1e-7 relative with an absolute floor
  of 1e-15 A for direct current, and 100 nV for the transient. It exists to show that the
  references are right. The simulator is never an oracle for the Engine.
- **Engine tolerance** (`engineTolerance` per fixture) is how far an Engine result may be from the
  `contract` answer: 1e-6 V and 1e-6 relative current for the direct-current cases, tighter
  absolute limits where the current is a leakage of 1e-11 A or less, and 5 mV for the transient,
  as in the contract files. Every one is at least ten times the reference error, which a test
  checks. An Engine test compares with the `contract` answers and with these tolerances, using an
  absolute tolerance plus a relative one so that a current that should be zero is not judged by a
  ratio.

The 5 mV of the transient is the contract's provisional tolerance. A reviewer's independent
emulation of the intended integration path differed from the reference by about 0.05 mV, which is
why it is not widened; tightening it, or loosening it, is a contract change.

## Regenerating

Nothing is overwritten without `--write`, and expected data is never replaced by an Engine
result. From the repository root, with Python 3 and, for the last two, a path to ngspice 47:

```text
python tests/golden/stage-7-references.py --check
python tests/golden/stage-7-references.py --write-netlists
python tests/golden/stage-7-references.py --analytical --write
python tests/golden/stage-7-references.py --manifest --write
python tests/golden/stage-7-references.py --ngspice <ngspice_con.exe> --write
```

`--check` needs no simulator: it verifies that the netlists match the case table, that the
manifest and the analytical file match the equations, and that the checked-in ngspice results
agree with the answers within the reference error. `--ngspice` without `--write` runs the
netlists and reports whether the result is identical to the checked-in file; with ngspice 47 it
is. A different simulator version is refused. The commands use no Rust code, so the references can
be regenerated and reviewed without building the Engine. The files are written with the standard
library's JSON writer; `prettier --write` is harmless, because every check compares parsed JSON.

A normal test run is offline: `cargo test` reads the checked-in files only
(`crates/engine/tests/stage_7_references.rs`).

## How the later tasks use the fixtures

- **Engine (#110, #111, #112).** The solver and device tests compile each fixture's circuit with
  the real Engine and compare with the `contract` answers using `engineTolerance`. The failure
  fixtures expect `ModelRangeExceeded` and no other diagnostic. Cases with a non-default model
  set the parameters through the catalog entries, not through special code.
- **Bridge (#113).** The same circuits are carried through `SimulationCore` as the Stage 6 session
  tests carry theirs: the result layout keys of the contract's section 7.1, the frames, a refused
  update, and the stop notice for a failed solve.
- **Editor (#114, #115).** The circuits are drawn with the editor's own commands, compiled by the
  real Engine through the bridge, and compared with the same answers; the wrong variants (the
  exchanged pins, the reversed diode) are drawn the way a student would draw them by mistake.
- Whoever changes a model, a constant, a range, or a tolerance changes the contract, the case
  table, and the answers together, and regenerates the files in the same change.
