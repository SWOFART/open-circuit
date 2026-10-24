# Stage 1 resistive solver

The Engine compiles the schematic into the derived netlist, then solves each electrical
island independently. It uses modified nodal analysis (MNA) with node voltages as unknowns.
The selected reference node in each island has voltage zero. An ideal voltage source adds
one branch-current unknown.

## Device equations and stamps

For a two-terminal device, `V = V(pin1) - V(pin2)` and positive current enters `pin1`.
Power follows the passive sign convention: `P = V × I`.

### Resistor

For resistance `R > 0`, conductance is `G = 1/R`, and branch current is
`I = G × (V(pin1) - V(pin2))`. Its node-voltage matrix contribution is:

~~~text
      pin1  pin2
pin1   +G    -G
pin2   -G    +G
~~~

### DC source with internal resistance

For EMF `Vs` and internal resistance `Rs > 0`, the Thevenin source is stamped using its
equivalent Norton model: conductance `G = 1/Rs` across the terminals and current injection
`Is = Vs/Rs` into `pin1` (drawn from `pin2`). The reported terminal current is
`I(pin1) = (V(pin1) - V(pin2) - Vs)/Rs`.

When `Rs = 0`, MNA adds a branch-current unknown `I` and the ideal constraint
`V(pin1) - V(pin2) = Vs`. The branch current is positive entering `pin1`. If an ideal source
connects both terminals to the same conductor, compilation reports `ShortCircuit`; it does
not divide by zero or return an indeterminate current.

## Dense LU and failure handling

Stage 1 uses partial-pivoted dense LU. Pivot selection scales each candidate by the largest
absolute coefficient in its row. A scaled pivot is treated as rank-deficient when it is at
or below `16 × f64::EPSILON × matrix_dimension`. The solver also checks the normalized
system residual:

~~~text
residual = ||A·x - b||∞
scale    = max(max_row Σj |A[row,j]·x[j]|, ||b||∞, f64::MIN_POSITIVE)
~~~

The solve is rejected if `residual / scale > 1e-10`, if the matrix or solution is non-finite,
or if a pivot is numerically rank-deficient. Rejections return structured diagnostics;
the Engine never returns a successful snapshot containing `NaN` or infinity.

The pivot and residual tolerances apply only to numerical solution checks. Schematic
connectivity uses exact integer grid coordinates and has no epsilon tolerance.

## Acceptance fixture

The canonical fixture uses `Vs = 5 V`, `Rs = 0.1 Ω`, and `R1 = R2 = 1000 Ω`:

~~~text
I = Vs / (Rs + R1 + R2)
  = 5 / 2000.1
  ≈ 0.0024998750062496877 A
V(R2) = I × R2 ≈ 2.4998750062496877 V
~~~

After changing `R1` to `3000 Ω`:

~~~text
I = 5 / 4000.1 ≈ 0.0012499687507812304 A
V(R2) = I × 1000 ≈ 1.2499687507812304 V
~~~

Golden-fixture acceptance tolerances are at most `1 µV` for voltage and `1 nA` for
current. The headless command `cargo run -p open-circuit-engine-cli` solves both cases,
checks them against `tests/golden/stage-1-divider.expected.json`, and prints the results.
