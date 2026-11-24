# Stage 13 solver notes: breaker trip classes

Contract: [`docs/contracts/stage-13-protection-and-safety.md`](../contracts/stage-13-protection-and-safety.md)
section 5. This file records how issue #199 implements it; the contract decides what it means.

## Parameters

`BreakerThreePoleParameters` gains five optional fields: `curveClass` (0 to 3) and, used only when
the class is 1 to 3, `ratedCurrentAmps`, `thermalSenseSeconds`, `tripScaleSeconds` and
`coolSeconds`. Absent, the class is 0 and the breaker is the Stage 10 breaker to the bit: the new
state fields stay zero and are never read, the layout has no extra slot, and the magnetic
threshold is `instantaneousTripAmps`. An absent thermal input takes the catalog default
(`DEFAULT_BREAKER_*`, the thermal overload relay's). All five are compile-only; the live parameter
of the breaker stays `instantaneousTripAmps` (ignored by the classes 1 to 3).

## Magnetic part

`magnetic_threshold_amps` is 5, 10 or 20 times the rated current for the classes B, C and D. The
check is the Stage 10 one (`BreakerState::trip_if_needed`, in `settle_discrete_events`): any pole's
absolute current at or above the threshold trips the breaker at the first sampled crossing, and a
close onto an overcurrent can trip in the same settled reply.

## Thermal part

`BreakerState` carries the detector (mean-square of the largest pole current, A²), the dose and the
last largest pole current. Each internal step `advance_thermal` runs the overload relay's detector
(`detector_step`) and dose law: above `q = detector / In² = 1` the dose gains `h (q − 1) / K`, below
it the dose decays with `exp(−h / cool)`; at a dose of 1 a conducting breaker latches. The dose cools
whether the breaker conducts or not. The trip goes through `settle_discrete_events` as a pending trip
(excluded from the chatter vector, like the overload relay's). An explicit open clears the latch and
keeps the dose; a close is refused only while latched. An override replayed after a live edit never
copies a detector or dose.

## Results

The result layout is built for each component from its own parameters
(`slots::component_slot_definitions`): a breaker with a class has `<id>/thermal_dose` after
`<id>/tripped` (the `ThermalDose` quantity). The advance step rewrites the last two values.
