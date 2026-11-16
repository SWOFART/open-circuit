# Stage 9 solver notes: alternating current in the time domain

These notes accompany [the Stage 9 contract](../contracts/stage-9-three-phase-foundation.md). They record how the contract's
requirements meet the existing transient engine; the contract wins where the two differ.

## What changes in the solve

- A sinusoidal source is a source whose value depends on the simulation time of the interval end. `SolveMode::Transient`
  already carries the step and the committed state (which holds `simulation_time_seconds`), so the device receives the interval
  end time and the accumulated phase. An operating-point solve uses `t = 0` and the accumulated phase at `t = 0`.
- The accumulated phase of each AC source is committed state, kept at the start of each public step as whole cycles plus a
  fraction of a cycle (contract section 3.5); it rolls back with a refused update or a step that fails to solve, and a frequency
  change continues it from the start of the recomputed public step (contract section 3.6).
- `choose_internal_substeps` takes `max(ω_circuit, 2π f_max)` as the rate bound, so 50 Hz uses 2 substeps, 60 Hz uses 4, and
  400 Hz uses 16 at the existing limit `√0.03`.
- Nothing about the nonlinear solve, the discrete events, or backward Euler after an event changes.

## What the measurements need

Each measuring device keeps, per window, running integrals of `x`, `x²` and `v·i` over the internal steps, a sample ring for the
`T/4` delay and the peak, and the last crossing times for the frequency. The window is `T = 1/f_meter`; one period is at most 1 s
(1 Hz), so at 64 substeps a ring of 64000 samples bounds the memory. An implementation may instead keep cumulative integrals and
subtract the value one period earlier by interpolation; it must match the definitions of the contract to its tolerances.
Validity restarts on a change of a source or meter frequency (contract section 5.4).

## Why 1 ms still works

The public step only sets how often frames and traces are published; the calculation and the measurements use the internal
substeps. 50 Hz is exactly 20 public steps per period; 60 Hz is not a whole number of steps, which matters for the display
only. The internal step has `ω h ≤ √0.03 ≈ 0.173`, where the trapezoidal rule's relative amplitude error is about
`(ω h)² / 12 ≈ 0.25 %`, which is why the contract's amplitude tolerance is 5e-3.

## Known limit

The relay and contactor coil thresholds compare the magnitude of the coil current, with hysteresis. On an alternating coil
current the contacts toggle between steps; `EventChatter` (same-instant re-solves only) does not fire, so nothing protects the
run and the behavior is undefined (the contract's section 9). Stage 10 adds an AC-aware coil.
