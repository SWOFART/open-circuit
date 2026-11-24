//! Engine-owned rolling measurements for the Stage 9 AC meters.

use std::collections::{HashMap, VecDeque};

use crate::{
    ComponentSpec, Diagnostic, DiagnosticCode, StableId,
    devices::{DeviceResult, component_diagnostic},
};

const MAX_HISTORY_SECONDS: f64 = 2.01;

#[derive(Clone, Copy, Debug)]
pub(super) struct MeasurementSample {
    pub(super) time_seconds: f64,
    pub(super) voltage_volts: f64,
    pub(super) current_amps: f64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct MeasurementState {
    samples: VecDeque<MeasurementSample>,
    valid_since_seconds: f64,
}

pub(super) fn seed(
    schematic: &crate::Schematic,
    results: &mut [DeviceResult],
    time_seconds: f64,
) -> HashMap<StableId, MeasurementState> {
    let mut states = HashMap::new();
    for component in &schematic.components {
        if !is_measurement(component) {
            continue;
        }
        let mut state = MeasurementState {
            samples: VecDeque::new(),
            valid_since_seconds: time_seconds,
        };
        if let Some(result) = result_mut(results, component.id()) {
            state.samples.push_back(sample_at(time_seconds, result));
        }
        states.insert(component.id().clone(), state);
    }
    let _ = update(schematic, &mut states, results, time_seconds);
    states
}

/// Append the settled internal-substep sample and refresh the result slots for every AC meter.
pub(super) fn update(
    schematic: &crate::Schematic,
    states: &mut HashMap<StableId, MeasurementState>,
    results: &mut [DeviceResult],
    time_seconds: f64,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for component in &schematic.components {
        let Some(frequency_hz) = frequency_hz(component) else {
            continue;
        };
        let Some(result_index) = results
            .iter()
            .position(|result| result.component_id == *component.id())
        else {
            continue;
        };
        let result = &mut results[result_index];
        let state = states
            .entry(component.id().clone())
            .or_insert_with(|| MeasurementState {
                samples: VecDeque::new(),
                valid_since_seconds: 0.0,
            });
        push_sample(state, sample_at(time_seconds, result));
        trim_history(state, time_seconds);

        // A touch voltage is read against the earth node: an exposed part whose island is not
        // joined to it has no reading (docs/contracts/stage-13-protection-and-safety.md 7.2).
        if matches!(component, ComponentSpec::ExposedPart { .. })
            && result
                .extra_values
                .iter()
                .find(|(key, _)| key.ends_with("/earth_joined"))
                .is_none_or(|(_, joined)| *joined != 1.0)
        {
            write_invalid(component, result);
            continue;
        }

        let period_seconds = 1.0 / frequency_hz;
        if !period_seconds.is_finite() || period_seconds <= 0.0 {
            diagnostics.push(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                component.id(),
            ));
            continue;
        }
        let window_start = time_seconds - period_seconds;
        let window_is_valid =
            time_seconds - state.valid_since_seconds >= period_seconds && window_start.is_finite();
        if !window_is_valid {
            write_invalid(component, result);
            continue;
        }

        let voltage_signal = matches!(component, ComponentSpec::AcVoltmeter { .. });
        let current_signal = matches!(component, ComponentSpec::AcAmmeter { .. });
        let samples = state.samples.iter().copied().collect::<Vec<_>>();
        let voltage_stats =
            match signal_stats(&samples, window_start, time_seconds, Signal::Voltage) {
                Some(stats) => stats,
                None => {
                    diagnostics.push(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        component.id(),
                    ));
                    write_invalid(component, result);
                    continue;
                }
            };
        let current_stats =
            match signal_stats(&samples, window_start, time_seconds, Signal::Current) {
                Some(stats) => stats,
                None => {
                    diagnostics.push(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        component.id(),
                    ));
                    write_invalid(component, result);
                    continue;
                }
            };
        let mut values = Vec::<(&'static str, f64)>::new();
        if voltage_signal {
            values.extend([
                ("rms_voltage", voltage_stats.rms),
                ("mean_voltage", voltage_stats.mean),
                ("peak_voltage", voltage_stats.peak),
            ]);
        } else if current_signal {
            values.extend([
                ("rms_current", current_stats.rms),
                ("mean_current", current_stats.mean),
                ("peak_current", current_stats.peak),
            ]);
        } else if let ComponentSpec::ExposedPart { parameters, .. } = component {
            // The touch voltage is the RMS of the voltage against the earth node over one
            // period; it is hazardous above the limit (a plain comparison, no time curve).
            values.extend([
                ("touch_voltage_rms", voltage_stats.rms),
                ("touch_voltage_peak", voltage_stats.peak),
                (
                    "hazard",
                    if voltage_stats.rms > parameters.touch_voltage_limit_volts {
                        1.0
                    } else {
                        0.0
                    },
                ),
            ]);
        } else {
            let Some(real_power) = integrate_product(&samples, window_start, time_seconds, 0.0)
            else {
                diagnostics.push(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    component.id(),
                ));
                write_invalid(component, result);
                continue;
            };
            let apparent_power = voltage_stats.rms * current_stats.rms;
            let Some(quadrature) =
                integrate_product(&samples, window_start, time_seconds, period_seconds / 4.0)
            else {
                // The shifted voltage integral needs T/4 of pre-window history. At a
                // run start that history does not exist yet, so keep every wattmeter
                // result invalid until its first complete shifted window is available.
                write_invalid(component, result);
                continue;
            };
            let reactive_power = quadrature.signum()
                * (apparent_power * apparent_power - real_power * real_power)
                    .max(0.0)
                    .sqrt();
            let power_factor = if apparent_power > 0.0 {
                real_power / apparent_power
            } else {
                0.0
            };
            let phase_angle = reactive_power.atan2(real_power).to_degrees();
            values.extend([
                ("rms_voltage", voltage_stats.rms),
                ("rms_current", current_stats.rms),
                ("real_power", real_power),
                ("reactive_power", reactive_power),
                ("apparent_power", apparent_power),
                ("power_factor", power_factor),
                ("phase_angle", phase_angle),
            ]);
        }
        if voltage_signal || current_signal {
            let (signal, mean) = if voltage_signal {
                (Signal::Voltage, voltage_stats.mean)
            } else {
                (Signal::Current, current_stats.mean)
            };
            let (frequency, frequency_valid) = measured_frequency(
                &samples,
                signal,
                mean,
                time_seconds - 2.0 * period_seconds,
                time_seconds,
                state.valid_since_seconds,
            );
            values.push(("frequency", frequency));
            values.push(("frequency_valid", if frequency_valid { 1.0 } else { 0.0 }));
        }
        values.push(("valid", 1.0));
        if values.iter().any(|(_, value)| !value.is_finite()) {
            diagnostics.push(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                component.id(),
            ));
            write_invalid(component, result);
            continue;
        }
        for (suffix, value) in values {
            set_extra(result, format!("{}/{}", component.id(), suffix), value);
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Mark the start of a new measurement epoch while retaining the physical history needed by the
/// wattmeter's quarter-period delayed voltage term.
pub(super) fn reset_epoch(
    states: &mut HashMap<StableId, MeasurementState>,
    component_ids: impl IntoIterator<Item = StableId>,
    time_seconds: f64,
) {
    for component_id in component_ids {
        if let Some(state) = states.get_mut(&component_id) {
            state.valid_since_seconds = time_seconds;
        }
    }
}

fn is_measurement(component: &ComponentSpec) -> bool {
    matches!(
        component,
        ComponentSpec::AcVoltmeter { .. }
            | ComponentSpec::AcAmmeter { .. }
            | ComponentSpec::Wattmeter { .. }
            | ComponentSpec::ExposedPart { .. }
    )
}

fn frequency_hz(component: &ComponentSpec) -> Option<f64> {
    match component {
        ComponentSpec::AcVoltmeter { parameters, .. } => Some(parameters.frequency_hz),
        ComponentSpec::AcAmmeter { parameters, .. } => Some(parameters.frequency_hz),
        ComponentSpec::Wattmeter { parameters, .. } => Some(parameters.frequency_hz),
        ComponentSpec::ExposedPart { parameters, .. } => Some(parameters.frequency_hz),
        _ => None,
    }
}

fn sample_at(time_seconds: f64, result: &DeviceResult) -> MeasurementSample {
    MeasurementSample {
        time_seconds,
        voltage_volts: result.voltage_volts,
        current_amps: result.current_amps,
    }
}

fn result_mut<'a>(
    results: &'a mut [DeviceResult],
    component_id: &StableId,
) -> Option<&'a mut DeviceResult> {
    results
        .iter_mut()
        .find(|result| result.component_id == *component_id)
}

fn push_sample(state: &mut MeasurementState, sample: MeasurementSample) {
    if let Some(previous) = state.samples.back_mut()
        && previous.time_seconds == sample.time_seconds
    {
        *previous = sample;
    } else {
        state.samples.push_back(sample);
    }
}

fn trim_history(state: &mut MeasurementState, time_seconds: f64) {
    let cutoff = time_seconds - MAX_HISTORY_SECONDS;
    while state.samples.len() > 2
        && state
            .samples
            .get(1)
            .is_some_and(|sample| sample.time_seconds < cutoff)
    {
        state.samples.pop_front();
    }
}

#[derive(Clone, Copy)]
enum Signal {
    Voltage,
    Current,
}

impl Signal {
    fn read(self, sample: MeasurementSample) -> f64 {
        match self {
            Self::Voltage => sample.voltage_volts,
            Self::Current => sample.current_amps,
        }
    }
}

#[derive(Clone, Copy)]
struct SignalStats {
    mean: f64,
    rms: f64,
    peak: f64,
}

fn signal_stats(
    samples: &[MeasurementSample],
    start: f64,
    end: f64,
    signal: Signal,
) -> Option<SignalStats> {
    let points = points_in_window(samples, start, end, signal)?;
    let mut integral = 0.0;
    let mut square_integral = 0.0;
    let mut peak: f64 = 0.0;
    for pair in points.windows(2) {
        let duration = pair[1].0 - pair[0].0;
        let left = pair[0].1;
        let right = pair[1].1;
        integral += duration * (left + right) * 0.5;
        // Integrate the squared samples. Squaring the interpolated waveform
        // instead biases the RMS of a sinusoid down at the contracted substep rate.
        square_integral += duration * (left * left + right * right) * 0.5;
        peak = peak.max(left.abs()).max(right.abs());
    }
    let duration = end - start;
    let mean = integral / duration;
    let mean_square = square_integral / duration;
    let rms = mean_square.max(0.0).sqrt();
    (mean.is_finite() && rms.is_finite() && peak.is_finite()).then_some(SignalStats {
        mean,
        rms,
        peak,
    })
}

fn points_in_window(
    samples: &[MeasurementSample],
    start: f64,
    end: f64,
    signal: Signal,
) -> Option<Vec<(f64, f64)>> {
    if !start.is_finite() || !end.is_finite() || end <= start {
        return None;
    }
    let mut points = vec![
        (start, interpolate(samples, start, signal)?),
        (end, interpolate(samples, end, signal)?),
    ];
    points.extend(
        samples
            .iter()
            .filter(|sample| sample.time_seconds > start && sample.time_seconds < end)
            .map(|sample| (sample.time_seconds, signal.read(*sample))),
    );
    points.sort_by(|left, right| left.0.total_cmp(&right.0));
    points.dedup_by(|left, right| left.0 == right.0);
    Some(points)
}

fn interpolate(samples: &[MeasurementSample], time_seconds: f64, signal: Signal) -> Option<f64> {
    let first = *samples.first()?;
    let last = *samples.last()?;
    if time_seconds < first.time_seconds || time_seconds > last.time_seconds {
        return None;
    }
    let upper = samples.partition_point(|sample| sample.time_seconds < time_seconds);
    if upper == 0 {
        return Some(signal.read(first));
    }
    let right = samples.get(upper).copied();
    let left = samples[upper - 1];
    let Some(right) = right else {
        return Some(signal.read(left));
    };
    if right.time_seconds == time_seconds {
        return Some(signal.read(right));
    }
    let fraction = (time_seconds - left.time_seconds) / (right.time_seconds - left.time_seconds);
    let value = signal.read(left) + fraction * (signal.read(right) - signal.read(left));
    value.is_finite().then_some(value)
}

fn integrate_product(
    samples: &[MeasurementSample],
    start: f64,
    end: f64,
    voltage_delay: f64,
) -> Option<f64> {
    if !start.is_finite() || !end.is_finite() || !voltage_delay.is_finite() || end <= start {
        return None;
    }
    let mut breakpoints = vec![start, end];
    breakpoints.extend(
        samples
            .iter()
            .map(|sample| sample.time_seconds)
            .filter(|time| *time > start && *time < end),
    );
    breakpoints.extend(
        samples
            .iter()
            .map(|sample| sample.time_seconds + voltage_delay)
            .filter(|time| *time > start && *time < end),
    );
    breakpoints.sort_by(f64::total_cmp);
    breakpoints.dedup_by(|left, right| *left == *right);
    let mut integral = 0.0;
    for pair in breakpoints.windows(2) {
        let left_time = pair[0];
        let right_time = pair[1];
        let v0 = interpolate(samples, left_time - voltage_delay, Signal::Voltage)?;
        let v1 = interpolate(samples, right_time - voltage_delay, Signal::Voltage)?;
        let i0 = interpolate(samples, left_time, Signal::Current)?;
        let i1 = interpolate(samples, right_time, Signal::Current)?;
        let duration = right_time - left_time;
        integral += if voltage_delay == 0.0 {
            // Use the same weights as RMS to keep real and apparent power consistent.
            duration * (v0 * i0 + v1 * i1) * 0.5
        } else {
            // The shifted integral supplies the reactive power sign.
            duration * (2.0 * v0 * i0 + v0 * i1 + v1 * i0 + 2.0 * v1 * i1) / 6.0
        };
    }
    let average = integral / (end - start);
    average.is_finite().then_some(average)
}

fn measured_frequency(
    samples: &[MeasurementSample],
    signal: Signal,
    threshold: f64,
    start: f64,
    end: f64,
    valid_since: f64,
) -> (f64, bool) {
    let start = start.max(samples.first().map_or(start, |sample| sample.time_seconds));
    let Some(points) = points_in_window(samples, start, end, signal) else {
        return (0.0, false);
    };
    let mut crossings = Vec::new();
    for pair in points.windows(2) {
        let (left_time, left_value) = pair[0];
        let (right_time, right_value) = pair[1];
        if left_time <= start.max(valid_since) || left_value >= threshold || right_value < threshold
        {
            continue;
        }
        let delta = right_value - left_value;
        if delta <= 0.0 {
            continue;
        }
        let fraction = (threshold - left_value) / delta;
        let crossing_time = left_time + fraction * (right_time - left_time);
        if crossing_time > start.max(valid_since) {
            crossings.push(crossing_time);
        }
    }
    if crossings.len() < 2 {
        return (0.0, false);
    }
    let previous = crossings[crossings.len() - 2];
    let last = crossings[crossings.len() - 1];
    let period = last - previous;
    if period > 0.0 && period.is_finite() {
        let frequency = 1.0 / period;
        (
            if frequency.is_finite() {
                frequency
            } else {
                0.0
            },
            frequency.is_finite(),
        )
    } else {
        (0.0, false)
    }
}

fn write_invalid(component: &ComponentSpec, result: &mut DeviceResult) {
    let keys: &[&str] = match component {
        ComponentSpec::AcVoltmeter { .. } => &[
            "rms_voltage",
            "mean_voltage",
            "peak_voltage",
            "frequency",
            "frequency_valid",
            "valid",
        ],
        ComponentSpec::AcAmmeter { .. } => &[
            "rms_current",
            "mean_current",
            "peak_current",
            "frequency",
            "frequency_valid",
            "valid",
        ],
        ComponentSpec::Wattmeter { .. } => &[
            "rms_voltage",
            "rms_current",
            "real_power",
            "reactive_power",
            "apparent_power",
            "power_factor",
            "phase_angle",
            "valid",
        ],
        ComponentSpec::ExposedPart { .. } => {
            &["touch_voltage_rms", "touch_voltage_peak", "hazard", "valid"]
        }
        _ => return,
    };
    for suffix in keys {
        set_extra(result, format!("{}/{}", component.id(), suffix), 0.0);
    }
}

fn set_extra(result: &mut DeviceResult, key: String, value: f64) {
    if let Some((_, existing)) = result
        .extra_values
        .iter_mut()
        .find(|(name, _)| *name == key)
    {
        *existing = value;
    } else {
        result.extra_values.push((key, value));
    }
}
