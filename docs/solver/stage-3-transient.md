# Stage 3 transient solver notes

This document records the Engine's deterministic internal-step estimate for
the Stage 3 ideal RC/RL/RLC models. The public simulation step remains 1 ms;
internal steps are not exposed as simulation times or trace samples.

## RC/RL/RLC natural-rate bound

For each electrical island, the Engine forms the small-signal nodal
conductance and capacitance matrices `G` and `C`, plus the inductor incidence
matrix `B` and diagonal inductance matrix `L`. Ideal voltage sources collapse
their terminals into one small-signal node; finite source resistances are
included in `G`. The linearized equations are `C v̇ + G v + B i = 0` and
`L i̇ = Bᵀ v`.

Nodes with positive capacitance diagonal entries are the candidate dynamic
voltage states; the rest are algebraic. The Engine removes algebraic voltages
with a Schur complement of `G`:

```text
G_eff = G_dd - G_da G_aa⁻¹ G_ad
B_eff = B_d  - G_da G_aa⁻¹ B_a
R_eff = B_aᵀ G_aa⁻¹ B_a

A = [ -C_dd⁻¹ G_eff   -C_dd⁻¹ B_eff ]
    [  L⁻¹ B_effᵀ     -L⁻¹ R_eff    ]
```

When there are no capacitors, only the lower-right inductor state block
remains. With no inductors, this reduces to the RC matrix `-C_dd⁻¹ G_eff`.
For each island, the Engine applies the diagonal similarity transform
`S = diag(sqrt(C_jj), sqrt(L_kk))` and uses the infinity norm
`||S A S⁻¹||∞`, inflated by a fixed relative margin, as an upper bound on the
magnitude of every eigenvalue (natural rate in `s⁻¹`, conventionally written
`rad/s`). The largest bound across islands is used for the circuit.

An island without capacitors or inductors contributes zero. If an algebraic
or capacitance reduction cannot be inverted reliably, the Engine
conservatively selects the 64-substep limit and reports
`StepAccuracyLimited`; it does not silently claim that the circuit is
resolved. The same fallback applies if the computed bound is non-finite.

The selected count is the smallest power of two `n` in `[1, 64]` satisfying
`omegaHat * (0.001 / n) <= sqrt(0.03)`. If no count satisfies it, the Engine
uses 64 and emits the warning. The result is included in compile and
successful live-parameter-update replies as `internalSubsteps`.

For the Stage 3 RC and RL fixtures, `omegaHat = 100 s⁻¹`, so the public 1 ms
step already satisfies the bound and `internalSubsteps = 1`. The lossless LC
fixture has `omegaHat = 316.228 rad/s`, which selects two 0.5 ms internal
steps and keeps its measured period error within the contract tolerance.
