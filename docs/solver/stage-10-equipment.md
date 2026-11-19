# Stage 10 solver notes

The [equipment contract](../contracts/stage-10-three-phase-equipment.md)
defines the models and observable results. These notes describe the proposed
numerical treatment for review under #172.

For each coupled winding block write v = Ri + dλ/dt with λ = Li.
In MNA, the winding currents add KCL terms at their ordered pins. For
trapezoidal integration the winding residual is
λ₁ − λ₀ − h·[(v₁ − Ri₁) + (v₀ − Ri₀)]/2. Backward Euler uses
λ₁ − λ₀ − h·(v₁ − Ri₁). The transformer uses a 2×2 inductance
block per bank. A magnetically coupled bank keeps both winding equations
in the solve even when the primary and secondary have separate electrical
references. Reference fixing depends on galvanic connectivity.

For the motor use the same endpoint residual on each derivative in §4 of
the contract. In the stator KCL rows transform the three computed current
coordinates back to the six terminals. At the right endpoint the
Jacobian includes both derivatives of p·ωm·J90(λr), the flux derivatives
of torque, and shaft damping. Check voltage/current, flux and shaft
residuals in their respective units using the Stage 7 scale-aware
convergence rules. A same-time re-solve holds the integrated states
fixed. Event candidates share the existing rollback boundary with those
states.

Include all coupled winding modes and the attached-network contribution
in the Stage 3 estimate. For a transformer winding matrix L with minimum
eigenvalue λmin > 0 and resistive driving matrix R, ‖R‖₂/λmin bounds its
electrical decay rates.

For each Stage 10 coupled solve block build a conservative interval
enclosure of the finite dynamic Jacobian after algebraic elimination.
Include the network's linear dynamic rows and all motor flux and shaft
rows. Normalize each state coordinate by a fixed positive SI scale for
its unit before taking row sums. A diagonal change of coordinates
preserves eigenvalues. The largest sum of entrywise absolute upper
bounds in any row bounds the magnitude of every finite mode. Take the
maximum with the existing circuit and source estimates and across every
reachable contact configuration. Compute from candidate parameters and
declared state limits at compile and after live parameter changes.

Let Φ = 100 V·s and Δ = Ls·Lr − Lm². In each motor's unscaled rows the
rotation term has flux derivative bounded by p·3000 and speed
derivative bounded by p·Φ. Write torque as
−p·Lm/Δ·(λsα·λrβ − λsβ·λrα). Each of its four flux derivatives is
bounded by p·Lm·Φ/Δ. Include each bound divided by Jm in the shaft
row, alongside B/Jm. Apply the same coordinate scaling as for the
linear terms before forming row sums. A configuration that cannot
produce a finite enclosure uses 64 substeps and reports
StepAccuracyLimited. A valid enclosure that exceeds the Stage 3
accuracy threshold follows the same existing cap and warning.
Reference fixtures verify convergence and accuracy for their actual
motions and report any conservative accuracy warning.

For the first-order coil and overload detectors with held squared-current
input u, use x₁ = x₀ + (u − x₀)·(1 − exp(−h/τ)).
Use a stable exponential difference near zero. The detector and dose
are advanced once after the pre-transition electrical solve. Same-time
resolves and operator resets do not advance either. The overload's
piecewise dose integration uses the detector at the right endpoint.

Reference fixtures use analytical DC, balanced phasors and independent
transient integration. SPICE behavioral sources implement the model
equations for comparisons, and coupled inductors cross-check transformer
mutual terms. Keep those implementations independent of the Rust stamps.
