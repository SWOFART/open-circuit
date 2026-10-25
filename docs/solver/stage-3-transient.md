# Stage 3 transient solver notes

This document records the Engine's deterministic internal-step estimate for
the Stage 3 ideal RC models. The public simulation step remains 1 ms; the
internal steps are not exposed as simulation times or trace samples.

## RC natural-rate bound

For each electrical island, the Engine forms the small-signal nodal
conductance matrix `G` and capacitance matrix `C`. Ideal voltage sources
collapse their two terminals into one small-signal node. Nodes with no
capacitance are algebraic and are removed with a Schur complement of `G`.
For the remaining dynamic nodes, the natural decay rates are the eigenvalues
of `C⁻¹ G_eff`; passive RC networks have non-negative real rates in `s⁻¹`,
the same dimension as angular frequency conventionally written `rad/s`. The
Engine uses the matrix infinity norm `||C⁻¹ G_eff||∞`, inflated by a fixed
relative margin, as a deterministic upper bound on the magnitude of every
eigenvalue.
The bound is computed per island and the largest is used for the circuit.

An island without an independent dynamic capacitor degree of freedom
contributes zero. If the algebraic or capacitance reduction cannot be inverted
reliably, the Engine conservatively selects the 64-substep limit and reports
`StepAccuracyLimited`; it does not silently claim that the circuit is
resolved. The same fallback applies if the computed bound is non-finite.

The selected count is the smallest power of two `n` in `[1, 64]` satisfying
`omegaHat * (0.001 / n) <= sqrt(0.03)`. If no count satisfies it, the Engine
uses 64 and emits the warning. The result is included in compile and successful
live-parameter-update replies as `internalSubsteps`.

For the Stage 3 RC fixtures, `omegaHat = 100 s⁻¹`, so the public 1 ms step
already satisfies the bound and `internalSubsteps = 1`. Stage 3's inductor
work extends this estimate to RLC circuits and is covered by the lossless LC
regression fixture.
