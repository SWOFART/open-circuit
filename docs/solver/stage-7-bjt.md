# Generic NPN and PNP implementation (#112)

`devices/bjt.rs` owns the Stage 7 transport-form Ebers–Moll equations and full
three-terminal analytical Jacobian. Both polarities use one constitutive law:
NPN has `p=+1`, PNP `p=-1`; junction inputs are `p*(Vb-Ve)` and `p*(Vb-Vc)`,
and terminal currents are multiplied by `p`. The two chain-rule signs cancel in
the derivatives with respect to actual node potentials. Fixed junction
conductance is physical; generic Newton limiting/continuation is not reported.

All four bias regions use the same equations. There is no operation-region state
or constant-beta shortcut in saturation. Temperature remains 27 °C; Early effect,
charge storage, parasitic resistance, breakdown, heating and RF behavior are
absent. Supported forward arguments stop at 80 thermal voltages, checked by the
shared solver; reverse bias carries transport leakage and the 1 pS shunts.

The authoritative catalog orders pins collector/base/emitter, and supplies the
inclusive bounds for saturation current, forward beta and reverse beta. There
are no cross-constraints: forward beta may be less than reverse beta. Compile,
project validation and every live update enforce the same catalog ranges.

Five explicit result keys report actual base-emitter and collector-emitter
voltage, then collector, base and emitter current. Public terminal currents enter
the named terminal; `i_E=-(i_C+i_B)` ensures conservation. Existing quantity,
unit and sign-convention enums are reused as the contract requires. No ambiguous
two-terminal BJT result is exposed. Input-only version 1 projects round-trip;
unknown fields/types are rejected. Generated DTOs come from Rust. UI symbols and
parameter presentation remain #114; the palette hides these Engine-only types.

Verification runs the actual Engine on all ten #109 BJT DC fixtures: cutoff,
forward active, saturation, reverse active, saturation edge, high current,
non-default beta and mirrored PNP. Manifest tolerances and reference files are
unchanged. Tests check terminal sum, KCL, power, full Jacobian at parameter
limits, both polarities and swapped pins, finite cross-combinations, rejected
edits, range failure, project roundtrip, repeated-run bits and current-step
trace replacement. A capacitor/Stage 6 switch test verifies explicit initial
state, RC charging, current-step backward-Euler replay on the contact edit and
settling to the independent forward-active bias point. Discrete state is not
committed during Newton iteration.

This branch builds on the separately reviewed diode/catalog change (#111).

Local validation: `cargo test --workspace` (412 tests), clippy with warnings
denied, 1199 Vitest tests, ESLint, typecheck, formatting, generated-type checks,
architecture boundaries, desktop build and production bundle check all pass.
The largest collector-current error in the ten independent DC fixtures is about
1.03e-16 A. Existing ts-rs attribute-parser warnings remain; no check setting or
golden tolerance was weakened.
