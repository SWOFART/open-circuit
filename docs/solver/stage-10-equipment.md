# Stage 10 solver notes

The [equipment contract](../contracts/stage-10-three-phase-equipment.md)
defines the models and observable results. These notes describe the proposed
numerical treatment for review under #172.

The grouped-pole framework resolves ordered terminal pairs and uses
one latch for topology, conductance stamps and result slots. The Stage 6
contactor exercises the same path with its two auxiliary poles. Its
published catalog entry retains the legacy shape. Group metadata is
optional in the generated catalog DTO and is populated by Stage 10
device entries as they are added.

The three-pole contactor carries its detector with the coil state
through advancing and same-time solves. The breaker keeps its operator
position and trip latch in the committed run. An accepted breaker
operation settles at the newest public time and replaces that trace
sample. Recomputing the newest interval after another edit reapplies
the latest breaker settings at the endpoint.

The thermal overload keeps all three sensing paths conductive and switches
both auxiliary contacts from a single latch. It updates the detector from
the maximum phase current at each interval endpoint and advances the dose
once per internal step. Trip settling and reset leave detector and dose
unchanged. The published fuse-style record reports the latch and last
transition time. An eligible reset settles the auxiliary contacts at the
current time and replaces the newest trace sample.

For each coupled winding block write v = Ri + dλ/dt with λ = Li.
In MNA, the winding currents add KCL terms at their ordered pins. For
trapezoidal integration the winding residual is
λ₁ − λ₀ − h·[(v₁ − Ri₁) + (v₀ − Ri₀)]/2. Backward Euler uses
λ₁ − λ₀ − h·(v₁ − Ri₁). The transformer uses a 2×2 inductance
block per bank. A magnetically coupled bank keeps both winding equations
in the solve even when the primary and secondary have separate electrical
references. Reference fixing depends on galvanic connectivity.

The transformer and R–L loads seed their winding currents during the
operating-point solve. A zero-inductance load branch remains algebraic.
Transient solves use a coupled inductance block for each transformer
bank and scalar companion equations for the dynamic load branches.
Same-time updates hold the stored currents fixed. If opening a path
would leave those currents without a return, the candidate is refused.
The rate estimator includes the coupled blocks and their driving
resistance after eliminating the algebraic network.

For the motor use the same endpoint residual on each derivative in §4 of
the contract. In the stator KCL rows transform the three computed current
coordinates back to the six terminals. At the right endpoint the
Jacobian includes both derivatives of p·ωm·J90(λr), the flux derivatives
of torque, and shaft damping. Check voltage/current, flux and shaft
residuals in their respective units using the Stage 7 scale-aware
convergence rules. A same-time re-solve holds the integrated states
fixed. Event candidates share the existing rollback boundary with those
states.

The induction motor uses six transient unknowns for stator αβ0 flux,
rotor αβ flux, and speed. The winding KCL rows use the linear flux-to-current
map. Newton evaluates the rotor rotation and shaft torque as bilinear
residual terms at the same endpoint as the electrical solution. The
same-time solve stamps the committed winding currents and keeps the flux
and speed fixed.

For coupled motor rows, the solver can refine a stalled Newton iterate
by solving the tangent system against the physical residual. It checks
the refined solution against the same update and residual tolerances.

The rate estimate encloses the motor Jacobian over the declared state
limits. Ideal voltage constraints can fix the winding differences. For a
static linear feeding network, the solver also bounds the winding-voltage
feedback using the driving resistance after algebraic elimination. When
a joint enclosure is unavailable, including mixed dynamic or switching
networks without ideal winding constraints, the existing 64-substep
fallback reports the accuracy warning.

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
The [reference fixture guide](stage-10-references.md) lists the cases and
regeneration commands.
