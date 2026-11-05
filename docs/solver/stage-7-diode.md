# Generic diode implementation (#111)

`devices/diode.rs` owns the Stage 7 §3.2 Shockley law, analytical Jacobian and
fixed 1 pS physical junction conductance. Anode-to-cathode voltage and current
entering the anode use the standard two result slots. The device is memoryless;
no diode state enters a frame or saved project. The existing bounded solver
owns limiting, continuation and range diagnostics; no diode formula was added
to it. Temperature is fixed at 27 °C; breakdown, charge storage, series resistance
and thermal behavior are absent.

The authoritative catalog supplies inclusive saturation-current [1e-18, 1e-9] A
and emission-coefficient [1, 2] bounds, defaults 1e-14 A and 1. Compile, project
validation and live updates use those same bounds. Existing catalog parameters
omit the new optional bound fields. Version 1 projects store only model inputs;
earlier files remain valid. Older readers reject the unknown `diode` type before
replacing an active document. UI symbols and range presentation remain #114;
the existing palette hides types without registered symbols.

`stage_7_diode.rs` runs actual Engine circuits against the independent #109
`contract` answers and manifest tolerances: forward, reverse, low/high current,
non-default emission, source sweep, reversed orientation, and both switched RC
timelines. It checks KCL, power, near-zero/extreme bias, parameter boundaries,
range failure, rejected-edit rollback, deterministic repeats, trace replacement
and project round-trip. The module test checks derivatives and passivity at
parameter limits. Generic forced nonconvergence/continuation exhaustion remains
covered by the #110 solver tests; the diode does not add an artificial failure
mode to its constitutive law.

Local validation: 403 Rust workspace tests, 1197 TypeScript tests, clippy with
warnings denied, ESLint, typecheck, formatting, generated-type consistency and
architecture boundaries pass. DC errors are at floating-point rounding scale;
the largest forward-RC sample error is 4.733 mV (limit 5 mV), while reversed-RC
sample errors are about 5.01 nV. Golden data and tolerances were not changed.
