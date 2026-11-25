//! Transient-state handling and deterministic internal-step selection.

use crate::time_relay::{DelayMode, TimerState, delay_steps};
use std::collections::{HashMap, HashSet};

use crate::{
    ComponentSpec, DeviceState, Diagnostic, DiagnosticCode, FuseState, GeometryEnvelope,
    RelayContactState, SIMULATION_TIMESTEP_SECONDS, Schematic, StableId, SwitchPosition,
    devices::{
        BreakerState, DeviceResult, IntegrationMethod, OverloadState, RcdState, coil_contact_state,
        component_diagnostic, detector_step,
    },
    netlist::derive_netlist_with_open,
};

use super::{DeviceModel, measurements::MeasurementState};

type PublicStepOutcome = Result<
    (
        Vec<DeviceResult>,
        CircuitStates,
        CircuitStates,
        Vec<Diagnostic>,
    ),
    Vec<Diagnostic>,
>;

/// The phase of a periodic source, accumulated without multiplying the public
/// simulation time by its frequency.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct AccumulatedPhase {
    whole_cycles: u64,
    fractional_cycles: f64,
}

impl AccumulatedPhase {
    pub(super) fn advanced_by_cycles(self, cycles: f64) -> Option<Self> {
        if !cycles.is_finite() || cycles < 0.0 || !(0.0..1.0).contains(&self.fractional_cycles) {
            return None;
        }
        let whole = cycles.floor();
        if whole > u64::MAX as f64 {
            return None;
        }
        let whole_cycles = self.whole_cycles.checked_add(whole as u64)?;
        let fractional_cycles = self.fractional_cycles + cycles.fract();
        let carry = u64::from(fractional_cycles >= 1.0);
        Some(Self {
            whole_cycles: whole_cycles.checked_add(carry)?,
            fractional_cycles: fractional_cycles - carry as f64,
        })
    }

    pub(super) fn instantaneous_voltage(
        self,
        voltage_rms_volts: f64,
        frequency_hz: f64,
        phase_offset_degrees: f64,
        public_step_fraction: f64,
    ) -> Option<f64> {
        if !public_step_fraction.is_finite() || !(0.0..=1.0).contains(&public_step_fraction) {
            return None;
        }
        let within_step_cycles = frequency_hz * public_step_fraction * SIMULATION_TIMESTEP_SECONDS;
        let phase = self.advanced_by_cycles(within_step_cycles)?;
        let radians =
            std::f64::consts::TAU * phase.fractional_cycles + phase_offset_degrees.to_radians();
        let voltage = std::f64::consts::SQRT_2 * voltage_rms_volts * radians.sin();
        voltage.is_finite().then_some(voltage)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CapacitorState {
    pub(super) voltage_volts: f64,
    pub(super) current_amps: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct InductorState {
    pub(super) current_amps: f64,
    pub(super) voltage_volts: f64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RelayState {
    /// What the coil belongs to; each kind is reported as its own kind of device state.
    pub(super) kind: RelayKind,
    pub(super) detector: Option<f64>,
    pub(super) current_amps: f64,
    pub(super) voltage_volts: f64,
    pub(super) state: RelayContactState,
    pub(super) last_transition_time_seconds: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelayKind {
    Relay,
    Contactor,
    Solenoid,
    Valve,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FuseRuntimeState {
    pub(super) current_amps: f64,
    pub(super) accumulated_i2t_ampere_squared_seconds: f64,
    pub(super) state: FuseState,
    pub(super) last_transition_time_seconds: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MotorState {
    pub(super) armature_current_amps: f64,
    pub(super) angular_velocity_radians_per_second: f64,
    pub(super) voltage_volts: f64,
}

#[derive(Clone, Default)]
pub(super) struct CircuitStates {
    pub(super) solver_memory: crate::newton::SolverMemory,
    pub(super) capacitors: HashMap<StableId, CapacitorState>,
    pub(super) inductors: HashMap<StableId, InductorState>,
    pub(super) relays: HashMap<StableId, RelayState>,
    pub(super) timers: HashMap<StableId, (DelayMode, TimerState)>,
    /// The reported state of each lamp, indicator and buzzer, read from the settled endpoint.
    pub(super) indicators: HashMap<StableId, DeviceState>,
    /// Whether each frequency converter is enabled: its DC input reached the enable voltage.
    pub(super) converters: HashMap<StableId, bool>,
    pub(super) breakers: HashMap<StableId, BreakerState>,
    pub(super) overloads: HashMap<StableId, OverloadState>,
    pub(super) rcds: HashMap<StableId, RcdState>,
    pub(super) fuses: HashMap<StableId, FuseRuntimeState>,
    pub(super) motors: HashMap<StableId, MotorState>,
    pub(super) measurements: HashMap<StableId, MeasurementState>,
    /// Committed phase pairs are held at the start of each public 1 ms step.
    pub(super) ac_source_phases: HashMap<StableId, AccumulatedPhase>,
    /// Progress within the current public step, used by same-time event re-solves.
    pub(super) ac_source_step_fraction: f64,
    pub(super) simulation_time_seconds: f64,
    pub(super) backward_euler_next_step: bool,
}

impl CircuitStates {
    pub(super) fn device_states(&self) -> Vec<DeviceState> {
        let mut states = self
            .relays
            .iter()
            .map(|(component_id, relay)| {
                let (component_id, state) = (component_id.clone(), relay.state);
                let last_transition_time_seconds = relay.last_transition_time_seconds;
                match relay.kind {
                    RelayKind::Contactor => DeviceState::Contactor {
                        component_id,
                        state,
                        last_transition_time_seconds,
                    },
                    RelayKind::Relay => DeviceState::Relay {
                        component_id,
                        state,
                        last_transition_time_seconds,
                    },
                    RelayKind::Solenoid => DeviceState::Solenoid {
                        component_id,
                        energized: state == RelayContactState::Picked,
                    },
                    RelayKind::Valve => DeviceState::Valve {
                        component_id,
                        position: if state == RelayContactState::Picked {
                            SwitchPosition::Open
                        } else {
                            SwitchPosition::Closed
                        },
                    },
                }
            })
            .collect::<Vec<_>>();
        states.extend(self.timers.iter().map(|(id, (mode, timer))| {
            let component_id = id.clone();
            let energized = timer.energized;
            let output_on = timer.output_on;
            let timing = timer.timing();
            let last_transition_time_seconds = timer
                .transition_step
                .map(|step| step as f64 * SIMULATION_TIMESTEP_SECONDS);
            match mode {
                DelayMode::On => DeviceState::RelayOnDelay {
                    component_id,
                    energized,
                    output_on,
                    timing,
                    last_transition_time_seconds,
                },
                DelayMode::Off => DeviceState::RelayOffDelay {
                    component_id,
                    energized,
                    output_on,
                    timing,
                    last_transition_time_seconds,
                },
            }
        }));
        states.extend(self.indicators.values().cloned());
        states.extend(self.converters.iter().map(|(id, enabled)| {
            DeviceState::FrequencyConverter {
                component_id: id.clone(),
                enabled: *enabled,
            }
        }));
        states.extend(self.breakers.iter().map(|(id, state)| DeviceState::Switch {
            component_id: id.clone(),
            position: state.position,
            conducting: state.conducting(),
            last_transition_time_seconds: state.last_transition_time_seconds,
        }));
        states.extend(self.overloads.iter().map(|(id, state)| state.snapshot(id)));
        states.extend(self.rcds.iter().map(|(id, state)| DeviceState::Switch {
            component_id: id.clone(),
            position: state.position,
            conducting: state.conducting(),
            last_transition_time_seconds: state.last_transition_time_seconds,
        }));
        states.extend(
            self.fuses
                .iter()
                .map(|(component_id, fuse)| DeviceState::Fuse {
                    component_id: component_id.clone(),
                    state: fuse.state,
                    last_transition_time_seconds: fuse.last_transition_time_seconds,
                }),
        );
        states.sort_by(|left, right| {
            left.component_id()
                .as_str()
                .cmp(right.component_id().as_str())
        });
        states
    }
}

#[derive(Clone, Copy)]
pub(super) enum SolveMode<'a> {
    OperatingPoint,
    InitialState(&'a CircuitStates),
    Transient {
        step_seconds: f64,
        states: &'a CircuitStates,
        integration_method: IntegrationMethod,
        phase_advance_fraction: f64,
    },
}

impl<'a> SolveMode<'a> {
    pub(super) fn states(self) -> Option<&'a CircuitStates> {
        match self {
            Self::OperatingPoint => None,
            Self::InitialState(states) | Self::Transient { states, .. } => Some(states),
        }
    }
}

pub(super) fn circuit_states_from_results(
    schematic: &Schematic,
    results: &[DeviceResult],
    simulation_time_seconds: f64,
    previous: &CircuitStates,
) -> CircuitStates {
    let by_component = results
        .iter()
        .map(|result| (result.component_id.as_str(), result))
        .collect::<HashMap<_, _>>();
    let overloads = schematic
        .components
        .iter()
        .filter_map(|component| {
            let ComponentSpec::ThermalOverloadThreePole { id, .. } = component else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            let mut state = previous.overloads.get(id).copied().unwrap_or_default();
            state.peak_phase_amps = result.current_amps;
            Some((id.clone(), state))
        })
        .collect();
    let rcds = schematic
        .components
        .iter()
        .filter_map(|component| {
            let (ComponentSpec::RcdTwoPole { id, .. } | ComponentSpec::RcdFourPole { id, .. }) =
                component
            else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            let DeviceState::Switch {
                position,
                last_transition_time_seconds,
                ..
            } = result.state.as_ref()?
            else {
                return None;
            };
            let tripped = result
                .extra_values
                .iter()
                .any(|(key, value)| key.ends_with("/tripped") && *value == 1.0);
            // The detector and the dose are state of the run, carried from the committed
            // predecessor; the result only reports them.
            let carried = previous
                .rcds
                .get(id)
                .copied()
                .unwrap_or(RcdState::new(false));
            Some((
                id.clone(),
                RcdState {
                    position: *position,
                    tripped,
                    residual_amps: result.current_amps,
                    last_transition_time_seconds: *last_transition_time_seconds,
                    ..carried
                },
            ))
        })
        .collect();
    let capacitors = schematic
        .components
        .iter()
        .filter_map(|component| {
            let ComponentSpec::Capacitor { id, .. } = component else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            Some((
                id.clone(),
                CapacitorState {
                    voltage_volts: result.voltage_volts,
                    current_amps: result.current_amps,
                },
            ))
        })
        .collect();
    let inductors = schematic
        .components
        .iter()
        .filter_map(|component| {
            let ComponentSpec::Inductor { id, .. } = component else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            Some((
                id.clone(),
                InductorState {
                    current_amps: result.current_amps,
                    voltage_volts: result.voltage_volts,
                },
            ))
        })
        .collect();
    let relays = schematic
        .components
        .iter()
        .filter_map(|component| {
            let (ComponentSpec::RelaySpdt { id, .. }
            | ComponentSpec::Contactor { id, .. }
            | ComponentSpec::Solenoid { id, .. }
            | ComponentSpec::Valve { id, .. }
            | ComponentSpec::ContactorThreePole { id, .. }) = component
            else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            let (DeviceState::Relay {
                state,
                last_transition_time_seconds,
                ..
            }
            | DeviceState::Contactor {
                state,
                last_transition_time_seconds,
                ..
            }) = &result.state.as_ref()?
            else {
                return None;
            };
            Some((
                id.clone(),
                RelayState {
                    kind: match component {
                        ComponentSpec::Contactor { .. }
                        | ComponentSpec::ContactorThreePole { .. } => RelayKind::Contactor,
                        ComponentSpec::Solenoid { .. } => RelayKind::Solenoid,
                        ComponentSpec::Valve { .. } => RelayKind::Valve,
                        _ => RelayKind::Relay,
                    },
                    detector: result
                        .extra_values
                        .iter()
                        .find(|(key, _)| key.ends_with("/coil/current_squared"))
                        .map(|(_, value)| *value),
                    current_amps: result.current_amps,
                    voltage_volts: result.voltage_volts,
                    state: *state,
                    last_transition_time_seconds: *last_transition_time_seconds,
                },
            ))
        })
        .collect();
    let breakers = schematic
        .components
        .iter()
        .filter_map(|component| {
            let ComponentSpec::BreakerThreePole { id, .. } = component else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            let DeviceState::Switch {
                position,
                last_transition_time_seconds,
                ..
            } = result.state.as_ref()?
            else {
                return None;
            };
            let tripped = result
                .extra_values
                .iter()
                .any(|(key, value)| key.ends_with("/tripped") && *value == 1.0);
            // The thermal detector and dose are state of the run, carried from the committed
            // predecessor; the result only reports the dose. The pole currents sit at
            // `1, 3, 5` of the extra values.
            let carried = previous.breakers.get(id).copied();
            let peak_amps = [1, 3, 5]
                .into_iter()
                .filter_map(|index| result.extra_values.get(index))
                .map(|(_, value)| value.abs())
                .fold(0.0_f64, f64::max);
            Some((
                id.clone(),
                BreakerState {
                    position: *position,
                    tripped,
                    last_transition_time_seconds: *last_transition_time_seconds,
                    detector: carried.map_or(0.0, |state| state.detector),
                    dose: carried.map_or(0.0, |state| state.dose),
                    peak_amps,
                },
            ))
        })
        .collect();
    let fuses = schematic
        .components
        .iter()
        .filter_map(|component| {
            let ComponentSpec::Fuse { id, .. } = component else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            let Some(DeviceState::Fuse {
                state,
                last_transition_time_seconds,
                ..
            }) = &result.state
            else {
                return None;
            };
            let accumulated_i2t_ampere_squared_seconds = result
                .extra_values
                .iter()
                .find(|(key, _)| key == &format!("{id}/accumulated_i2t"))?
                .1;
            Some((
                id.clone(),
                FuseRuntimeState {
                    current_amps: result.current_amps,
                    accumulated_i2t_ampere_squared_seconds,
                    state: *state,
                    last_transition_time_seconds: *last_transition_time_seconds,
                },
            ))
        })
        .collect();
    let motors = schematic
        .components
        .iter()
        .filter_map(|component| {
            let ComponentSpec::PmdcMotor { id, .. } = component else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            let angular_velocity_radians_per_second = result
                .extra_values
                .iter()
                .find(|(key, _)| key == &format!("{id}/angular_velocity"))?
                .1;
            Some((
                id.clone(),
                MotorState {
                    armature_current_amps: result.current_amps,
                    angular_velocity_radians_per_second,
                    voltage_volts: result.voltage_volts,
                },
            ))
        })
        .collect();
    let ac_source_phases = schematic
        .components
        .iter()
        .filter_map(|component| {
            let id = match component {
                ComponentSpec::AcVoltageSource { id, .. }
                | ComponentSpec::ThreePhaseSourceStar { id, .. }
                | ComponentSpec::ThreePhaseSourceDelta { id, .. }
                | ComponentSpec::FrequencyConverter { id, .. } => id,
                _ => return None,
            };
            Some((id.clone(), AccumulatedPhase::default()))
        })
        .collect();
    CircuitStates {
        solver_memory: crate::newton::SolverMemory::default(),
        capacitors,
        inductors,
        relays,
        converters: results
            .iter()
            .filter_map(|result| match result.state.as_ref()? {
                DeviceState::FrequencyConverter { enabled, .. } => {
                    Some((result.component_id.clone(), *enabled))
                }
                _ => None,
            })
            .collect(),
        indicators: results
            .iter()
            .filter_map(|result| match result.state.as_ref()? {
                state @ (DeviceState::Lamp { .. }
                | DeviceState::Indicator { .. }
                | DeviceState::Buzzer { .. }
                | DeviceState::Led { .. }
                | DeviceState::ZenerDiode { .. }
                | DeviceState::MosfetN { .. }
                | DeviceState::OperationalAmplifier { .. }) => {
                    Some((result.component_id.clone(), state.clone()))
                }
                _ => None,
            })
            .collect(),
        timers: schematic
            .components
            .iter()
            .filter_map(|component| {
                let mode = match component {
                    ComponentSpec::RelayOnDelay { .. } => DelayMode::On,
                    ComponentSpec::RelayOffDelay { .. } => DelayMode::Off,
                    _ => return None,
                };
                let id = component.id();
                let timer = previous
                    .timers
                    .get(id)
                    .map_or(TimerState::default(), |(_, timer)| *timer);
                Some((id.clone(), (mode, timer)))
            })
            .collect(),
        breakers,
        overloads,
        rcds,
        fuses,
        motors,
        measurements: HashMap::new(),
        ac_source_phases,
        ac_source_step_fraction: 0.0,
        simulation_time_seconds,
        backward_euler_next_step: false,
    }
}

pub(super) fn advance_public_steps(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    open: &HashSet<StableId>,
    mut states: CircuitStates,
    internal_substeps: u32,
    first_public_step: u64,
    public_step_count: u32,
) -> PublicStepOutcome {
    let internal_step_seconds = SIMULATION_TIMESTEP_SECONDS / f64::from(internal_substeps);
    if !internal_step_seconds.is_finite() || internal_step_seconds <= 0.0 {
        return Err(vec![Diagnostic::error(
            DiagnosticCode::InvalidParameter,
            Vec::new(),
        )]);
    }

    let mut results = Vec::new();
    let mut diagnostics = Vec::new();
    let mut step_start_states = states.clone();
    for public_step_offset in 0..public_step_count {
        let Some(public_step_index) = first_public_step.checked_add(u64::from(public_step_offset))
        else {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        };
        let Some(previous_public_step) = public_step_index.checked_sub(1) else {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        };
        let public_step_start_time = previous_public_step as f64 * SIMULATION_TIMESTEP_SECONDS;
        let public_step_end_time = public_step_index as f64 * SIMULATION_TIMESTEP_SECONDS;
        if !public_step_start_time.is_finite() || !public_step_end_time.is_finite() {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        }
        states.simulation_time_seconds = public_step_start_time;
        step_start_states = states.clone();
        for internal_step_index in 0..internal_substeps {
            let step_end_time = if internal_step_index + 1 == internal_substeps {
                public_step_end_time
            } else {
                public_step_start_time + internal_step_seconds * f64::from(internal_step_index + 1)
            };
            if !step_end_time.is_finite() {
                return Err(vec![Diagnostic::error(
                    DiagnosticCode::NonFiniteState,
                    Vec::new(),
                )]);
            }
            let interval_start_states = states.clone();
            let integration_method = if states.backward_euler_next_step {
                IntegrationMethod::BackwardEuler
            } else {
                IntegrationMethod::Trapezoidal
            };
            let (mut step_results, step_diagnostics, memory) = super::compute_circuit(
                schematic,
                geometry,
                open,
                SolveMode::Transient {
                    step_seconds: internal_step_seconds,
                    states: &states,
                    integration_method,
                    phase_advance_fraction: f64::from(internal_step_index + 1)
                        / f64::from(internal_substeps),
                },
            )?;
            diagnostics.extend(step_diagnostics);
            let mut candidate =
                circuit_states_from_results(schematic, &step_results, step_end_time, &states);
            // The requested recovery method was consumed by this interval. A new
            // relay transition below can schedule another backward-Euler interval.
            candidate.solver_memory = memory;
            candidate.backward_euler_next_step = false;
            candidate.ac_source_phases = states.ac_source_phases.clone();
            candidate.ac_source_step_fraction =
                f64::from(internal_step_index + 1) / f64::from(internal_substeps);
            for component in &schematic.components {
                let ComponentSpec::ContactorThreePole { id, parameters, .. } = component else {
                    continue;
                };
                let Some(end) = candidate.relays.get_mut(id) else {
                    continue;
                };
                let Some(start) = interval_start_states.relays.get(id) else {
                    continue;
                };
                let Some(detector) = detector_step(
                    start.detector.unwrap_or(0.0),
                    start.current_amps,
                    end.current_amps,
                    internal_step_seconds,
                    parameters.coil_sense_seconds,
                ) else {
                    return Err(vec![component_diagnostic(
                        DiagnosticCode::NonFiniteState,
                        id,
                    )]);
                };
                end.detector = Some(detector);
                let result = step_results
                    .iter_mut()
                    .find(|result| result.component_id == *id)
                    .expect("contactor result");
                let (_, value) = result
                    .extra_values
                    .iter_mut()
                    .find(|(key, _)| key.ends_with("/coil/current_squared"))
                    .expect("contactor detector slot");
                *value = detector;
            }
            let mut trips = accumulate_fuse_i2t(
                schematic,
                &interval_start_states,
                &mut candidate,
                internal_step_seconds,
                step_end_time,
            )?;
            for component in &schematic.components {
                let ComponentSpec::ThermalOverloadThreePole { id, parameters, .. } = component
                else {
                    continue;
                };
                let (Some(start), Some(end)) = (
                    interval_start_states.overloads.get(id),
                    candidate.overloads.get_mut(id),
                ) else {
                    continue;
                };
                let Some(next) = end.advance(
                    start.peak_phase_amps,
                    end.peak_phase_amps,
                    internal_step_seconds,
                    step_end_time,
                    parameters,
                ) else {
                    return Err(vec![component_diagnostic(
                        DiagnosticCode::NonFiniteState,
                        id,
                    )]);
                };
                if next.tripped && !end.tripped {
                    trips.push(id.clone());
                }
                *end = next;
                let result = step_results
                    .iter_mut()
                    .find(|result| result.component_id == *id)
                    .expect("thermal overload result");
                result.extra_values.last_mut().expect("thermal dose slot").1 = next.dose;
            }
            for component in &schematic.components {
                let ComponentSpec::BreakerThreePole { id, parameters, .. } = component else {
                    continue;
                };
                if !parameters.thermal_magnetic() {
                    continue;
                }
                let (Some(start), Some(end)) = (
                    interval_start_states.breakers.get(id),
                    candidate.breakers.get_mut(id),
                ) else {
                    continue;
                };
                let Some(next) = end.advance_thermal(
                    start.peak_amps,
                    end.peak_amps,
                    internal_step_seconds,
                    step_end_time,
                    parameters,
                ) else {
                    return Err(vec![component_diagnostic(
                        DiagnosticCode::NonFiniteState,
                        id,
                    )]);
                };
                if next.tripped && !end.tripped {
                    trips.push(id.clone());
                }
                *end = next;
                let result = step_results
                    .iter_mut()
                    .find(|result| result.component_id == *id)
                    .expect("breaker result");
                let slots = result.extra_values.len();
                result.extra_values[slots - 2].1 = f64::from(next.tripped);
                result.extra_values[slots - 1].1 = next.dose;
            }
            for component in &schematic.components {
                let (ComponentSpec::RcdTwoPole { id, parameters, .. }
                | ComponentSpec::RcdFourPole { id, parameters, .. }) = component
                else {
                    continue;
                };
                let (Some(start), Some(end)) = (
                    interval_start_states.rcds.get(id),
                    candidate.rcds.get_mut(id),
                ) else {
                    continue;
                };
                let Some(next) = end.advance(
                    start.residual_amps,
                    end.residual_amps,
                    internal_step_seconds,
                    step_end_time,
                    parameters,
                ) else {
                    return Err(vec![component_diagnostic(
                        DiagnosticCode::NonFiniteState,
                        id,
                    )]);
                };
                if next.tripped && !end.tripped {
                    trips.push(id.clone());
                }
                *end = next;
                let result = step_results
                    .iter_mut()
                    .find(|result| result.component_id == *id)
                    .expect("rcd result");
                let slots = result.extra_values.len();
                result.extra_values[slots - 3].1 = f64::from(next.tripped);
                result.extra_values[slots - 2].1 = next.detector.sqrt();
                result.extra_values[slots - 1].1 = next.dose;
            }
            settle_discrete_events(
                CircuitInput {
                    schematic,
                    geometry,
                    open,
                },
                &mut candidate,
                &mut step_results,
                trips,
                step_end_time,
                (internal_step_index + 1 == internal_substeps).then_some(public_step_index),
                &mut diagnostics,
            )?;
            candidate.measurements = states.measurements.clone();
            super::measurements::update(
                schematic,
                &mut candidate.measurements,
                &mut step_results,
                step_end_time,
            )?;
            states = candidate;
            results = step_results;
        }
        states.ac_source_phases = advance_ac_source_phases(schematic, &step_start_states)?;
        states.ac_source_step_fraction = 0.0;
    }
    sort_diagnostics(&mut diagnostics);
    Ok((results, states, step_start_states, diagnostics))
}

fn advance_ac_source_phases(
    schematic: &Schematic,
    start_states: &CircuitStates,
) -> Result<HashMap<StableId, AccumulatedPhase>, Vec<Diagnostic>> {
    let mut phases = start_states.ac_source_phases.clone();
    for component in &schematic.components {
        let (id, frequency_hz) = match component {
            ComponentSpec::AcVoltageSource { id, parameters, .. } => (id, parameters.frequency_hz),
            ComponentSpec::ThreePhaseSourceStar { id, parameters, .. } => {
                (id, parameters.frequency_hz)
            }
            ComponentSpec::ThreePhaseSourceDelta { id, parameters, .. } => {
                (id, parameters.frequency_hz)
            }
            ComponentSpec::FrequencyConverter { id, parameters, .. } => {
                (id, parameters.frequency_hz)
            }
            _ => continue,
        };
        let Some(phase) = phases
            .get(id)
            .copied()
            .unwrap_or_default()
            .advanced_by_cycles(frequency_hz * SIMULATION_TIMESTEP_SECONDS)
        else {
            return Err(vec![component_diagnostic(
                DiagnosticCode::InvalidParameter,
                id,
            )]);
        };
        phases.insert(id.clone(), phase);
    }
    Ok(phases)
}

fn accumulate_fuse_i2t(
    schematic: &Schematic,
    previous_states: &CircuitStates,
    current_states: &mut CircuitStates,
    step_seconds: f64,
    event_time_seconds: f64,
) -> Result<Vec<StableId>, Vec<Diagnostic>> {
    let mut transitions = Vec::new();
    for component in &schematic.components {
        let ComponentSpec::Fuse { id, parameters, .. } = component else {
            continue;
        };
        let previous = previous_states.fuses.get(id);
        if previous.is_some_and(|state| state.state == FuseState::Tripped) {
            continue;
        }
        let old_current = previous.map_or(0.0, |state| state.current_amps);
        let Some(fuse) = current_states.fuses.get_mut(id) else {
            continue;
        };
        let increment = 0.5
            * step_seconds
            * (old_current * old_current + fuse.current_amps * fuse.current_amps);
        let accumulated = fuse.accumulated_i2t_ampere_squared_seconds + increment;
        if !increment.is_finite() || !accumulated.is_finite() {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::NonFiniteState,
                vec![crate::DiagnosticReference::Component {
                    component_id: id.clone(),
                }],
            )]);
        }
        fuse.accumulated_i2t_ampere_squared_seconds = accumulated;
        if fuse.state == FuseState::Intact
            && accumulated >= parameters.trip_i2t_ampere_squared_seconds
        {
            fuse.state = FuseState::Tripped;
            fuse.last_transition_time_seconds = Some(event_time_seconds);
            transitions.push(id.clone());
        }
    }
    transitions.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    Ok(transitions)
}

/// The circuit that is solved: the schematic, its pin geometry, and the components whose
/// branch a fault has removed.
#[derive(Clone, Copy)]
pub(super) struct CircuitInput<'a> {
    pub schematic: &'a Schematic,
    pub geometry: &'a GeometryEnvelope,
    pub open: &'a HashSet<StableId>,
}

pub(super) fn settle_discrete_events(
    circuit: CircuitInput<'_>,
    states: &mut CircuitStates,
    results: &mut Vec<DeviceResult>,
    mut pending_trips: Vec<StableId>,
    event_time_seconds: f64,
    timer_step: Option<u64>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let CircuitInput {
        schematic,
        geometry,
        open,
    } = circuit;
    const MAX_SAME_TIME_TRANSITIONS: usize = 16;
    let mut seen = HashSet::new();
    seen.insert(discrete_state_vector(states, &pending_trips));
    let mut same_time_resolves = 0;
    loop {
        let mut relay_changes = Vec::new();
        let mut breaker_changes = Vec::new();
        for component in &schematic.components {
            let id = component.id();
            if let ComponentSpec::BreakerThreePole { parameters, .. } = component {
                let Some(state) = states.breakers.get(id).copied() else {
                    continue;
                };
                if !state.conducting() {
                    continue;
                }
                let result = results
                    .iter()
                    .find(|result| result.component_id == *id)
                    .expect("breaker result");
                let currents = [1, 3, 5].map(|index| result.extra_values[index].1);
                let next = state.trip_if_needed(
                    currents,
                    parameters.magnetic_threshold_amps(),
                    event_time_seconds,
                );
                if next != state {
                    breaker_changes.push((id.clone(), next));
                }
                continue;
            }
            let Some(relay) = states.relays.get(id) else {
                continue;
            };
            let next_state = match component {
                ComponentSpec::RelaySpdt { parameters, .. }
                | ComponentSpec::Contactor { parameters, .. } => match relay.state {
                    RelayContactState::Released
                        if relay.current_amps.abs() >= parameters.pickup_current_amps =>
                    {
                        RelayContactState::Picked
                    }
                    RelayContactState::Picked
                        if relay.current_amps.abs() <= parameters.dropout_current_amps =>
                    {
                        RelayContactState::Released
                    }
                    _ => relay.state,
                },
                ComponentSpec::Solenoid { parameters, .. }
                | ComponentSpec::Valve { parameters, .. } => match relay.state {
                    RelayContactState::Released
                        if relay.current_amps.abs() >= parameters.pickup_current_amps =>
                    {
                        RelayContactState::Picked
                    }
                    RelayContactState::Picked
                        if relay.current_amps.abs() <= parameters.dropout_current_amps =>
                    {
                        RelayContactState::Released
                    }
                    _ => relay.state,
                },
                ComponentSpec::ContactorThreePole { parameters, .. } => coil_contact_state(
                    relay.state,
                    relay.detector.unwrap_or(0.0),
                    parameters.pickup_current_amps,
                    parameters.dropout_current_amps,
                ),
                _ => continue,
            };
            if next_state != relay.state {
                relay_changes.push((id.clone(), next_state));
            }
        }
        let mut timer_changes = Vec::new();
        for component in &schematic.components {
            let (ComponentSpec::RelayOnDelay { parameters, .. }
            | ComponentSpec::RelayOffDelay { parameters, .. }) = component
            else {
                continue;
            };
            let id = component.id();
            let Some((mode, timer)) = states.timers.get_mut(id) else {
                continue;
            };
            let result = results
                .iter_mut()
                .find(|result| result.component_id == *id)
                .expect("timer coil result");
            if let Some(step) = timer_step.filter(|_| {
                relay_changes.is_empty() && breaker_changes.is_empty() && pending_trips.is_empty()
            }) && timer.sample(
                *mode,
                result.current_amps,
                parameters.pickup_current_amps,
                parameters.dropout_current_amps,
                delay_steps(parameters.delay_seconds),
                step,
            ) {
                timer_changes.push(id.clone());
            }
            let step = timer_step.unwrap_or_else(|| {
                (event_time_seconds / SIMULATION_TIMESTEP_SECONDS).floor() as u64
            });
            let elapsed = timer.elapsed_steps(step) as f64 * SIMULATION_TIMESTEP_SECONDS;
            let slots = crate::slots::component_slot_definitions(component);
            result.extra_values.truncate(6);
            for ((key, _), slot) in result.extra_values.iter_mut().zip(&slots) {
                *key = slot.key(id.as_str());
            }
            result
                .extra_values
                .push((slots[6].key(id.as_str()), elapsed));
        }
        // A converter is enabled while its DC input is at or above the enable voltage, judged on
        // the settled endpoint; a change is one output event.
        let mut converter_changes = Vec::new();
        for component in &schematic.components {
            let ComponentSpec::FrequencyConverter { id, parameters, .. } = component else {
                continue;
            };
            let Some(result) = results.iter().find(|result| result.component_id == *id) else {
                continue;
            };
            let next = result.voltage_volts >= parameters.enable_volts;
            if states.converters.get(id) != Some(&next) {
                converter_changes.push((id.clone(), next));
            }
        }
        if relay_changes.is_empty()
            && timer_changes.is_empty()
            && breaker_changes.is_empty()
            && converter_changes.is_empty()
            && pending_trips.is_empty()
        {
            return Ok(());
        }
        relay_changes.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
        let mut changed_ids = relay_changes
            .iter()
            .map(|(component_id, _)| component_id.clone())
            .collect::<Vec<_>>();
        changed_ids.extend(breaker_changes.iter().map(|(id, _)| id.clone()));
        changed_ids.extend(timer_changes);
        changed_ids.extend(converter_changes.iter().map(|(id, _)| id.clone()));
        changed_ids.append(&mut pending_trips);
        changed_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        changed_ids.dedup();
        if same_time_resolves >= MAX_SAME_TIME_TRANSITIONS {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::EventChatter,
                changed_ids
                    .into_iter()
                    .map(|component_id| crate::DiagnosticReference::Component { component_id })
                    .collect(),
            ));
            sort_diagnostics(diagnostics);
            return Err(diagnostics.clone());
        }
        for (component_id, next_state) in relay_changes {
            let relay = states
                .relays
                .get_mut(&component_id)
                .expect("a proposed relay transition has a state");
            relay.state = next_state;
            relay.last_transition_time_seconds = Some(event_time_seconds);
        }
        breaker_changes.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        states.breakers.extend(breaker_changes);
        states.converters.extend(converter_changes);
        pending_trips = Vec::new();
        states.backward_euler_next_step = true;
        let state_vector = discrete_state_vector(states, &[]);
        if !seen.insert(state_vector) {
            diagnostics.push(Diagnostic::error(
                DiagnosticCode::EventChatter,
                changed_ids
                    .into_iter()
                    .map(|component_id| crate::DiagnosticReference::Component { component_id })
                    .collect(),
            ));
            sort_diagnostics(diagnostics);
            return Err(diagnostics.clone());
        }
        let (resolved, event_diagnostics, memory) =
            super::compute_circuit(schematic, geometry, open, SolveMode::InitialState(states))?;
        *results = resolved;
        diagnostics.extend(event_diagnostics);
        let mut resolved_states =
            circuit_states_from_results(schematic, results, event_time_seconds, states);
        // The event state is the state vector used to solve this reconfiguration; the
        // device results carry that same vector and its exact transition times.
        resolved_states.solver_memory = memory;
        resolved_states.simulation_time_seconds = event_time_seconds;
        resolved_states.ac_source_phases = states.ac_source_phases.clone();
        resolved_states.ac_source_step_fraction = states.ac_source_step_fraction;
        resolved_states.backward_euler_next_step = states.backward_euler_next_step;
        *states = resolved_states;
        same_time_resolves += 1;
    }
}

fn discrete_state_vector(states: &CircuitStates, pending_trips: &[StableId]) -> Vec<(String, u8)> {
    let mut vector = states
        .relays
        .iter()
        .map(|(id, relay)| {
            (
                id.as_str().to_owned(),
                u8::from(relay.state == RelayContactState::Picked),
            )
        })
        .collect::<Vec<_>>();
    vector.extend(states.timers.iter().map(|(id, (_, timer))| {
        (
            id.as_str().to_owned(),
            u8::from(timer.output_on) | (u8::from(timer.energized) << 1),
        )
    }));
    vector.extend(
        states
            .converters
            .iter()
            .map(|(id, enabled)| (id.as_str().to_owned(), u8::from(*enabled))),
    );
    vector.extend(states.fuses.iter().map(|(id, fuse)| {
        let pending = pending_trips.iter().any(|pending| pending == id);
        (
            id.as_str().to_owned(),
            u8::from(!pending && fuse.state == FuseState::Tripped),
        )
    }));
    vector.extend(states.breakers.iter().map(|(id, state)| {
        let pending = pending_trips.contains(id);
        (id.as_str().to_owned(), u8::from(!pending && state.tripped))
    }));
    vector.extend(states.overloads.iter().map(|(id, state)| {
        let pending = pending_trips.contains(id);
        (id.as_str().to_owned(), u8::from(!pending && state.tripped))
    }));
    vector.extend(states.rcds.iter().map(|(id, state)| {
        let pending = pending_trips.contains(id);
        (id.as_str().to_owned(), u8::from(!pending && state.tripped))
    }));
    vector.sort_by(|left, right| left.0.cmp(&right.0));
    vector
}

pub(super) fn choose_internal_substeps(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    open: &HashSet<StableId>,
) -> (u32, bool) {
    const MAX_SUBSTEPS: u32 = 64;
    let source_rate = schematic
        .components
        .iter()
        .filter(|component| !open.contains(component.id()))
        .filter_map(|component| match component {
            ComponentSpec::AcVoltageSource { parameters, .. } => {
                Some(std::f64::consts::TAU * parameters.frequency_hz)
            }
            ComponentSpec::ThreePhaseSourceStar { parameters, .. } => {
                Some(std::f64::consts::TAU * parameters.frequency_hz)
            }
            ComponentSpec::ThreePhaseSourceDelta { parameters, .. } => {
                Some(std::f64::consts::TAU * parameters.frequency_hz)
            }
            ComponentSpec::FrequencyConverter { parameters, .. } => {
                Some(std::f64::consts::TAU * parameters.frequency_hz)
            }
            _ => None,
        })
        .fold(0.0_f64, f64::max);
    let omega_hat = estimate_transient_rate_bound(schematic, geometry, open)
        .unwrap_or(f64::MAX)
        .max(source_rate);
    let limit = 0.03_f64.sqrt();
    let mut substeps = 1;
    while substeps < MAX_SUBSTEPS
        && omega_hat * (SIMULATION_TIMESTEP_SECONDS / f64::from(substeps)) > limit
    {
        substeps *= 2;
    }
    let limited = omega_hat * (SIMULATION_TIMESTEP_SECONDS / f64::from(substeps)) > limit;
    (substeps, limited)
}

/// Bounds the RC/RL/RLC natural rates after eliminating algebraic nodes.
/// Ideal voltage sources collapse nodes in the small-signal circuit. The
/// energy-scaled state matrix uses a diagonal similarity transform; its
/// infinity norm bounds the magnitudes of all natural rates. Singular
/// reductions deliberately return no estimate and select the documented
/// 64-step limit with a warning.
fn estimate_transient_rate_bound(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    open: &HashSet<StableId>,
) -> Option<f64> {
    let netlist = derive_netlist_with_open(schematic, geometry, open).ok()?;
    let specs = super::build_device_specs(schematic, &netlist, SolveMode::OperatingPoint)
        .ok()?
        .into_iter()
        .filter(|spec| !open.contains(&spec.component_id))
        .collect::<Vec<_>>();
    let mut maximum_rate: f64 = 0.0;
    for spec in &specs {
        if let DeviceModel::RelaySpdt {
            coil_resistance_ohms,
            coil_inductance_henries,
            ..
        }
        | DeviceModel::Contactor {
            coil_resistance_ohms,
            coil_inductance_henries,
            ..
        } = spec.model
        {
            maximum_rate = maximum_rate.max(coil_resistance_ohms / coil_inductance_henries);
        }
        if let DeviceModel::PmdcMotor {
            armature_resistance_ohms,
            armature_inductance_henries,
            motor_constant_newton_meters_per_amp,
            rotor_inertia_kilogram_meters_squared,
            viscous_friction_newton_meter_seconds_per_radian,
            ..
        } = spec.model
        {
            let coupling_rate = motor_constant_newton_meters_per_amp
                / (armature_inductance_henries * rotor_inertia_kilogram_meters_squared).sqrt();
            let electrical_rate = armature_resistance_ohms / armature_inductance_henries;
            let mechanical_rate = viscous_friction_newton_meter_seconds_per_radian
                / rotor_inertia_kilogram_meters_squared;
            maximum_rate = maximum_rate.max(electrical_rate + coupling_rate);
            maximum_rate = maximum_rate.max(mechanical_rate + coupling_rate);
        }
    }

    for island in &netlist.islands {
        let component_ids = island
            .component_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<HashSet<_>>();
        let island_specs = specs
            .iter()
            .filter(|spec| component_ids.contains(spec.component_id.as_str()))
            .collect::<Vec<_>>();
        if island_specs.is_empty() {
            continue;
        }
        let reference_node = island.reference_node?;
        let mut sets = DisjointSets::new(netlist.nodes.len());
        for spec in &island_specs {
            if matches!(
                spec.model,
                DeviceModel::DcVoltageSource {
                    internal_resistance_ohms: 0.0,
                    ..
                }
            ) {
                sets.union(spec.terminals.pin1, spec.terminals.pin2);
            }
            if let DeviceModel::ThreePhaseSource {
                windings,
                internal_resistance_ohms: 0.0,
                star: true,
                ..
            } = spec.model
            {
                for winding in windings {
                    sets.union(winding.pin1, winding.pin2);
                }
            }
        }

        let includes_windings = island_specs
            .iter()
            .any(|spec| matches!(spec.model, DeviceModel::Windings(_)));
        let mut reference_roots = HashSet::from([sets.find(reference_node)]);
        if includes_windings {
            reference_roots.extend(
                super::local_reference_nodes(
                    &island_specs,
                    island,
                    SolveMode::OperatingPoint,
                    netlist.nodes.len(),
                    reference_node,
                )
                .into_iter()
                .map(|node| sets.find(node)),
            );
        }
        let mut root_to_variable = HashMap::new();
        for node in island.node_indices.iter().copied() {
            let root = sets.find(node);
            if !reference_roots.contains(&root) {
                let next = root_to_variable.len();
                root_to_variable.entry(root).or_insert(next);
            }
        }
        let dimension = root_to_variable.len();
        let induction_windings = island_specs
            .iter()
            .filter_map(|spec| match spec.model {
                DeviceModel::InductionMotor {
                    windings,
                    parameters,
                } => Some((windings, parameters)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut conductance = vec![vec![0.0; dimension]; dimension];
        let mut capacitance = vec![vec![0.0; dimension]; dimension];
        let inductor_specs = island_specs
            .iter()
            .filter(|spec| {
                matches!(
                    spec.model,
                    DeviceModel::Inductor { .. } | DeviceModel::PmdcMotor { .. }
                )
            })
            .copied()
            .collect::<Vec<_>>();
        let winding_branches = island_specs
            .iter()
            .filter_map(|spec| match spec.model {
                DeviceModel::Windings(network) => Some((spec.component_id.clone(), network)),
                _ => None,
            })
            .flat_map(|(id, network)| {
                (0..network.windings().len()).filter_map(move |i| {
                    let winding = network.windings()[i];
                    (winding.inductance > 0.0).then(|| (id.clone(), network, i, winding))
                })
            })
            .collect::<Vec<_>>();
        if !winding_branches.is_empty()
            && schematic.components.iter().any(|component| {
                island.component_ids.contains(component.id())
                    && matches!(
                        component,
                        ComponentSpec::SwitchSpst { .. }
                            | ComponentSpec::PushButtonNo { .. }
                            | ComponentSpec::PushButtonNc { .. }
                            | ComponentSpec::Fuse { .. }
                            | ComponentSpec::RelaySpdt { .. }
                            | ComponentSpec::Contactor { .. }
                            | ComponentSpec::ContactorThreePole { .. }
                            | ComponentSpec::BreakerThreePole { .. }
                            | ComponentSpec::ThermalOverloadThreePole { .. }
                            | ComponentSpec::RcdTwoPole { .. }
                            | ComponentSpec::RcdFourPole { .. }
                            | ComponentSpec::Diode { .. }
                            | ComponentSpec::Led { .. }
                            | ComponentSpec::ZenerDiode { .. }
                            | ComponentSpec::MosfetN { .. }
                            | ComponentSpec::OperationalAmplifier { .. }
                            | ComponentSpec::BjtNpn { .. }
                            | ComponentSpec::BjtPnp { .. }
                    )
            })
        {
            return None;
        }
        let mut incidence =
            vec![vec![0.0; inductor_specs.len() + winding_branches.len()]; dimension];

        for spec in &island_specs {
            let (pin1, pin2) = (
                sets.find(spec.terminals.pin1),
                sets.find(spec.terminals.pin2),
            );
            let variable1 = root_to_variable.get(&pin1).copied();
            let variable2 = root_to_variable.get(&pin2).copied();
            match spec.model {
                DeviceModel::Selector {
                    throws, resistance, ..
                } => {
                    for terminals in throws {
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(terminals.pin1)).copied(),
                            root_to_variable.get(&sets.find(terminals.pin2)).copied(),
                            1.0 / resistance,
                        )?;
                    }
                }
                DeviceModel::TimeRelay {
                    coil_resistance,
                    contact_resistance,
                    poles,
                } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / coil_resistance,
                    )?;
                    for pole in poles {
                        let terminals = pole.terminals;
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(terminals.pin1)).copied(),
                            root_to_variable.get(&sets.find(terminals.pin2)).copied(),
                            1.0 / contact_resistance,
                        )?;
                    }
                }
                DeviceModel::FrequencyConverter {
                    input_resistance,
                    outputs,
                    output_resistance,
                    ..
                } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / input_resistance,
                    )?;
                    for output in outputs {
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(output.pin1)).copied(),
                            root_to_variable.get(&sets.find(output.pin2)).copied(),
                            1.0 / output_resistance,
                        )?;
                    }
                }
                DeviceModel::Resistor { resistance_ohms }
                | DeviceModel::Indicator {
                    resistance_ohms, ..
                } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / resistance_ohms,
                    )?;
                }
                DeviceModel::ExposedPart { .. } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / crate::engine::EXPOSED_PART_RESISTANCE_OHMS,
                    )?;
                }
                DeviceModel::Wattmeter {
                    current_terminals,
                    input_resistance_ohms,
                    burden_resistance_ohms,
                } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / input_resistance_ohms,
                    )?;
                    add_laplacian(
                        &mut conductance,
                        root_to_variable
                            .get(&sets.find(current_terminals.pin1))
                            .copied(),
                        root_to_variable
                            .get(&sets.find(current_terminals.pin2))
                            .copied(),
                        1.0 / burden_resistance_ohms,
                    )?;
                }
                DeviceModel::Fuse {
                    intact_resistance_ohms,
                    ..
                } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / intact_resistance_ohms,
                    )?;
                }
                DeviceModel::DcVoltageSource {
                    internal_resistance_ohms,
                    ..
                } if internal_resistance_ohms > 0.0 => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / internal_resistance_ohms,
                    )?;
                }
                DeviceModel::ThreePhaseSource {
                    windings,
                    internal_resistance_ohms,
                    ..
                } if internal_resistance_ohms > 0.0 => {
                    for winding in windings {
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(winding.pin1)).copied(),
                            root_to_variable.get(&sets.find(winding.pin2)).copied(),
                            1.0 / internal_resistance_ohms,
                        )?;
                    }
                }
                DeviceModel::Capacitor {
                    capacitance_farads, ..
                } => {
                    add_laplacian(&mut capacitance, variable1, variable2, capacitance_farads)?;
                }
                DeviceModel::Inductor { .. } => {}
                DeviceModel::PmdcMotor { .. } => {}
                DeviceModel::RelaySpdt {
                    coil_resistance_ohms,
                    closed_contact_resistance_ohms,
                    common_node,
                    normally_open_node,
                    normally_closed_node,
                    ..
                } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / coil_resistance_ohms,
                    )?;
                    add_laplacian(
                        &mut conductance,
                        root_to_variable.get(&sets.find(common_node)).copied(),
                        root_to_variable
                            .get(&sets.find(normally_open_node))
                            .copied(),
                        1.0 / closed_contact_resistance_ohms,
                    )?;
                    add_laplacian(
                        &mut conductance,
                        root_to_variable.get(&sets.find(common_node)).copied(),
                        root_to_variable
                            .get(&sets.find(normally_closed_node))
                            .copied(),
                        1.0 / closed_contact_resistance_ohms,
                    )?;
                }
                DeviceModel::Contactor {
                    coil_resistance_ohms,
                    closed_contact_resistance_ohms,
                    poles,
                    ..
                } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / coil_resistance_ohms,
                    )?;
                    for pole in poles.as_slice() {
                        let contact = pole.terminals;
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(contact.pin1)).copied(),
                            root_to_variable.get(&sets.find(contact.pin2)).copied(),
                            1.0 / closed_contact_resistance_ohms,
                        )?;
                    }
                }
                DeviceModel::Breaker {
                    resistance, poles, ..
                } => {
                    for pole in poles {
                        let terminals = pole.terminals;
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(terminals.pin1)).copied(),
                            root_to_variable.get(&sets.find(terminals.pin2)).copied(),
                            1.0 / resistance,
                        )?;
                    }
                }
                DeviceModel::Rcd {
                    resistance,
                    poles,
                    pole_count,
                    ..
                } => {
                    for pole in &poles[..pole_count] {
                        let terminals = pole.terminals;
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(terminals.pin1)).copied(),
                            root_to_variable.get(&sets.find(terminals.pin2)).copied(),
                            1.0 / resistance,
                        )?;
                    }
                }
                DeviceModel::Overload {
                    poles,
                    auxiliaries,
                    sense_resistance,
                    contact_resistance,
                } => {
                    for (pole, resistance) in poles
                        .iter()
                        .map(|pole| (pole, sense_resistance))
                        .chain(auxiliaries.iter().map(|pole| (pole, contact_resistance)))
                    {
                        let terminals = pole.terminals;
                        add_laplacian(
                            &mut conductance,
                            root_to_variable.get(&sets.find(terminals.pin1)).copied(),
                            root_to_variable.get(&sets.find(terminals.pin2)).copied(),
                            1.0 / resistance,
                        )?;
                    }
                }
                DeviceModel::InductionMotor { parameters, .. } => {
                    maximum_rate = maximum_rate.max(parameters.fixed_voltage_rate_bound());
                }
                DeviceModel::Windings(network) => {
                    for winding in network.windings() {
                        if winding.inductance == 0.0 {
                            let left = root_to_variable
                                .get(&sets.find(winding.terminals.pin1))
                                .copied();
                            let right = root_to_variable
                                .get(&sets.find(winding.terminals.pin2))
                                .copied();
                            add_laplacian(
                                &mut conductance,
                                left,
                                right,
                                winding.resistance.recip(),
                            )?;
                        }
                    }
                }
                DeviceModel::DcVoltageSource { .. }
                | DeviceModel::ThreePhaseSource { .. }
                | DeviceModel::Diode { .. }
                | DeviceModel::Led { .. }
                | DeviceModel::Zener { .. }
                | DeviceModel::Mosfet { .. }
                | DeviceModel::OpAmp { .. }
                | DeviceModel::Bjt { .. } => {}
            }
        }
        if induction_windings.iter().any(|(windings, _)| {
            windings
                .iter()
                .any(|winding| sets.find(winding.pin1) != sets.find(winding.pin2))
        }) {
            // For a static linear network eliminate the node voltages and
            // bound the extra winding-voltage derivatives from its driving
            // resistance. Mixed dynamic and switching networks fall back to
            // the cap when a joint enclosure cannot be established.
            if island_specs.iter().any(|spec| {
                !matches!(
                    spec.model,
                    DeviceModel::InductionMotor { .. }
                        | DeviceModel::Resistor { .. }
                        | DeviceModel::Indicator { .. }
                        | DeviceModel::FrequencyConverter { .. }
                        | DeviceModel::ExposedPart { .. }
                        | DeviceModel::DcVoltageSource { .. }
                        | DeviceModel::ThreePhaseSource { .. }
                        | DeviceModel::Wattmeter { .. }
                ) || schematic.components.iter().any(|component| {
                    component.id() == &spec.component_id
                        && matches!(
                            component,
                            ComponentSpec::SwitchSpst { .. }
                                | ComponentSpec::PushButtonNo { .. }
                                | ComponentSpec::PushButtonNc { .. }
                        )
                })
            }) {
                return None;
            }
            let columns = induction_windings.len() * 3;
            let mut branches = vec![vec![0.0; columns]; dimension];
            for (index, winding) in induction_windings
                .iter()
                .flat_map(|(windings, _)| windings)
                .enumerate()
            {
                let left = root_to_variable.get(&sets.find(winding.pin1)).copied();
                let right = root_to_variable.get(&sets.find(winding.pin2)).copied();
                add_incidence(&mut branches, left, right, index)?;
            }
            let inverse = invert_matrix(conductance.clone())?;
            let response = multiply_matrices(&inverse, &branches)?;
            let driving = multiply_matrices(&transpose(&branches), &response)?;
            let resistance_norm = driving
                .iter()
                .map(|row| row.iter().map(|value| value.abs()).sum::<f64>())
                .fold(0.0_f64, f64::max);
            let current_gain = induction_windings
                .iter()
                .map(|(_, parameters)| parameters.phase_current_gain())
                .fold(0.0_f64, f64::max);
            // The L1 norm of any power-invariant Clarke row is at most √3.
            maximum_rate += 3.0_f64.sqrt() * resistance_norm * current_gain / 100.0;
            if !maximum_rate.is_finite() {
                return None;
            }
        }
        for (branch, spec) in inductor_specs.iter().enumerate() {
            let pin1 = root_to_variable
                .get(&sets.find(spec.terminals.pin1))
                .copied();
            let pin2 = root_to_variable
                .get(&sets.find(spec.terminals.pin2))
                .copied();
            add_incidence(&mut incidence, pin1, pin2, branch)?;
        }
        for (i, (_, _, _, winding)) in winding_branches.iter().enumerate() {
            let left = root_to_variable
                .get(&sets.find(winding.terminals.pin1))
                .copied();
            let right = root_to_variable
                .get(&sets.find(winding.terminals.pin2))
                .copied();
            add_incidence(&mut incidence, left, right, inductor_specs.len() + i)?;
        }

        let dynamic = (0..dimension)
            .filter(|index| capacitance[*index][*index] > 0.0)
            .collect::<Vec<_>>();
        let algebraic = (0..dimension)
            .filter(|index| capacitance[*index][*index] == 0.0)
            .collect::<Vec<_>>();
        let mut l_values = inductor_specs
            .iter()
            .map(|spec| match spec.model {
                DeviceModel::Inductor {
                    inductance_henries, ..
                } => Some(inductance_henries),
                DeviceModel::PmdcMotor {
                    armature_inductance_henries,
                    ..
                } => Some(armature_inductance_henries),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        l_values.extend(
            winding_branches
                .iter()
                .map(|(_, _, _, winding)| winding.inductance),
        );
        let motor_branches = inductor_specs
            .iter()
            .enumerate()
            .filter_map(|(branch, spec)| match spec.model {
                DeviceModel::PmdcMotor {
                    armature_resistance_ohms,
                    motor_constant_newton_meters_per_amp,
                    rotor_inertia_kilogram_meters_squared,
                    viscous_friction_newton_meter_seconds_per_radian,
                    ..
                } => Some((
                    branch,
                    armature_resistance_ohms,
                    motor_constant_newton_meters_per_amp,
                    rotor_inertia_kilogram_meters_squared,
                    viscous_friction_newton_meter_seconds_per_radian,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        if dynamic.is_empty() && l_values.is_empty() {
            continue;
        }

        let gdd = submatrix(&conductance, &dynamic, &dynamic);
        let b_dynamic = submatrix_columns(&incidence, &dynamic);
        let b_algebraic = submatrix_columns(&incidence, &algebraic);
        let mut reduced_g = gdd;
        let mut reduced_b = b_dynamic;
        let mut reduced_inductor_resistance = vec![vec![0.0; l_values.len()]; l_values.len()];
        if !algebraic.is_empty() {
            let gaa = submatrix(&conductance, &algebraic, &algebraic);
            let gad = submatrix(&conductance, &algebraic, &dynamic);
            let gda = submatrix(&conductance, &dynamic, &algebraic);
            let inverse_gaa = invert_matrix(gaa)?;
            let eliminated = multiply_matrices(&multiply_matrices(&gda, &inverse_gaa)?, &gad)?;
            for row in 0..reduced_g.len() {
                for column in 0..reduced_g.len() {
                    reduced_g[row][column] -= eliminated[row][column];
                }
            }
            let eliminated_b =
                multiply_matrices(&multiply_matrices(&gda, &inverse_gaa)?, &b_algebraic)?;
            for row in 0..reduced_b.len() {
                for column in 0..l_values.len() {
                    reduced_b[row][column] -= eliminated_b[row][column];
                }
            }
            let inverse_b = multiply_matrices(&inverse_gaa, &b_algebraic)?;
            reduced_inductor_resistance = multiply_matrices(&transpose(&b_algebraic), &inverse_b)?;
        }
        for (branch, armature_resistance_ohms, ..) in &motor_branches {
            reduced_inductor_resistance[*branch][*branch] += *armature_resistance_ohms;
        }
        for (i, (_, _, _, winding)) in winding_branches.iter().enumerate() {
            let branch = inductor_specs.len() + i;
            reduced_inductor_resistance[branch][branch] += winding.resistance;
        }
        let inverse_inductance = if winding_branches.is_empty() {
            None
        } else {
            let mut matrix = vec![vec![0.0; l_values.len()]; l_values.len()];
            for (i, value) in l_values.iter().copied().enumerate() {
                matrix[i][i] = value;
            }
            for (i, (id, network, row, _)) in winding_branches.iter().enumerate() {
                for (j, (other, _, column, _)) in winding_branches.iter().enumerate() {
                    if id == other && row != column {
                        matrix[inductor_specs.len() + i][inductor_specs.len() + j] =
                            network.inductance(*row, *column);
                    }
                }
            }
            Some(invert_matrix(matrix)?)
        };

        let motor_state_start = dynamic.len() + l_values.len();
        let mut state_matrix = vec![
            vec![0.0; motor_state_start + motor_branches.len()];
            motor_state_start + motor_branches.len()
        ];
        if !dynamic.is_empty() {
            let reduced_c = submatrix(&capacitance, &dynamic, &dynamic);
            let inverse_c = invert_matrix(reduced_c)?;
            let voltage_g = multiply_matrices(&inverse_c, &reduced_g)?;
            let voltage_b = multiply_matrices(&inverse_c, &reduced_b)?;
            for row in 0..dynamic.len() {
                for column in 0..dynamic.len() {
                    state_matrix[row][column] = -voltage_g[row][column];
                }
                for column in 0..l_values.len() {
                    state_matrix[row][dynamic.len() + column] = -voltage_b[row][column];
                }
            }
            let mut branch_voltage_b = transpose(&reduced_b);
            if let Some(inverse) = &inverse_inductance {
                branch_voltage_b = multiply_matrices(inverse, &branch_voltage_b)?;
            }
            for row in 0..l_values.len() {
                for column in 0..dynamic.len() {
                    state_matrix[dynamic.len() + row][column] = if inverse_inductance.is_some() {
                        branch_voltage_b[row][column]
                    } else {
                        branch_voltage_b[row][column] / l_values[row]
                    };
                }
            }
        }
        if let Some(inverse) = &inverse_inductance {
            reduced_inductor_resistance = multiply_matrices(inverse, &reduced_inductor_resistance)?;
        }
        for row in 0..l_values.len() {
            for column in 0..l_values.len() {
                state_matrix[dynamic.len() + row][dynamic.len() + column] =
                    if inverse_inductance.is_some() {
                        -reduced_inductor_resistance[row][column]
                    } else {
                        -reduced_inductor_resistance[row][column] / l_values[row]
                    };
            }
        }

        for (motor_index, (branch, _, motor_constant, rotor_inertia, viscous_friction)) in
            motor_branches.iter().enumerate()
        {
            let motor_speed_state = motor_state_start + motor_index;
            let branch_state = dynamic.len() + *branch;
            state_matrix[branch_state][motor_speed_state] = -*motor_constant / l_values[*branch];
            state_matrix[motor_speed_state][branch_state] = *motor_constant / *rotor_inertia;
            state_matrix[motor_speed_state][motor_speed_state] =
                -*viscous_friction / *rotor_inertia;
        }

        let scales = dynamic
            .iter()
            .map(|index| capacitance[*index][*index].sqrt())
            .chain(l_values.iter().map(|inductance| inductance.sqrt()))
            .chain(
                motor_branches
                    .iter()
                    .map(|(_, _, _, inertia, _)| inertia.sqrt()),
            )
            .collect::<Vec<_>>();
        if scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale <= 0.0)
        {
            return None;
        }
        for row in 0..state_matrix.len() {
            for column in 0..state_matrix.len() {
                state_matrix[row][column] *= scales[row] / scales[column];
                if !state_matrix[row][column].is_finite() {
                    return None;
                }
            }
        }
        let norm = state_matrix
            .iter()
            .map(|row| row.iter().map(|value| value.abs()).sum::<f64>())
            .fold(0.0_f64, f64::max);
        if !norm.is_finite() {
            return None;
        }
        maximum_rate = maximum_rate.max(norm * (1.0 + 1e-12));
    }
    Some(maximum_rate)
}

fn add_incidence(
    matrix: &mut [Vec<f64>],
    pin1: Option<usize>,
    pin2: Option<usize>,
    branch: usize,
) -> Option<()> {
    if pin1 == pin2 {
        return Some(());
    }
    if let Some(pin1) = pin1 {
        matrix
            .get_mut(pin1)?
            .get_mut(branch)
            .map(|value| *value += 1.0)?;
    }
    if let Some(pin2) = pin2 {
        matrix
            .get_mut(pin2)?
            .get_mut(branch)
            .map(|value| *value -= 1.0)?;
    }
    Some(())
}

fn submatrix_columns(matrix: &[Vec<f64>], rows: &[usize]) -> Vec<Vec<f64>> {
    rows.iter().map(|row| matrix[*row].clone()).collect()
}

fn transpose(matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let columns = matrix.first().map_or(0, Vec::len);
    (0..columns)
        .map(|column| matrix.iter().map(|row| row[column]).collect())
        .collect()
}

fn add_laplacian(
    matrix: &mut [Vec<f64>],
    pin1: Option<usize>,
    pin2: Option<usize>,
    value: f64,
) -> Option<()> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    if pin1 == pin2 {
        return Some(());
    }
    if let Some(pin1) = pin1 {
        matrix[pin1][pin1] += value;
        if !matrix[pin1][pin1].is_finite() {
            return None;
        }
    }
    if let Some(pin2) = pin2 {
        matrix[pin2][pin2] += value;
        if !matrix[pin2][pin2].is_finite() {
            return None;
        }
    }
    if let (Some(pin1), Some(pin2)) = (pin1, pin2) {
        matrix[pin1][pin2] -= value;
        matrix[pin2][pin1] -= value;
        if !matrix[pin1][pin2].is_finite() || !matrix[pin2][pin1].is_finite() {
            return None;
        }
    }
    Some(())
}

fn submatrix(matrix: &[Vec<f64>], rows: &[usize], columns: &[usize]) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| columns.iter().map(|column| matrix[*row][*column]).collect())
        .collect()
}

fn multiply_matrices(left: &[Vec<f64>], right: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    if left.is_empty() {
        return Some(Vec::new());
    }
    let inner = left[0].len();
    if right.len() != inner || right.iter().any(|row| row.len() != right[0].len()) {
        return None;
    }
    let columns = right.first().map_or(0, Vec::len);
    let mut result = vec![vec![0.0; columns]; left.len()];
    for row in 0..left.len() {
        for column in 0..columns {
            for index in 0..inner {
                result[row][column] += left[row][index] * right[index][column];
            }
            if !result[row][column].is_finite() {
                return None;
            }
        }
    }
    Some(result)
}

fn invert_matrix(matrix: Vec<Vec<f64>>) -> Option<Vec<Vec<f64>>> {
    let dimension = matrix.len();
    if dimension == 0 || matrix.iter().any(|row| row.len() != dimension) {
        return None;
    }
    let mut augmented = matrix
        .into_iter()
        .enumerate()
        .map(|(index, mut row)| {
            row.extend((0..dimension).map(|column| if index == column { 1.0 } else { 0.0 }));
            row
        })
        .collect::<Vec<_>>();
    let scale = augmented
        .iter()
        .flat_map(|row| row[..dimension].iter())
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if !scale.is_finite() || scale == 0.0 {
        return None;
    }
    for pivot in 0..dimension {
        let (pivot_row, pivot_value) = (pivot..dimension)
            .map(|row| (row, augmented[row][pivot].abs()))
            .max_by(|left, right| left.1.total_cmp(&right.1))?;
        if pivot_value <= scale * f64::EPSILON * dimension as f64 * 16.0 {
            return None;
        }
        augmented.swap(pivot, pivot_row);
        let divisor = augmented[pivot][pivot];
        for value in &mut augmented[pivot] {
            *value /= divisor;
        }
        let pivot_values = augmented[pivot].clone();
        for (row_index, row) in augmented.iter_mut().enumerate() {
            if row_index == pivot {
                continue;
            }
            let factor = row[pivot];
            for (value, pivot_value) in row.iter_mut().zip(&pivot_values) {
                *value -= factor * pivot_value;
            }
        }
    }
    let inverse = augmented
        .into_iter()
        .map(|row| row[dimension..].to_vec())
        .collect::<Vec<_>>();
    inverse
        .iter()
        .flatten()
        .all(|value| value.is_finite())
        .then_some(inverse)
}

struct DisjointSets {
    parents: Vec<usize>,
}

impl DisjointSets {
    fn new(count: usize) -> Self {
        Self {
            parents: (0..count).collect(),
        }
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parents[value] != value {
            self.parents[value] = self.find(self.parents[value]);
        }
        self.parents[value]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root != right_root {
            let (parent, child) = if left_root < right_root {
                (left_root, right_root)
            } else {
                (right_root, left_root)
            };
            self.parents[child] = parent;
        }
    }
}

fn sort_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    for diagnostic in diagnostics.iter_mut() {
        diagnostic
            .references
            .sort_by_key(|reference| format!("{reference:?}"));
        diagnostic.references.dedup();
    }
    diagnostics.sort_by_key(|diagnostic| format!("{diagnostic:?}"));
    diagnostics.dedup();
}
