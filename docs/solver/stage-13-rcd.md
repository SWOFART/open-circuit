# Stage 13 solver notes: the residual-current device

Contract: [`docs/contracts/stage-13-protection-and-safety.md`](../contracts/stage-13-protection-and-safety.md)
section 4. This file records how issue #198 implements it; the contract decides what it means.

## State and devices

`rcd_two_pole` and `rcd_four_pole` are one device model (`DeviceModel::Rcd`, `RcdDevice`) with two or
four linked poles (`pin_groups::RCD_TWO_POLES`, `RCD_FOUR_POLES`) of `closedContactResistanceOhms`
when conducting and an open circuit otherwise. `RcdState` holds the operator position, the latch,
the detector (mean-square residual, A²), the trip dose, the residual current of the last solve and
the last transition time. It lives in `CircuitStates::rcds` and is snapshotted as a `Switch`
device state, like the breaker's.

## One internal step

For each RCD, after the circuit of the step is solved, `RcdState::advance` takes the residual
current (the sum of the pole currents, each entering its first pin) at the start and the end of the
step, runs the same held-input first-order detector as the contactor and the overload relay
(`detector_step`, time constant `residualSenseSeconds`), and updates the dose: while the detector
RMS is at least `ratedResidualCurrentAmps` the dose grows by `h·(I/In)²/T` (T =
`tripTimeAtRatedSeconds`), below it the dose is cleared, and a latched device holds a dose of 1. At
a dose of 1 the device latches and the step goes through `settle_discrete_events` as a trip, so the
poles open in the same settled step (a same-time re-solve, like the overload relay's trip). The
first-order detector only approaches a constant residual from below, so a residual trips the device
only when it is above `In`.

## Operator

An explicit open clears the latch, the dose and the detector; a close is refused while latched; a
press is refused. A close never trips in its own reply because the detector and the dose start from
rest. The latest explicit position is kept in `rcd_overrides` and re-applied when the newest step
is recomputed (a live parameter edit), as the breaker's is; the detector and the dose are never
copied from an override. The rated residual current and the trip time are live parameters; the
sense time, the contact resistance and the initial state are compile-only.

## Results

After the pole voltage and current pairs: `<id>/tripped`, `<id>/residual_rms` (the detector RMS, a
`RmsCurrent` quantity) and `<id>/trip_dose` (the `ThermalDose` quantity, a dose from 0 to 1). The
advance step rewrites the last three values with the state at the end of the step.
