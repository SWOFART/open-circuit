# Stage 7 nonlinear solver implementation (issue #110)

The owner approved the #117 sign/motor-unit corrections and #118 finite roundoff
allowance on 2026-11-04. This implementation remains subject to independent Gate A.

## Interfaces and state

`Device::nonlinear()` supplies a constitutive-law view. Its junctions identify polarity,
thermal voltage and saturation current. Evaluation returns terminal currents as separate
physical terms and derivatives with respect to terminal-node potentials. Linear stamping
must exclude these same nonlinear contributions so they are not counted twice. Evaluation
must be pure: a Newton iterate does not update continuous or discrete device state.

The solver limits junction voltages separately from the solution vector. Terminal
linearization uses `G V* - I(V*)` as injection into nodal KCL. The residual evaluates
unlimited voltages and physical terms; auxiliary continuation conductance is present only
in the equations of its own stage. The last stage has none. Update and residual tests
must both pass in a non-limited iteration, before supported-range validation.

All 100-iteration limits and the ordered 11-conductance-plus-zero ladder follow the
contract. Nonlinear matrices are column/row equilibrated for dense LU, using the same LU
tolerances; solutions are unscaled before physical residual testing. Linear-only systems
keep the earlier direct solve and diagnostic behavior.

Committed node and junction voltages are private solver memory in `CircuitStates`, cloned
with continuous/discrete state and the public-step-start state. No solver state enters
project serialization, result layouts, frames or traces. Initial solves start at zero;
subsequent transient/event/update solves start from committed memory. Failed compile,
update and multi-step advance attempts never commit candidate memory or samples. A fatal
advance retains the previous snapshot and refuses further advance until a successful
update or compile. Stop-notice transport is separately owned by #113.

## Verification and current limit

Test-only constitutive devices exercise the actual Engine paths; production diode/BJT
catalog/model implementations remain #111/#112. Tests cover analytical forward/reverse
junctions, ideal-source range refusal, the full continuation ladder, bounded exhaustion,
singular/non-finite failures, original-equation residual rejection, motor dimensions,
mixed implicitly coupled motor solving, deterministic repeats, rollback and trace replacement.
They also cover a three-terminal two-junction law and rollback of a failed
nonlinear same-time fuse resolve, including its pending I²t transition and traces.

The switched RC test keeps the contract's independent reference values and 5 mV tolerance.
Before #118, subtracting potentials near 5 V across the 0.01 Ω switch produced a KCL
roundoff residual about 8.14e-14 A against a 3.99e-14 A threshold and failed to converge.
The approved allowance uses `gamma(m)` arithmetic bounds on each linear stamp's uncombined
products/RHS, plus row accumulation, as specified by Stage 7 section 4.4. It remains
separate from physical-current scale `S` and rejects non-finite bounds. Regression tests
accept this cancellation-limited case, reject a physically wrong current and refuse an
overflowing bound. The complete canonical RC timeline now passes with its unchanged
reference values. No compiler, lint, test or CI setting was weakened.

Norton conductance/history-injection stamps are grouped by physical branch when
forming `S`: the finite source, capacitor, inductor, relay coil and contactor coil
each contribute the magnitude of their total branch current. Contact currents
remain separate even when external wiring ties coil/contact nodes together.
Individual stamps remain available for the separate #118 roundoff bound.
Cancellation regressions use actual source/C/L devices to reject a roughly
10 nA KCL error that would otherwise inherit a roughly 100 A stamp scale.

Local validation on 2026-11-04: `cargo test --workspace` (387 tests),
`cargo clippy --workspace --all-targets -- -D warnings`, `pnpm exec vitest run`
(1196 tests), `pnpm format:check`, `pnpm typecheck`, `pnpm exec eslint .`,
`pnpm types:generate`, `pnpm types:check`, `pnpm boundaries:check`, `pnpm build`
and `pnpm bundle:check` passed. Existing ts-rs attribute-parser warnings remain.

Test-only counters record 12 Newton iterations for the 5 V/1 kohm forward
junction, 2 for reverse bias, and 32 across the full forced continuation ladder.
The canonical switched RC uses one internal substep per public 1 ms step.
The ceiling is 100 iterations per stage, at most 13 stages per island solve;
same-time event settling retains its separate existing bound. The app already
calls `advance` for one public step at a time and yields after its 8 ms batch
budget (`crates/app/src/simulation.rs`), so commands are serviced between complete
public steps. A single expensive step can exceed that budget; there is no
mid-Newton cancellation or change to the deterministic integration schedule.
