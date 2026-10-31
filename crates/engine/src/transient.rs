//! Transient-state handling and deterministic internal-step selection.

use std::collections::{HashMap, HashSet};

use crate::{
    ComponentSpec, DeviceState, Diagnostic, DiagnosticCode, FuseState, GeometryEnvelope,
    RelayContactState, SIMULATION_TIMESTEP_SECONDS, Schematic, StableId,
    devices::{DeviceResult, IntegrationMethod},
    netlist::derive_netlist_with_open,
};

use super::DeviceModel;

type PublicStepOutcome = Result<
    (
        Vec<DeviceResult>,
        CircuitStates,
        CircuitStates,
        Vec<Diagnostic>,
    ),
    Vec<Diagnostic>,
>;

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
    pub(super) current_amps: f64,
    pub(super) voltage_volts: f64,
    pub(super) state: RelayContactState,
    pub(super) last_transition_time_seconds: Option<f64>,
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
    pub(super) capacitors: HashMap<StableId, CapacitorState>,
    pub(super) inductors: HashMap<StableId, InductorState>,
    pub(super) relays: HashMap<StableId, RelayState>,
    pub(super) fuses: HashMap<StableId, FuseRuntimeState>,
    pub(super) motors: HashMap<StableId, MotorState>,
    pub(super) simulation_time_seconds: f64,
    pub(super) backward_euler_next_step: bool,
}

impl CircuitStates {
    pub(super) fn device_states(&self) -> Vec<DeviceState> {
        let mut states = self
            .relays
            .iter()
            .map(|(component_id, relay)| DeviceState::Relay {
                component_id: component_id.clone(),
                state: relay.state,
                last_transition_time_seconds: relay.last_transition_time_seconds,
            })
            .collect::<Vec<_>>();
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
    },
}

pub(super) fn circuit_states_from_results(
    schematic: &Schematic,
    results: &[DeviceResult],
    simulation_time_seconds: f64,
) -> CircuitStates {
    let by_component = results
        .iter()
        .map(|result| (result.component_id.as_str(), result))
        .collect::<HashMap<_, _>>();
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
            let ComponentSpec::RelaySpdt { id, .. } = component else {
                return None;
            };
            let result = by_component.get(id.as_str())?;
            let Some(DeviceState::Relay {
                state,
                last_transition_time_seconds,
                ..
            }) = &result.state
            else {
                return None;
            };
            Some((
                id.clone(),
                RelayState {
                    current_amps: result.current_amps,
                    voltage_volts: result.voltage_volts,
                    state: *state,
                    last_transition_time_seconds: *last_transition_time_seconds,
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
    CircuitStates {
        capacitors,
        inductors,
        relays,
        fuses,
        motors,
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
            let (mut step_results, step_diagnostics) = super::compute_circuit(
                schematic,
                geometry,
                open,
                SolveMode::Transient {
                    step_seconds: internal_step_seconds,
                    states: &states,
                    integration_method,
                },
            )?;
            diagnostics.extend(step_diagnostics);
            let mut candidate =
                circuit_states_from_results(schematic, &step_results, step_end_time);
            // The requested recovery method was consumed by this interval. A new
            // relay transition below can schedule another backward-Euler interval.
            candidate.backward_euler_next_step = false;
            let fuse_transitions = accumulate_fuse_i2t(
                schematic,
                &interval_start_states,
                &mut candidate,
                internal_step_seconds,
                step_end_time,
            )?;
            settle_discrete_events(
                CircuitInput {
                    schematic,
                    geometry,
                    open,
                },
                &mut candidate,
                &mut step_results,
                fuse_transitions,
                step_end_time,
                &mut diagnostics,
            )?;
            states = candidate;
            results = step_results;
        }
    }
    sort_diagnostics(&mut diagnostics);
    Ok((results, states, step_start_states, diagnostics))
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
    mut fuse_transitions: Vec<StableId>,
    event_time_seconds: f64,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let CircuitInput {
        schematic,
        geometry,
        open,
    } = circuit;
    const MAX_SAME_TIME_TRANSITIONS: usize = 16;
    let mut seen = HashSet::new();
    seen.insert(discrete_state_vector(states, &fuse_transitions));
    let mut same_time_resolves = 0;
    loop {
        let mut relay_changes = Vec::new();
        for component in &schematic.components {
            let ComponentSpec::RelaySpdt { id, parameters, .. } = component else {
                continue;
            };
            let Some(relay) = states.relays.get(id) else {
                continue;
            };
            let next_state = match relay.state {
                RelayContactState::Released
                    if relay.current_amps.abs() >= parameters.pickup_current_amps =>
                {
                    Some(RelayContactState::Picked)
                }
                RelayContactState::Picked
                    if relay.current_amps.abs() <= parameters.dropout_current_amps =>
                {
                    Some(RelayContactState::Released)
                }
                _ => None,
            };
            if let Some(next_state) = next_state {
                relay_changes.push((id.clone(), next_state));
            }
        }
        if relay_changes.is_empty() && fuse_transitions.is_empty() {
            return Ok(());
        }
        relay_changes.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
        let mut changed_ids = relay_changes
            .iter()
            .map(|(component_id, _)| component_id.clone())
            .collect::<Vec<_>>();
        changed_ids.append(&mut fuse_transitions);
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
        fuse_transitions = Vec::new();
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
        let (resolved, event_diagnostics) =
            super::compute_circuit(schematic, geometry, open, SolveMode::InitialState(states))?;
        *results = resolved;
        diagnostics.extend(event_diagnostics);
        let mut resolved_states =
            circuit_states_from_results(schematic, results, event_time_seconds);
        // The event state is the state vector used to solve this reconfiguration; the
        // device results carry that same vector and its exact transition times.
        resolved_states.simulation_time_seconds = event_time_seconds;
        resolved_states.backward_euler_next_step = states.backward_euler_next_step;
        *states = resolved_states;
        same_time_resolves += 1;
    }
}

fn discrete_state_vector(
    states: &CircuitStates,
    pending_fuse_transitions: &[StableId],
) -> Vec<(String, u8)> {
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
    vector.extend(states.fuses.iter().map(|(id, fuse)| {
        let pending = pending_fuse_transitions.iter().any(|pending| pending == id);
        (
            id.as_str().to_owned(),
            u8::from(!pending && fuse.state == FuseState::Tripped),
        )
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
    let omega_hat = estimate_transient_rate_bound(schematic, geometry, open).unwrap_or(f64::MAX);
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
    let specs = super::build_device_specs(schematic, &netlist)
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
        }

        let reference_root = sets.find(reference_node);
        let mut root_to_variable = HashMap::new();
        for node in island.node_indices.iter().copied() {
            let root = sets.find(node);
            if root != reference_root {
                let next = root_to_variable.len();
                root_to_variable.entry(root).or_insert(next);
            }
        }
        let dimension = root_to_variable.len();
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
        let mut incidence = vec![vec![0.0; inductor_specs.len()]; dimension];

        for spec in &island_specs {
            let (pin1, pin2) = (
                sets.find(spec.terminals.pin1),
                sets.find(spec.terminals.pin2),
            );
            let variable1 = root_to_variable.get(&pin1).copied();
            let variable2 = root_to_variable.get(&pin2).copied();
            match spec.model {
                DeviceModel::Resistor { resistance_ohms } => {
                    add_laplacian(
                        &mut conductance,
                        variable1,
                        variable2,
                        1.0 / resistance_ohms,
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
                DeviceModel::DcVoltageSource { .. } => {}
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

        let dynamic = (0..dimension)
            .filter(|index| capacitance[*index][*index] > 0.0)
            .collect::<Vec<_>>();
        let algebraic = (0..dimension)
            .filter(|index| capacitance[*index][*index] == 0.0)
            .collect::<Vec<_>>();
        let l_values = inductor_specs
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
            let branch_voltage_b = transpose(&reduced_b);
            for row in 0..l_values.len() {
                for column in 0..dynamic.len() {
                    state_matrix[dynamic.len() + row][column] =
                        branch_voltage_b[row][column] / l_values[row];
                }
            }
        }
        for row in 0..l_values.len() {
            for column in 0..l_values.len() {
                state_matrix[dynamic.len() + row][dynamic.len() + column] =
                    -reduced_inductor_resistance[row][column] / l_values[row];
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
