# Stage 17 contract — extra analyses

Status — approved for implementation of the Stage 17 analyses (#233).

DC sweep, frequency response, and spectrum are separate requests. They leave
the fixed 1 ms public step, deterministic internal substeps, trapezoidal
integration, event rules and earlier accuracy targets unchanged. Rust owns
connectivity, validation, solving, spectrum math and diagnostics. The app owns
worker scheduling, cancellation and IPC. TypeScript sends inputs and displays
results. Projects continue to store drawn inputs only.

## 1. Shared inputs and bounds

Each request selects one quantity and uses SI units. DC sweep and frequency
response take an immutable schematic snapshot with its document revision.
They reuse ordinary schematic validation and Engine connectivity, including
rail attachment and per-island references. They accept at most 128 components,
512 wires, 128 rails, 128 markers (reference and junction combined) and 128
matrix unknowns per solved island.
Validate these bounds before the corresponding allocation or solve. Apply the
device allowlists to the whole drawing. Requests use the authored schematic
with no assignment fault overlay or live operator state.

For a DC output, select one existing scalar voltage or current slot by its
stable result key. For a frequency response, select an ordered pair of component
pins by stable ID and pin ID. Both pins must belong to the same electrical
island as the excitation. Its output is the voltage at the first pin minus the
voltage at the second. Reuse existing Engine sign conventions for DC readings.

## 2. DC sweep (#234)

Select one `dc_voltage_source` by component ID. Vary its `voltageVolts` and
retain its internal resistance. Other DC sources keep their authored values.
The endpoints must be finite, distinct, and within both the source's validated
range and ±1,000 V. Use 2–257 evenly spaced points, with the supplied endpoints
included exactly. Ascending and descending requests are both valid.

Support DC sources, R, C, L, diode, NPN and PNP, and the basic DC ammeter and
voltmeter. Capacitors are open and inductors are short at the operating point.
Transient initial conditions do not constrain that operating point. Use the
existing nonlinear model equations, convergence rules and aids. Start the
first point from the ordinary DC operating-point initialization. Seed every
subsequent point from the preceding accepted solution, using the same bounded
fallback as at the first point. Do not insert hidden sweep points. One failed
point fails the full request and reports its zero-based index and the underlying
Engine diagnostic. Discard accumulated output on failure or cancellation.

Return ordered pairs of source volts and the selected reading in volts or
amperes. Compare a resistive divider with its closed form, a diode load line
with an independently solved model equation, and NPN and PNP sweeps with
independently solved Ebers–Moll equations using the contracted shunt conductance.
For each reading require absolute error at most
`1e-8 V (or A) + 1e-4 × abs(reference)`. Exercise both directions,
a failed solve after a successful point, and the endpoint and count limits.

## 3. Frequency response (#235)

Support a linear circuit of DC voltage sources and R, C and L. Select one
`dc_voltage_source` as the input. Apply a unit small-signal voltage to its
ideal source element, before its internal resistance. Set the ideal voltage
of every other source to zero and retain all source resistances. Use the
linear complex MNA equations with capacitor admittance `jωC` and inductor
impedance `jωL`, where `ω = 2πf`. DC values and transient initial conditions
do not enter this perturbation solve. Reject unsupported devices with an
analysis diagnostic that identifies the offending components.

Require finite endpoints with `0.01 ≤ startHz < stopHz ≤ 100000`. Use 2–257
points and explicit linear or logarithmic spacing. Include endpoints exactly.
Return ascending frequency in Hz, magnitude `abs(Vout/Vin)` in V/V, and
principal phase `arg(Vout/Vin)` in degrees in `(-180, 180]`. Exact zero
magnitude has null phase. A singular or non-finite point fails the entire
request and identifies the failed index.

Check RC and RL first-order responses and an RLC series response with output
across the resistor, including points below, at and above resonance. Use
independent analytical transfer functions and cover finite source resistance.
Require magnitude error at most `1e-8 + 1e-5 × abs(reference)` and wrapped
phase error at most 0.01° when the reference magnitude is at least `1e-8`.
Cover unsupported devices, singular circuits and bounds.

## 4. Harmonic spectrum (#236)

Use the active run's raw Engine trace history. The request carries run ID,
layout version, document revision, signal key, inclusive end step and sample
count. Accept one instantaneous voltage or current slot and a power-of-two
sample count from 64 to 4096. Require exactly that many contiguous public-step
samples, all from the requested revision. Reject a stale run or layout,
eviction, missing or mixed-revision samples, and unsupported signal kinds.
Copy the bounded samples before doing the calculation. A previously captured
sample buffer is immutable. Neither interpolated values nor trace display
min/max buckets can serve as input.

Use a periodic Hann window `w[n] = 0.5 - 0.5 cos(2πn/N)` and the forward
DFT convention `X[k] = Σ w[n] x[n] exp(-j2πkn/N)`. Return all one-sided
bins from 0 to `N/2`. Their spacing is `1000/N Hz` and Nyquist is 500 Hz.
Report peak amplitude in the original signal unit, using
`2 abs(X[k]) / Σ w[n]` for interior bins and `abs(X[k]) / Σ w[n]` for DC
and Nyquist. Retain the DC bin. Report bin amplitudes directly, including
window leakage. The result concerns the recorded 1 kHz signal and inherits
its aliasing limits. There is no resampling or inferred fundamental.

Verify constant, Nyquist, bin-centered multitone and off-bin waveforms against
closed-form values or an independent direct DFT. Require amplitude error at
most `1e-10 V (or A) + 1e-8 × abs(reference)`. Check missing and mixed-revision
samples, minimum and maximum lengths, and repeatability for identical input.

## 5. Job and transport contract (#237)

Generate the tagged request and outcome DTOs from Rust using ts-rs. Use
dedicated Tauri JSON start, read and cancel commands with a session-local job
ID. IDs and step numbers that can exceed the JavaScript safe integer range
cross JSON as decimal strings. Each request and response names its analysis
kind. Successful payloads carry the document revision and selection, plus the
ordered points from sections 2–4. Spectrum also carries run ID, layout
version, inclusive capture bounds, bin spacing and amplitude unit.

Use one pending or running job per application window and retain at most one
terminal result there. Starting a second job while one is active returns busy.
A successful start replaces the prior terminal result. The app returns only
states for jobs owned by the requesting window. Read reports pending, running
or a terminal outcome of success, failure or cancelled. Failure includes a
machine-readable reason (invalid request, unsupported circuit or signal, stale
input, unavailable history, or solve failure) and any relevant Engine
diagnostics. Only success carries points. Validate all numeric output as finite
before publication. Requests and complete responses use bounded JSON payloads
of at most 512 KiB each. Exceeding a bound returns a structured error.

Run analysis outside the simulation worker. Capture spectrum samples with a
bounded read of the current history. Check cancellation before setup, between
DC points, between frequency points, between spectrum transform stages or
equivalent bounded blocks, and atomically before publishing. A cancel accepted
before publication discards the result. Cancelling a terminal job leaves its
outcome available for read. Document edits or reset cancel affected jobs and
invalidate old results at the app and UI boundaries. OCF2 frames, OCT1 traces
and the project file schema retain their existing formats.

Monte Carlo, optimization, noise, arbitrary parameter sweeps, nonlinear AC
linearization, and full Stage 17 UI and desktop acceptance are outside this
contract's Engine implementation issues.

The independent reference conventions for operating points and small-signal
AC can be cross-checked against the [ngspice manual](https://ngspice.sourceforge.io/docs/ngspice-manual.pdf).

The Engine DC sweep entry point is `engine::analyses::dc_sweep`. It takes
borrowed schematic geometry and a cancellation check, and returns either a
complete series or an analysis error. The application transport is tracked
by #237.

The Engine frequency entry point is `engine::analyses::frequency_response`.
It solves the excitation island and uses the existing dense LU solver on the
real block representation of its complex MNA system. The request validates
the whole drawing and identifies the output by two ordered pins.
