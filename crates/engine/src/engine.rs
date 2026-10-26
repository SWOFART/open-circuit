//! Stateful Engine API for compiling, updating, and advancing circuit simulations.

#[path = "trace.rs"]
mod trace;
#[path = "transient.rs"]
mod transient;

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentSpec, Diagnostic, DiagnosticCode,
    DiagnosticReference, DiagnosticSeverity, GeometryEnvelope, ParameterId, PinId,
    RelayContactState, ResultLayout, ResultQuantity, ResultSignConvention, ResultSlot,
    ResultSnapshot, SIMULATION_TIMESTEP_SECONDS, Schematic, SetParameterRequest,
    SetParameterResult, StableId, TraceBatch, TraceRequest, Unit,
    devices::{
        CapacitorDevice, CapacitorMode, DcVoltageSourceDevice, Device, DeviceResult,
        InductorDevice, InductorMode, RelayCoilMode, RelayDevice, ResistorDevice, Terminals,
        component_diagnostic,
    },
    mna::{MnaSystem, NodeVariables},
    netlist::{ElectricalIsland, Netlist, PinRef, derive_netlist},
    solver::SolverError,
};
use trace::TraceHistory;
use transient::{
    CapacitorState, CircuitStates, InductorState, SolveMode, advance_public_steps,
    choose_internal_substeps, circuit_states_from_results, settle_relay_events,
};

/// Owns the active compiled run and its latest valid result.
#[derive(Default)]
pub struct Engine {
    compiled: Option<CompiledRun>,
}

struct CompiledRun {
    run_id: StableId,
    schematic: Schematic,
    geometry: GeometryEnvelope,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
    states: CircuitStates,
    step_start_states: CircuitStates,
    internal_substeps: u32,
    trace_history: TraceHistory,
}

type InitializationOutcome =
    Result<(Vec<DeviceResult>, CircuitStates, Vec<Diagnostic>), Vec<Diagnostic>>;

#[derive(Clone)]
struct DeviceSpec {
    component_id: StableId,
    terminals: Terminals,
    model: DeviceModel,
}

#[derive(Clone, Copy)]
enum DeviceModel {
    Resistor {
        resistance_ohms: f64,
    },
    DcVoltageSource {
        voltage_volts: f64,
        internal_resistance_ohms: f64,
    },
    Capacitor {
        capacitance_farads: f64,
        initial_voltage_volts: Option<f64>,
    },
    Inductor {
        inductance_henries: f64,
        initial_current_amps: Option<f64>,
    },
    RelaySpdt {
        coil_resistance_ohms: f64,
        coil_inductance_henries: f64,
        closed_contact_resistance_ohms: f64,
        common_node: usize,
        normally_open_node: usize,
        normally_closed_node: usize,
    },
}

impl Engine {
    /// Validate, compile, and solve the operating point at simulation step zero.
    pub fn compile(&mut self, request: CompileRequest) -> CompileResult {
        let (device_results, states, mut diagnostics) =
            match initialize_circuit(&request.schematic, &request.geometry) {
                Ok(computed) => computed,
                Err(diagnostics) => return CompileResult::Failure { diagnostics },
            };
        let (internal_substeps, limited) =
            choose_internal_substeps(&request.schematic, &request.geometry);
        if limited {
            diagnostics.push(Diagnostic::warning(
                DiagnosticCode::StepAccuracyLimited,
                Vec::new(),
            ));
            sort_diagnostics(&mut diagnostics);
        }
        let layout = match build_result_layout(&request.schematic) {
            Ok(layout) => layout,
            Err(diagnostics) => return CompileResult::Failure { diagnostics },
        };
        let values = match values_for_layout(&layout, &device_results) {
            Ok(values) => values,
            Err(diagnostics) => return CompileResult::Failure { diagnostics },
        };
        let run_id = new_stable_uuid();
        let snapshot = ResultSnapshot {
            metadata: crate::FrameMetadata {
                run_id: run_id.clone(),
                document_revision: request.document_revision.clone(),
                layout_version: layout.layout_version,
                simulation_step: 0,
                simulation_time_seconds: 0.0,
                frame_sequence: 0,
            },
            values,
            states: states.device_states(),
        };

        let Some(mut trace_history) = TraceHistory::new(layout.value_count as usize) else {
            return CompileResult::Failure {
                diagnostics: vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    Vec::new(),
                )],
            };
        };
        if trace_history
            .append(0, &snapshot.metadata.document_revision, &snapshot.values)
            .is_err()
        {
            return CompileResult::Failure {
                diagnostics: vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    Vec::new(),
                )],
            };
        }

        self.compiled = Some(CompiledRun {
            run_id: run_id.clone(),
            schematic: request.schematic,
            geometry: request.geometry,
            layout: layout.clone(),
            snapshot: snapshot.clone(),
            step_start_states: states.clone(),
            states,
            internal_substeps,
            trace_history,
        });

        CompileResult::Success {
            run_id,
            layout,
            internal_substeps,
            initial_snapshot: snapshot,
            diagnostics,
        }
    }

    /// Atomically update one parameter and recalculate the same compiled run.
    pub fn set_parameter(&mut self, request: SetParameterRequest) -> SetParameterResult {
        let Some(active) = self.compiled.as_ref() else {
            return SetParameterResult::Failure {
                diagnostics: vec![Diagnostic::error(DiagnosticCode::StaleRun, Vec::new())],
            };
        };
        if request.run_id != active.run_id {
            return SetParameterResult::Failure {
                diagnostics: vec![Diagnostic::error(DiagnosticCode::StaleRun, Vec::new())],
            };
        }
        if request.expected_document_revision != active.snapshot.metadata.document_revision
            || request.document_revision == active.snapshot.metadata.document_revision
        {
            return SetParameterResult::Failure {
                diagnostics: vec![component_diagnostic(
                    DiagnosticCode::StaleDocumentRevision,
                    &request.component_id,
                )],
            };
        }
        let Some(frame_sequence) = active.snapshot.metadata.frame_sequence.checked_add(1) else {
            return SetParameterResult::Failure {
                diagnostics: vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    vec![DiagnosticReference::Component {
                        component_id: request.component_id,
                    }],
                )],
            };
        };

        let mut candidate = active.schematic.clone();
        if !update_parameter(
            &mut candidate,
            &request.component_id,
            request.parameter_id,
            request.value,
        ) {
            return SetParameterResult::Failure {
                diagnostics: vec![component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &request.component_id,
                )],
            };
        }

        let geometry = active.geometry.clone();
        let run_id = active.run_id.clone();
        let layout = active.layout.clone();
        let simulation_step = active.snapshot.metadata.simulation_step;
        let document_revision = request.document_revision;
        let previous_step_states = active.step_start_states.clone();
        let (internal_substeps, limited) = choose_internal_substeps(&candidate, &geometry);
        let (device_results, states, mut diagnostics) = if simulation_step == 0 {
            match initialize_circuit(&candidate, &geometry) {
                Ok(computed) => computed,
                Err(diagnostics) => return SetParameterResult::Failure { diagnostics },
            }
        } else {
            match advance_public_steps(
                &candidate,
                &geometry,
                previous_step_states,
                internal_substeps,
                simulation_step,
                1,
            ) {
                Ok((results, states, _step_start_states, diagnostics)) => {
                    (results, states, diagnostics)
                }
                Err(diagnostics) => return SetParameterResult::Failure { diagnostics },
            }
        };
        if limited {
            diagnostics.push(Diagnostic::warning(
                DiagnosticCode::StepAccuracyLimited,
                Vec::new(),
            ));
            sort_diagnostics(&mut diagnostics);
        }
        let values = match values_for_layout(&layout, &device_results) {
            Ok(values) => values,
            Err(diagnostics) => return SetParameterResult::Failure { diagnostics },
        };
        let simulation_time_seconds = simulation_step as f64 * SIMULATION_TIMESTEP_SECONDS;
        if !simulation_time_seconds.is_finite() {
            return SetParameterResult::Failure {
                diagnostics: vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    vec![DiagnosticReference::Component {
                        component_id: request.component_id,
                    }],
                )],
            };
        }

        let snapshot = ResultSnapshot {
            metadata: crate::FrameMetadata {
                run_id: run_id.clone(),
                document_revision: document_revision.clone(),
                layout_version: layout.layout_version,
                simulation_step,
                simulation_time_seconds,
                frame_sequence,
            },
            values,
            states: states.device_states(),
        };
        let active = self
            .compiled
            .as_mut()
            .expect("active run was checked above");
        if active
            .trace_history
            .replace_latest(
                simulation_step,
                &snapshot.metadata.document_revision,
                &snapshot.values,
            )
            .is_err()
        {
            return SetParameterResult::Failure {
                diagnostics: vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    vec![DiagnosticReference::Component {
                        component_id: request.component_id,
                    }],
                )],
            };
        }
        active.schematic = candidate;
        active.snapshot = snapshot.clone();
        active.states = states.clone();
        if simulation_step == 0 {
            active.step_start_states = states;
        }
        active.internal_substeps = internal_substeps;
        SetParameterResult::Success {
            snapshot,
            internal_substeps,
            diagnostics,
        }
    }

    /// Advance the compiled circuit by the requested number of public transient steps.
    pub fn advance(&mut self, request: AdvanceRequest) -> Result<ResultSnapshot, Vec<Diagnostic>> {
        let Some(active) = self.compiled.as_mut() else {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::StaleRun,
                Vec::new(),
            )]);
        };
        if request.run_id != active.run_id {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::StaleRun,
                Vec::new(),
            )]);
        }
        if request.step_count == 0 {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        }
        let Some(simulation_step) = active
            .snapshot
            .metadata
            .simulation_step
            .checked_add(u64::from(request.step_count))
        else {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        };
        let Some(frame_sequence) = active.snapshot.metadata.frame_sequence.checked_add(1) else {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        };
        if !(simulation_step as f64 * SIMULATION_TIMESTEP_SECONDS).is_finite() {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        }

        let mut states = active.states.clone();
        let mut step_start_states = active.step_start_states.clone();
        let mut samples = TraceHistory::with_capacity(
            active.layout.value_count as usize,
            (request.step_count as usize).min(active.trace_history.capacity()),
        )
        .ok_or_else(|| {
            vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]
        })?;
        let document_revision = active.snapshot.metadata.document_revision.clone();
        let mut final_values = None;

        for offset in 1..=request.step_count {
            let current_step = active.snapshot.metadata.simulation_step + u64::from(offset);
            let (device_results, next_states, next_step_start_states, _diagnostics) =
                advance_public_steps(
                    &active.schematic,
                    &active.geometry,
                    states,
                    active.internal_substeps,
                    current_step,
                    1,
                )?;
            let values = values_for_layout(&active.layout, &device_results)?;
            let simulation_time_seconds = current_step as f64 * SIMULATION_TIMESTEP_SECONDS;
            if !simulation_time_seconds.is_finite()
                || samples
                    .append(current_step, &document_revision, &values)
                    .is_err()
            {
                return Err(vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    Vec::new(),
                )]);
            }

            states = next_states;
            step_start_states = next_step_start_states;
            final_values = Some(values);
        }

        active.trace_history.append_history(&samples).map_err(|_| {
            vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]
        })?;
        active.step_start_states = step_start_states;
        active.states = states;
        active.snapshot.values = final_values.expect("advance requires at least one step");
        active.snapshot.states = active.states.device_states();
        active.snapshot.metadata.simulation_step = simulation_step;
        active.snapshot.metadata.simulation_time_seconds =
            simulation_step as f64 * SIMULATION_TIMESTEP_SECONDS;
        active.snapshot.metadata.frame_sequence = frame_sequence;
        Ok(active.snapshot.clone())
    }

    pub fn latest_snapshot(&self) -> Option<&ResultSnapshot> {
        self.compiled.as_ref().map(|active| &active.snapshot)
    }

    pub fn result_layout(&self) -> Option<&ResultLayout> {
        self.compiled.as_ref().map(|active| &active.layout)
    }

    /// Reads a bounded selection of samples from the active run's trace history.
    pub fn read_traces(&self, request: TraceRequest) -> Result<TraceBatch, Vec<Diagnostic>> {
        let Some(active) = self.compiled.as_ref() else {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::StaleRun,
                Vec::new(),
            )]);
        };
        if request.run_id != active.run_id || request.layout_version != active.layout.layout_version
        {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::StaleRun,
                Vec::new(),
            )]);
        }

        active
            .trace_history
            .read(&request, active.run_id.clone(), &active.layout)
    }
}

fn initialize_circuit(schematic: &Schematic, geometry: &GeometryEnvelope) -> InitializationOutcome {
    let capacitors = schematic
        .components
        .iter()
        .filter_map(|component| match component {
            ComponentSpec::Capacitor { id, parameters, .. } => {
                Some((id.clone(), parameters.initial_voltage_volts))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let inductors = schematic
        .components
        .iter()
        .filter_map(|component| match component {
            ComponentSpec::Inductor { id, parameters, .. } => {
                Some((id.clone(), parameters.initial_current_amps))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let has_explicit_initial_state = capacitors.iter().any(|(_, initial)| initial.is_some())
        || inductors.iter().any(|(_, initial)| initial.is_some());
    let has_uninitialized_state = capacitors.iter().any(|(_, initial)| initial.is_none())
        || inductors.iter().any(|(_, initial)| initial.is_none());
    let needs_operating_point = has_explicit_initial_state && has_uninitialized_state;
    let mut diagnostics = Vec::new();
    let mut states = CircuitStates::default();

    if needs_operating_point {
        let (operating_point, operating_diagnostics) =
            compute_circuit(schematic, geometry, SolveMode::OperatingPoint)?;
        diagnostics.extend(operating_diagnostics);
        states = circuit_states_from_results(schematic, &operating_point, 0.0);
    }

    if has_explicit_initial_state {
        for (component_id, initial_voltage_volts) in &capacitors {
            if let Some(voltage_volts) = initial_voltage_volts {
                states.capacitors.insert(
                    component_id.clone(),
                    CapacitorState {
                        voltage_volts: *voltage_volts,
                        current_amps: 0.0,
                    },
                );
            }
        }
        for (component_id, initial_current_amps) in &inductors {
            if let Some(current_amps) = initial_current_amps {
                states.inductors.insert(
                    component_id.clone(),
                    InductorState {
                        current_amps: *current_amps,
                        voltage_volts: 0.0,
                    },
                );
            }
        }
    }

    let (mut results, initial_diagnostics) = if has_explicit_initial_state {
        compute_circuit(schematic, geometry, SolveMode::InitialState(&states))?
    } else {
        compute_circuit(schematic, geometry, SolveMode::OperatingPoint)?
    };
    diagnostics.extend(initial_diagnostics);
    states = circuit_states_from_results(schematic, &results, 0.0);
    settle_relay_events(
        schematic,
        geometry,
        &mut states,
        &mut results,
        0.0,
        &mut diagnostics,
    )?;
    sort_diagnostics(&mut diagnostics);
    Ok((results, states, diagnostics))
}

fn compute_circuit(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    mode: SolveMode<'_>,
) -> Result<(Vec<DeviceResult>, Vec<Diagnostic>), Vec<Diagnostic>> {
    let netlist = derive_netlist(schematic, geometry)?;
    let mut diagnostics = netlist.diagnostics.clone();
    let device_specs = match build_device_specs(schematic, &netlist) {
        Ok(specs) => specs,
        Err(mut errors) => {
            diagnostics.append(&mut errors);
            sort_diagnostics(&mut diagnostics);
            return Err(diagnostics);
        }
    };

    let constraint_errors =
        check_ideal_voltage_constraints(&device_specs, netlist.nodes.len(), mode);
    if !constraint_errors.is_empty() {
        diagnostics.extend(constraint_errors);
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
    }
    if let SolveMode::InitialState(states) = mode {
        let state_errors = check_initial_state_constraints(
            &device_specs,
            states,
            netlist.nodes.len(),
            &netlist.islands,
        );
        if !state_errors.is_empty() {
            diagnostics.extend(state_errors);
            sort_diagnostics(&mut diagnostics);
            return Err(diagnostics);
        }
    }

    let mut results = Vec::with_capacity(device_specs.len());
    for island in &netlist.islands {
        let island_component_ids = island
            .component_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<HashSet<_>>();
        let island_specs = device_specs
            .iter()
            .filter(|spec| island_component_ids.contains(spec.component_id.as_str()))
            .collect::<Vec<_>>();
        if island_specs.is_empty() {
            continue;
        }
        let Some(reference_node) = island.reference_node else {
            diagnostics.push(diagnostic_for_island(DiagnosticCode::FloatingNode, island));
            continue;
        };

        let additional_reference_nodes = local_reference_nodes(
            &island_specs,
            island,
            mode,
            netlist.nodes.len(),
            reference_node,
        );
        let variables = NodeVariables::for_island(
            netlist.nodes.len(),
            &island.node_indices,
            reference_node,
            &additional_reference_nodes,
        );
        let mut branch_variables = HashMap::new();
        let mut next_variable = variables.voltage_variable_count();
        for spec in &island_specs {
            let ideal_source = matches!(
                spec.model,
                DeviceModel::DcVoltageSource {
                    internal_resistance_ohms: 0.0,
                    ..
                }
            );
            let initial_capacitor = matches!(spec.model, DeviceModel::Capacitor { .. })
                && matches!(mode, SolveMode::InitialState(_));
            let operating_point_inductor = matches!(spec.model, DeviceModel::Inductor { .. })
                && matches!(mode, SolveMode::OperatingPoint);
            if ideal_source || initial_capacitor || operating_point_inductor {
                branch_variables.insert(spec.component_id.as_str().to_owned(), next_variable);
                next_variable += 1;
            }
        }

        let devices = island_specs
            .iter()
            .map(|spec| {
                make_device(
                    spec,
                    branch_variables.get(spec.component_id.as_str()).copied(),
                    mode,
                )
            })
            .collect::<Vec<_>>();
        let mut system = MnaSystem::new(next_variable);
        for device in &devices {
            if let Err(diagnostic) = device.stamp(&mut system, &variables) {
                diagnostics.push(diagnostic);
            }
        }
        if has_errors(&diagnostics) {
            continue;
        }

        let solved = match system.solve() {
            Ok(solved) => solved,
            Err(
                SolverError::DimensionMismatch
                | SolverError::NonFinite
                | SolverError::Singular
                | SolverError::ResidualTooLarge,
            ) => {
                diagnostics.push(diagnostic_for_island(
                    DiagnosticCode::SingularSystem,
                    island,
                ));
                continue;
            }
        };

        let mut node_voltages = vec![0.0; netlist.nodes.len()];
        for node in island.node_indices.iter().copied() {
            if let Some(variable) = variables.voltage_variable(node) {
                node_voltages[node] = solved.values[variable];
            }
        }
        for device in devices {
            match device.result(&node_voltages, &solved.values) {
                Ok(result) => results.push(result),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }

    sort_diagnostics(&mut diagnostics);
    if has_errors(&diagnostics) {
        return Err(diagnostics);
    }
    results.sort_by(|left, right| left.component_id.as_str().cmp(right.component_id.as_str()));
    Ok((results, diagnostics))
}

/// A netlist island can contain electrically independent ports of a multi-port
/// device (the relay coil and contacts are galvanically isolated). Give each
/// disconnected matrix block its own gauge reference so one port cannot make
/// another port singular or accidentally tie their potentials together.
fn local_reference_nodes(
    specs: &[&DeviceSpec],
    island: &ElectricalIsland,
    mode: SolveMode<'_>,
    node_count: usize,
    primary_reference: usize,
) -> HashSet<usize> {
    let mut adjacency = vec![Vec::new(); node_count];
    let mut connect = |left: usize, right: usize| {
        adjacency[left].push(right);
        adjacency[right].push(left);
    };
    for spec in specs {
        match spec.model {
            DeviceModel::Resistor { .. } | DeviceModel::DcVoltageSource { .. } => {
                connect(spec.terminals.pin1, spec.terminals.pin2);
            }
            DeviceModel::Capacitor { .. } if !matches!(mode, SolveMode::OperatingPoint) => {
                connect(spec.terminals.pin1, spec.terminals.pin2);
            }
            DeviceModel::Inductor { .. }
                if matches!(
                    mode,
                    SolveMode::OperatingPoint | SolveMode::Transient { .. }
                ) =>
            {
                connect(spec.terminals.pin1, spec.terminals.pin2);
            }
            DeviceModel::RelaySpdt {
                coil_resistance_ohms: _,
                common_node,
                normally_open_node,
                normally_closed_node,
                ..
            } => {
                let coil_has_conductance = match mode {
                    SolveMode::OperatingPoint => true,
                    SolveMode::InitialState(states) => {
                        !states.relays.contains_key(&spec.component_id)
                    }
                    SolveMode::Transient { .. } => true,
                };
                if coil_has_conductance {
                    connect(spec.terminals.pin1, spec.terminals.pin2);
                }
                let contact_state = match mode {
                    SolveMode::OperatingPoint => RelayContactState::Released,
                    SolveMode::InitialState(states) | SolveMode::Transient { states, .. } => states
                        .relays
                        .get(&spec.component_id)
                        .map_or(RelayContactState::Released, |state| state.state),
                };
                let active = match contact_state {
                    RelayContactState::Released => normally_closed_node,
                    RelayContactState::Picked => normally_open_node,
                };
                connect(common_node, active);
            }
            DeviceModel::Capacitor { .. } | DeviceModel::Inductor { .. } => {}
        }
    }

    let mut extra_references = HashSet::new();
    let mut visited = vec![false; node_count];
    for start in island.node_indices.iter().copied() {
        if visited[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut component = Vec::new();
        visited[start] = true;
        while let Some(node) = stack.pop() {
            component.push(node);
            for adjacent in adjacency[node].iter().copied() {
                if !visited[adjacent] {
                    visited[adjacent] = true;
                    stack.push(adjacent);
                }
            }
        }
        if !component.contains(&primary_reference) {
            extra_references.insert(*component.iter().min().expect("component is nonempty"));
        }
    }
    extra_references
}

fn build_device_specs(
    schematic: &Schematic,
    netlist: &Netlist,
) -> Result<Vec<DeviceSpec>, Vec<Diagnostic>> {
    let mut pin_to_node = HashMap::<PinRef, usize>::new();
    for node in &netlist.nodes {
        for pin in &node.pins {
            pin_to_node.insert(pin.clone(), node.index);
        }
    }

    let mut components = schematic.components.iter().collect::<Vec<_>>();
    components.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
    let mut specs = Vec::with_capacity(components.len());
    let mut diagnostics = Vec::new();
    for component in components {
        let node_for_pin = |pin_id| {
            pin_to_node
                .get(&PinRef {
                    component_id: component.id().clone(),
                    pin_id,
                })
                .copied()
        };
        let (primary_pin1, primary_pin2) = if matches!(component, ComponentSpec::RelaySpdt { .. }) {
            (PinId::CoilPositive, PinId::CoilNegative)
        } else {
            (PinId::Pin1, PinId::Pin2)
        };
        let (Some(pin1), Some(pin2)) = (node_for_pin(primary_pin1), node_for_pin(primary_pin2))
        else {
            diagnostics.push(component_diagnostic(
                DiagnosticCode::InvalidGeometry,
                component.id(),
            ));
            continue;
        };
        let terminals = Terminals { pin1, pin2 };

        let model = match component {
            ComponentSpec::Resistor { parameters, .. } => DeviceModel::Resistor {
                resistance_ohms: parameters.resistance_ohms,
            },
            ComponentSpec::DcVoltageSource { parameters, .. } => {
                if parameters.internal_resistance_ohms == 0.0 && pin1 == pin2 {
                    diagnostics.push(Diagnostic::error(
                        DiagnosticCode::ShortCircuit,
                        vec![
                            DiagnosticReference::Component {
                                component_id: component.id().clone(),
                            },
                            DiagnosticReference::pin(component.id().clone(), PinId::Pin1),
                            DiagnosticReference::pin(component.id().clone(), PinId::Pin2),
                        ],
                    ));
                }
                DeviceModel::DcVoltageSource {
                    voltage_volts: parameters.voltage_volts,
                    internal_resistance_ohms: parameters.internal_resistance_ohms,
                }
            }
            ComponentSpec::Capacitor { parameters, .. } => DeviceModel::Capacitor {
                capacitance_farads: parameters.capacitance_farads,
                initial_voltage_volts: parameters.initial_voltage_volts,
            },
            ComponentSpec::Inductor { parameters, .. } => DeviceModel::Inductor {
                inductance_henries: parameters.inductance_henries,
                initial_current_amps: parameters.initial_current_amps,
            },
            ComponentSpec::RelaySpdt { parameters, .. } => {
                let (Some(common_node), Some(normally_open_node), Some(normally_closed_node)) = (
                    node_for_pin(PinId::Common),
                    node_for_pin(PinId::NormallyOpen),
                    node_for_pin(PinId::NormallyClosed),
                ) else {
                    diagnostics.push(component_diagnostic(
                        DiagnosticCode::InvalidGeometry,
                        component.id(),
                    ));
                    continue;
                };
                DeviceModel::RelaySpdt {
                    coil_resistance_ohms: parameters.coil_resistance_ohms,
                    coil_inductance_henries: parameters.coil_inductance_henries,
                    closed_contact_resistance_ohms: parameters.closed_contact_resistance_ohms,
                    common_node,
                    normally_open_node,
                    normally_closed_node,
                }
            }
        };
        specs.push(DeviceSpec {
            component_id: component.id().clone(),
            terminals,
            model,
        });
    }
    if has_errors(&diagnostics) {
        Err(diagnostics)
    } else {
        Ok(specs)
    }
}

fn make_device(
    spec: &DeviceSpec,
    branch_variable: Option<usize>,
    mode: SolveMode<'_>,
) -> Box<dyn Device> {
    match spec.model {
        DeviceModel::Resistor { resistance_ohms } => Box::new(ResistorDevice::new(
            spec.component_id.clone(),
            spec.terminals,
            resistance_ohms,
        )),
        DeviceModel::DcVoltageSource {
            voltage_volts,
            internal_resistance_ohms,
        } => Box::new(DcVoltageSourceDevice::new(
            spec.component_id.clone(),
            spec.terminals,
            voltage_volts,
            internal_resistance_ohms,
            branch_variable,
        )),
        DeviceModel::Capacitor {
            capacitance_farads, ..
        } => {
            let capacitor_mode = match mode {
                SolveMode::OperatingPoint => CapacitorMode::Open,
                SolveMode::InitialState(states) => {
                    let state = states
                        .capacitors
                        .get(&spec.component_id)
                        .expect("every capacitor has an initialized state");
                    CapacitorMode::Initial {
                        voltage_volts: state.voltage_volts,
                        branch_variable: branch_variable
                            .expect("initial-state capacitors have branch variables"),
                    }
                }
                SolveMode::Transient {
                    step_seconds,
                    states,
                    integration_method,
                } => {
                    let state = states
                        .capacitors
                        .get(&spec.component_id)
                        .expect("every transient capacitor has a state");
                    CapacitorMode::Transient {
                        step_seconds,
                        start_voltage_volts: state.voltage_volts,
                        start_current_amps: state.current_amps,
                        integration_method,
                    }
                }
            };
            Box::new(CapacitorDevice::new(
                spec.component_id.clone(),
                spec.terminals,
                capacitance_farads,
                capacitor_mode,
            ))
        }
        DeviceModel::Inductor {
            inductance_henries, ..
        } => {
            let inductor_mode = match mode {
                SolveMode::OperatingPoint => InductorMode::Short {
                    branch_variable: branch_variable
                        .expect("operating-point inductors have branch variables"),
                },
                SolveMode::InitialState(states) => InductorMode::Initial {
                    current_amps: states
                        .inductors
                        .get(&spec.component_id)
                        .expect("every inductor has an initialized state")
                        .current_amps,
                },
                SolveMode::Transient {
                    step_seconds,
                    states,
                    integration_method,
                } => {
                    let state = states
                        .inductors
                        .get(&spec.component_id)
                        .expect("every transient inductor has a state");
                    InductorMode::Transient {
                        step_seconds,
                        start_current_amps: state.current_amps,
                        start_voltage_volts: state.voltage_volts,
                        integration_method,
                    }
                }
            };
            Box::new(InductorDevice::new(
                spec.component_id.clone(),
                spec.terminals,
                inductance_henries,
                inductor_mode,
            ))
        }
        DeviceModel::RelaySpdt {
            coil_resistance_ohms,
            coil_inductance_henries,
            closed_contact_resistance_ohms,
            common_node,
            normally_open_node,
            normally_closed_node,
            ..
        } => {
            let (contact_state, last_transition_time_seconds) = match mode {
                SolveMode::OperatingPoint => (RelayContactState::Released, None),
                SolveMode::InitialState(states) | SolveMode::Transient { states, .. } => states
                    .relays
                    .get(&spec.component_id)
                    .map(|state| (state.state, state.last_transition_time_seconds))
                    .unwrap_or((RelayContactState::Released, None)),
            };
            let coil_mode = match mode {
                SolveMode::OperatingPoint => RelayCoilMode::OperatingPoint,
                SolveMode::InitialState(states) => states
                    .relays
                    .get(&spec.component_id)
                    .map(|state| RelayCoilMode::Initial {
                        current_amps: state.current_amps,
                    })
                    .unwrap_or(RelayCoilMode::OperatingPoint),
                SolveMode::Transient {
                    step_seconds,
                    states,
                    integration_method,
                } => {
                    let state = states.relays.get(&spec.component_id);
                    RelayCoilMode::Transient {
                        step_seconds,
                        start_current_amps: state.map_or(0.0, |state| state.current_amps),
                        start_voltage_volts: state.map_or(0.0, |state| state.voltage_volts),
                        integration_method,
                    }
                }
            };
            Box::new(RelayDevice::new(
                spec.component_id.clone(),
                spec.terminals,
                common_node,
                normally_open_node,
                normally_closed_node,
                coil_resistance_ohms,
                coil_inductance_henries,
                closed_contact_resistance_ohms,
                contact_state,
                last_transition_time_seconds,
                coil_mode,
            ))
        }
    }
}

fn check_ideal_voltage_constraints(
    specs: &[DeviceSpec],
    node_count: usize,
    mode: SolveMode<'_>,
) -> Vec<Diagnostic> {
    let mut adjacency = vec![Vec::<(usize, f64, StableId)>::new(); node_count];
    for spec in specs {
        let voltage = match spec.model {
            DeviceModel::DcVoltageSource {
                voltage_volts,
                internal_resistance_ohms: 0.0,
            } => Some(voltage_volts),
            DeviceModel::Inductor { .. } if matches!(mode, SolveMode::OperatingPoint) => Some(0.0),
            _ => None,
        };
        let Some(voltage_volts) = voltage else {
            continue;
        };
        adjacency[spec.terminals.pin1].push((
            spec.terminals.pin2,
            -voltage_volts,
            spec.component_id.clone(),
        ));
        adjacency[spec.terminals.pin2].push((
            spec.terminals.pin1,
            voltage_volts,
            spec.component_id.clone(),
        ));
    }

    let mut potentials = vec![None::<f64>; node_count];
    let mut diagnostics = Vec::new();
    let mut reported_conflicts = HashSet::new();
    for start in 0..node_count {
        if potentials[start].is_some() || adjacency[start].is_empty() {
            continue;
        }
        potentials[start] = Some(0.0);
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            let current_potential = potentials[node].expect("visited node has a potential");
            for (neighbor, delta, source_id) in &adjacency[node] {
                let expected = current_potential + delta;
                if !expected.is_finite() {
                    diagnostics.push(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        source_id,
                    ));
                    continue;
                }
                match potentials[*neighbor] {
                    None => {
                        potentials[*neighbor] = Some(expected);
                        stack.push(*neighbor);
                    }
                    Some(actual) => {
                        let tolerance = f64::EPSILON
                            * (specs.len().max(1) as f64)
                            * 32.0
                            * actual.abs().max(expected.abs()).max(1.0);
                        if (actual - expected).abs() > tolerance
                            && reported_conflicts.insert(source_id.clone())
                        {
                            diagnostics.push(Diagnostic::error(
                                DiagnosticCode::InconsistentIdealConstraint,
                                vec![DiagnosticReference::Component {
                                    component_id: source_id.clone(),
                                }],
                            ));
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

fn check_initial_state_constraints(
    specs: &[DeviceSpec],
    states: &CircuitStates,
    node_count: usize,
    islands: &[ElectricalIsland],
) -> Vec<Diagnostic> {
    type VoltageEdge = (usize, f64, Option<StableId>);

    let mut conflicting_explicit_states = HashSet::<StableId>::new();
    for island in islands {
        let component_ids = island
            .component_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<HashSet<_>>();
        let mut island_specs = specs
            .iter()
            .filter(|spec| component_ids.contains(spec.component_id.as_str()))
            .collect::<Vec<_>>();
        island_specs
            .sort_by(|left, right| left.component_id.as_str().cmp(right.component_id.as_str()));

        let mut adjacency = vec![Vec::<VoltageEdge>::new(); node_count];
        let mut explicit_capacitors = Vec::new();
        for spec in &island_specs {
            let constraint = match spec.model {
                DeviceModel::DcVoltageSource {
                    voltage_volts,
                    internal_resistance_ohms: 0.0,
                } => Some((voltage_volts, None)),
                DeviceModel::Capacitor {
                    initial_voltage_volts: Some(voltage_volts),
                    ..
                } => {
                    explicit_capacitors.push((spec, voltage_volts));
                    None
                }
                DeviceModel::Capacitor {
                    initial_voltage_volts: None,
                    ..
                } => states
                    .capacitors
                    .get(&spec.component_id)
                    .map(|state| (state.voltage_volts, None)),
                _ => None,
            };
            if let Some((voltage, explicit_component_id)) = constraint {
                add_voltage_constraint(
                    &mut adjacency,
                    spec.terminals.pin1,
                    spec.terminals.pin2,
                    voltage,
                    explicit_component_id,
                );
            }
        }

        // Unspecified capacitor states come from the DC solve. If those base
        // constraints are already inconsistent, let the ordinary solver report
        // that failure; it is not caused by a user-imposed initial condition.
        if !voltage_constraints_consistent(&adjacency, island_specs.len()) {
            continue;
        }

        for (spec, voltage_volts) in explicit_capacitors {
            let path = voltage_path(&adjacency, spec.terminals.pin1, spec.terminals.pin2);
            if let Some((actual_delta, path_initial_states)) = path {
                let expected_delta = -voltage_volts;
                let tolerance = f64::EPSILON
                    * (island_specs.len().max(1) as f64)
                    * 32.0
                    * actual_delta.abs().max(expected_delta.abs()).max(1.0);
                if !actual_delta.is_finite() || (actual_delta - expected_delta).abs() > tolerance {
                    conflicting_explicit_states.insert(spec.component_id.clone());
                    conflicting_explicit_states.extend(path_initial_states);
                    continue;
                }
            }
            add_voltage_constraint(
                &mut adjacency,
                spec.terminals.pin1,
                spec.terminals.pin2,
                voltage_volts,
                Some(spec.component_id.clone()),
            );
        }
    }

    // During the initial-state solve, capacitors and ideal voltage sources
    // constrain voltage while resistors and finite source resistance carry
    // arbitrary current. Inductors are imposed current sources. Removing the
    // current-source branches partitions the island into cuts; each partition
    // must have zero net imposed current for KCL to hold.
    for island in islands {
        let component_ids = island
            .component_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<HashSet<_>>();
        let island_specs = specs
            .iter()
            .filter(|spec| component_ids.contains(spec.component_id.as_str()))
            .collect::<Vec<_>>();
        let mut adjacency = vec![Vec::<usize>::new(); node_count];
        for spec in &island_specs {
            if !matches!(spec.model, DeviceModel::Inductor { .. }) {
                adjacency[spec.terminals.pin1].push(spec.terminals.pin2);
                adjacency[spec.terminals.pin2].push(spec.terminals.pin1);
            }
        }
        let mut groups = vec![usize::MAX; node_count];
        let mut group_count = 0;
        for start in 0..node_count {
            if groups[start] != usize::MAX {
                continue;
            }
            groups[start] = group_count;
            let mut stack = vec![start];
            while let Some(node) = stack.pop() {
                for neighbor in &adjacency[node] {
                    if groups[*neighbor] == usize::MAX {
                        groups[*neighbor] = group_count;
                        stack.push(*neighbor);
                    }
                }
            }
            group_count += 1;
        }

        let mut injections = vec![0.0_f64; group_count];
        let mut scales = vec![0.0_f64; group_count];
        for spec in &island_specs {
            if !matches!(spec.model, DeviceModel::Inductor { .. }) {
                continue;
            }
            let Some(state) = states.inductors.get(&spec.component_id) else {
                continue;
            };
            let pin1_group = groups[spec.terminals.pin1];
            let pin2_group = groups[spec.terminals.pin2];
            injections[pin1_group] -= state.current_amps;
            injections[pin2_group] += state.current_amps;
            scales[pin1_group] += state.current_amps.abs();
            scales[pin2_group] += state.current_amps.abs();
        }
        for (group, injection) in injections.iter().enumerate() {
            let tolerance =
                f64::EPSILON * (island_specs.len().max(1) as f64) * 32.0 * scales[group].max(1.0);
            if injection.abs() > tolerance {
                // Only explicit inductor states crossing the unbalanced cut
                // are attributed to this initial-state conflict.
                for spec in &island_specs {
                    let DeviceModel::Inductor {
                        initial_current_amps: Some(_),
                        ..
                    } = spec.model
                    else {
                        continue;
                    };
                    let pin1_group = groups[spec.terminals.pin1];
                    let pin2_group = groups[spec.terminals.pin2];
                    if (pin1_group == group) != (pin2_group == group) {
                        conflicting_explicit_states.insert(spec.component_id.clone());
                    }
                }
            }
        }
    }

    if conflicting_explicit_states.is_empty() {
        return Vec::new();
    }
    let references = conflicting_explicit_states
        .into_iter()
        .map(|component_id| DiagnosticReference::Component { component_id })
        .collect();
    vec![Diagnostic::error(
        DiagnosticCode::InconsistentState,
        references,
    )]
}

fn add_voltage_constraint(
    adjacency: &mut [Vec<(usize, f64, Option<StableId>)>],
    pin1: usize,
    pin2: usize,
    voltage_volts: f64,
    explicit_component_id: Option<StableId>,
) {
    adjacency[pin1].push((pin2, -voltage_volts, explicit_component_id.clone()));
    adjacency[pin2].push((pin1, voltage_volts, explicit_component_id));
}

fn voltage_constraints_consistent(
    adjacency: &[Vec<(usize, f64, Option<StableId>)>],
    constraint_count: usize,
) -> bool {
    let mut potentials = vec![None::<f64>; adjacency.len()];
    for start in 0..adjacency.len() {
        if potentials[start].is_some() || adjacency[start].is_empty() {
            continue;
        }
        potentials[start] = Some(0.0);
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            let current_potential = potentials[node].expect("visited node has a potential");
            for (neighbor, delta, _) in &adjacency[node] {
                let expected = current_potential + delta;
                if !expected.is_finite() {
                    return false;
                }
                match potentials[*neighbor] {
                    None => {
                        potentials[*neighbor] = Some(expected);
                        stack.push(*neighbor);
                    }
                    Some(actual) => {
                        let tolerance = f64::EPSILON
                            * (constraint_count.max(1) as f64)
                            * 32.0
                            * actual.abs().max(expected.abs()).max(1.0);
                        if (actual - expected).abs() > tolerance {
                            return false;
                        }
                    }
                }
            }
        }
    }
    true
}

fn voltage_path(
    adjacency: &[Vec<(usize, f64, Option<StableId>)>],
    start: usize,
    target: usize,
) -> Option<(f64, Vec<StableId>)> {
    let mut potentials = vec![None::<f64>; adjacency.len()];
    let mut parents = vec![None::<(usize, Option<StableId>)>; adjacency.len()];
    potentials[start] = Some(0.0);
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
        if node == target {
            break;
        }
        let current_potential = potentials[node]?;
        for (neighbor, delta, explicit_component_id) in &adjacency[node] {
            if potentials[*neighbor].is_some() {
                continue;
            }
            let potential = current_potential + delta;
            potentials[*neighbor] = Some(potential);
            parents[*neighbor] = Some((node, explicit_component_id.clone()));
            stack.push(*neighbor);
        }
    }
    let voltage_delta = potentials[target]?;
    let mut explicit_states = Vec::new();
    let mut node = target;
    while node != start {
        let (parent, explicit_component_id) = parents[node].clone()?;
        if let Some(component_id) = explicit_component_id {
            explicit_states.push(component_id);
        }
        node = parent;
    }
    Some((voltage_delta, explicit_states))
}

fn diagnostic_for_island(code: DiagnosticCode, island: &ElectricalIsland) -> Diagnostic {
    Diagnostic::error(
        code,
        island
            .component_ids
            .iter()
            .cloned()
            .map(|component_id| DiagnosticReference::Component { component_id })
            .collect(),
    )
}

fn build_result_layout(schematic: &Schematic) -> Result<ResultLayout, Vec<Diagnostic>> {
    let mut components = schematic.components.iter().collect::<Vec<_>>();
    components.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
    let mut slots = Vec::with_capacity(components.len().saturating_mul(2));
    for component in components {
        let definitions = if matches!(component, ComponentSpec::RelaySpdt { .. }) {
            vec![
                (
                    ResultQuantity::Voltage,
                    Unit::Volt,
                    "coil/voltage",
                    ResultSignConvention::VoltagePin1MinusPin2,
                ),
                (
                    ResultQuantity::Current,
                    Unit::Amp,
                    "coil/current",
                    ResultSignConvention::CurrentIntoPin1,
                ),
                (
                    ResultQuantity::Voltage,
                    Unit::Volt,
                    "normally_open/voltage",
                    ResultSignConvention::VoltagePin1MinusPin2,
                ),
                (
                    ResultQuantity::Current,
                    Unit::Amp,
                    "normally_open/current",
                    ResultSignConvention::CurrentIntoPin1,
                ),
                (
                    ResultQuantity::Voltage,
                    Unit::Volt,
                    "normally_closed/voltage",
                    ResultSignConvention::VoltagePin1MinusPin2,
                ),
                (
                    ResultQuantity::Current,
                    Unit::Amp,
                    "normally_closed/current",
                    ResultSignConvention::CurrentIntoPin1,
                ),
            ]
        } else {
            vec![
                (
                    ResultQuantity::Voltage,
                    Unit::Volt,
                    "voltage",
                    ResultSignConvention::VoltagePin1MinusPin2,
                ),
                (
                    ResultQuantity::Current,
                    Unit::Amp,
                    "current",
                    ResultSignConvention::CurrentIntoPin1,
                ),
            ]
        };
        for (quantity, unit, suffix, sign_convention) in definitions {
            let key = if matches!(component, ComponentSpec::RelaySpdt { .. }) {
                format!("{}/{suffix}", component.id())
            } else {
                format!("{}:{suffix}", component.id())
            };
            slots.push(ResultSlot {
                key,
                component_id: component.id().clone(),
                quantity,
                unit,
                sign_convention,
                offset: slots.len() as u32,
            });
        }
    }
    let Some(value_count) = u32::try_from(slots.len()).ok() else {
        return Err(vec![Diagnostic::error(
            DiagnosticCode::InvalidGeometry,
            Vec::new(),
        )]);
    };
    Ok(ResultLayout {
        layout_version: 1,
        value_count,
        slots,
    })
}

fn values_for_layout(
    layout: &ResultLayout,
    results: &[DeviceResult],
) -> Result<Vec<f64>, Vec<Diagnostic>> {
    let by_component = results
        .iter()
        .map(|result| (result.component_id.as_str(), result))
        .collect::<HashMap<_, _>>();
    let mut values = vec![0.0; layout.value_count as usize];
    let mut diagnostics = Vec::new();
    for slot in &layout.slots {
        let Some(result) = by_component.get(slot.component_id.as_str()) else {
            diagnostics.push(component_diagnostic(
                DiagnosticCode::SingularSystem,
                &slot.component_id,
            ));
            continue;
        };
        let value = result
            .extra_values
            .iter()
            .find(|(key, _)| key == &slot.key)
            .map(|(_, value)| *value)
            .unwrap_or_else(|| match slot.quantity {
                ResultQuantity::Voltage => result.voltage_volts,
                ResultQuantity::Current => result.current_amps,
            });
        if value.is_finite() {
            values[slot.offset as usize] = value;
        } else {
            diagnostics.push(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &slot.component_id,
            ));
        }
    }
    if has_errors(&diagnostics) {
        Err(diagnostics)
    } else {
        Ok(values)
    }
}

fn update_parameter(
    schematic: &mut Schematic,
    component_id: &StableId,
    parameter_id: ParameterId,
    value: f64,
) -> bool {
    let Some(component) = schematic
        .components
        .iter_mut()
        .find(|component| component.id() == component_id)
    else {
        return false;
    };
    match component {
        ComponentSpec::Resistor { parameters, .. } => {
            if parameter_id != ParameterId::ResistanceOhms {
                return false;
            }
            parameters.resistance_ohms = value;
        }
        ComponentSpec::DcVoltageSource { parameters, .. } => match parameter_id {
            ParameterId::VoltageVolts => parameters.voltage_volts = value,
            ParameterId::InternalResistanceOhms => {
                parameters.internal_resistance_ohms = value;
            }
            _ => return false,
        },
        ComponentSpec::Capacitor { parameters, .. } => match parameter_id {
            ParameterId::CapacitanceFarads => parameters.capacitance_farads = value,
            _ => return false,
        },
        ComponentSpec::Inductor { parameters, .. } => match parameter_id {
            ParameterId::InductanceHenries => parameters.inductance_henries = value,
            _ => return false,
        },
        ComponentSpec::RelaySpdt { parameters, .. } => match parameter_id {
            ParameterId::CoilResistanceOhms => parameters.coil_resistance_ohms = value,
            ParameterId::CoilInductanceHenries => parameters.coil_inductance_henries = value,
            ParameterId::PickupCurrentAmps => parameters.pickup_current_amps = value,
            ParameterId::DropoutCurrentAmps => parameters.dropout_current_amps = value,
            ParameterId::ClosedContactResistanceOhms => {
                parameters.closed_contact_resistance_ohms = value;
            }
            _ => return false,
        },
    }
    true
}

fn new_stable_uuid() -> StableId {
    StableId::parse(Uuid::new_v4().hyphenated().to_string())
        .expect("a generated UUID is in canonical form")
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_updates_are_atomic_and_revision_checked() {
        let request = test_compile_request();
        let mut engine = Engine::default();
        let CompileResult::Success {
            run_id,
            initial_snapshot,
            ..
        } = engine.compile(request)
        else {
            panic!("divider should compile");
        };
        let component_id = StableId::parse("00000000-0000-4000-8000-000000000002").unwrap();

        let invalid = engine.set_parameter(SetParameterRequest {
            run_id: run_id.clone(),
            expected_document_revision: "revision-1".to_owned(),
            document_revision: "revision-2".to_owned(),
            component_id: component_id.clone(),
            parameter_id: ParameterId::ResistanceOhms,
            value: 0.0,
        });
        let SetParameterResult::Failure {
            diagnostics: invalid_diagnostics,
        } = invalid
        else {
            panic!("zero resistance update should fail");
        };
        assert!(
            invalid_diagnostics
                .iter()
                .any(|item| item.code == DiagnosticCode::InvalidParameter)
        );
        assert_eq!(engine.latest_snapshot(), Some(&initial_snapshot));

        let stale_revision = engine.set_parameter(SetParameterRequest {
            run_id: run_id.clone(),
            expected_document_revision: "revision-1".to_owned(),
            document_revision: "revision-1".to_owned(),
            component_id: component_id.clone(),
            parameter_id: ParameterId::ResistanceOhms,
            value: 2000.0,
        });
        let SetParameterResult::Failure {
            diagnostics: stale_revision_diagnostics,
        } = stale_revision
        else {
            panic!("unchanged document revision should fail");
        };
        assert!(
            stale_revision_diagnostics
                .iter()
                .any(|item| item.code == DiagnosticCode::StaleDocumentRevision)
        );
        assert_eq!(engine.latest_snapshot(), Some(&initial_snapshot));

        let wrong_run = engine.set_parameter(SetParameterRequest {
            run_id: StableId::parse("00000000-0000-4000-8000-000000000099").unwrap(),
            expected_document_revision: "revision-1".to_owned(),
            document_revision: "revision-2".to_owned(),
            component_id,
            parameter_id: ParameterId::ResistanceOhms,
            value: 2000.0,
        });
        let SetParameterResult::Failure {
            diagnostics: wrong_run_diagnostics,
        } = wrong_run
        else {
            panic!("stale run ID should fail");
        };
        assert!(
            wrong_run_diagnostics
                .iter()
                .any(|item| item.code == DiagnosticCode::StaleRun)
        );
        assert_eq!(engine.latest_snapshot(), Some(&initial_snapshot));
    }

    #[test]
    fn out_of_order_parameter_updates_cannot_regress_the_active_revision() {
        let mut engine = Engine::default();
        let CompileResult::Success { run_id, .. } = engine.compile(test_compile_request()) else {
            panic!("divider should compile");
        };
        let component_id = StableId::parse("00000000-0000-4000-8000-000000000002").unwrap();

        let SetParameterResult::Success { snapshot, .. } =
            engine.set_parameter(SetParameterRequest {
                run_id: run_id.clone(),
                expected_document_revision: "revision-1".to_owned(),
                document_revision: "revision-2".to_owned(),
                component_id: component_id.clone(),
                parameter_id: ParameterId::ResistanceOhms,
                value: 2000.0,
            })
        else {
            panic!("first valid update should succeed");
        };

        let delayed_update = engine.set_parameter(SetParameterRequest {
            run_id: run_id.clone(),
            expected_document_revision: "revision-1".to_owned(),
            document_revision: "revision-3".to_owned(),
            component_id: component_id.clone(),
            parameter_id: ParameterId::ResistanceOhms,
            value: 3000.0,
        });
        assert_stale_revision(delayed_update);
        assert_eq!(engine.latest_snapshot(), Some(&snapshot));
    }

    #[test]
    fn trace_history_samples_each_public_step_and_rejects_stale_runs() {
        let mut engine = Engine::default();
        let CompileResult::Success { run_id, layout, .. } = engine.compile(test_compile_request())
        else {
            panic!("divider should compile");
        };
        let signal_key = layout.slots[0].key.clone();
        engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 3,
            })
            .unwrap();

        let batch = engine
            .read_traces(TraceRequest {
                run_id: run_id.clone(),
                layout_version: layout.layout_version,
                signal_keys: vec![signal_key],
                query: crate::TraceQuery::History {
                    start_step: 0,
                    end_step: 3,
                },
                max_points: 16,
            })
            .unwrap();
        assert_eq!(
            batch.metadata.sample_interval_seconds,
            SIMULATION_TIMESTEP_SECONDS
        );
        assert_eq!(batch.metadata.sample_count, 4);
        assert_eq!(batch.metadata.first_sample_step, Some(0));
        assert_eq!(batch.metadata.last_sample_step, Some(3));
        assert_eq!(batch.points.len(), 4);
        assert_eq!(
            batch
                .points
                .iter()
                .map(|point| point.simulation_step)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        for pair in batch.points.windows(2) {
            assert!(pair[0].simulation_step < pair[1].simulation_step);
        }
        for point in &batch.points {
            let time = point.simulation_step as f64 * batch.metadata.sample_interval_seconds;
            assert!(time.is_finite());
            assert_eq!(time, point.simulation_step as f64 * 0.001);
        }

        let stale = engine.read_traces(TraceRequest {
            run_id: StableId::parse("00000000-0000-4000-8000-000000000099").unwrap(),
            layout_version: layout.layout_version,
            signal_keys: vec![layout.slots[0].key.clone()],
            query: crate::TraceQuery::History {
                start_step: 0,
                end_step: 3,
            },
            max_points: 16,
        });
        assert!(stale.is_err());
        assert_eq!(stale.unwrap_err()[0].code, DiagnosticCode::StaleRun);

        let excessive_points = engine.read_traces(TraceRequest {
            run_id: run_id.clone(),
            layout_version: layout.layout_version,
            signal_keys: vec![layout.slots[0].key.clone()],
            query: crate::TraceQuery::History {
                start_step: 0,
                end_step: 3,
            },
            max_points: 4097,
        });
        assert_eq!(
            excessive_points.unwrap_err()[0].code,
            DiagnosticCode::InvalidParameter
        );

        let stale_layout = engine.read_traces(TraceRequest {
            run_id,
            layout_version: layout.layout_version + 1,
            signal_keys: vec![layout.slots[0].key.clone()],
            query: crate::TraceQuery::History {
                start_step: 0,
                end_step: 3,
            },
            max_points: 8,
        });
        assert_eq!(stale_layout.unwrap_err()[0].code, DiagnosticCode::StaleRun);
    }

    #[test]
    fn trace_replaces_the_current_sample_on_live_parameter_update() {
        let mut engine = Engine::default();
        let CompileResult::Success { run_id, layout, .. } = engine.compile(test_compile_request())
        else {
            panic!("divider should compile");
        };
        engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        let component_id = StableId::parse("00000000-0000-4000-8000-000000000002").unwrap();
        assert!(matches!(
            engine.set_parameter(SetParameterRequest {
                run_id: run_id.clone(),
                expected_document_revision: "revision-1".to_owned(),
                document_revision: "revision-2".to_owned(),
                component_id,
                parameter_id: ParameterId::ResistanceOhms,
                value: 2000.0,
            }),
            SetParameterResult::Success { .. }
        ));

        let replaced = engine
            .read_traces(TraceRequest {
                run_id: run_id.clone(),
                layout_version: layout.layout_version,
                signal_keys: vec![layout.slots[0].key.clone()],
                query: crate::TraceQuery::NewSamples {
                    after_step: 1,
                    after_document_revision: "revision-1".to_owned(),
                },
                max_points: 8,
            })
            .unwrap();
        assert!(replaced.metadata.replaced);
        assert_eq!(replaced.metadata.document_revision, "revision-2");
        assert_eq!(replaced.metadata.sample_count, 1);
        assert_eq!(replaced.points[0].simulation_step, 1);

        engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        let replaced_before_latest = engine
            .read_traces(TraceRequest {
                run_id: run_id.clone(),
                layout_version: layout.layout_version,
                signal_keys: vec![layout.slots[0].key.clone()],
                query: crate::TraceQuery::NewSamples {
                    after_step: 1,
                    after_document_revision: "revision-1".to_owned(),
                },
                max_points: 8,
            })
            .unwrap();
        assert!(replaced_before_latest.metadata.replaced);
        assert_eq!(replaced_before_latest.metadata.sample_count, 2);
        assert_eq!(
            replaced_before_latest
                .points
                .iter()
                .map(|point| point.simulation_step)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );

        let CompileResult::Success {
            run_id: next_run, ..
        } = engine.compile(test_compile_request())
        else {
            panic!("divider should recompile");
        };
        let old_run = engine.read_traces(TraceRequest {
            run_id,
            layout_version: layout.layout_version,
            signal_keys: vec![layout.slots[0].key.clone()],
            query: crate::TraceQuery::History {
                start_step: 0,
                end_step: 1,
            },
            max_points: 8,
        });
        assert!(old_run.is_err());
        let new_run = engine
            .read_traces(TraceRequest {
                run_id: next_run,
                layout_version: layout.layout_version,
                signal_keys: vec![layout.slots[0].key.clone()],
                query: crate::TraceQuery::History {
                    start_step: 0,
                    end_step: 0,
                },
                max_points: 8,
            })
            .unwrap();
        assert_eq!(new_run.metadata.oldest_sample_step, Some(0));
        assert_eq!(new_run.metadata.latest_sample_step, Some(0));
        assert_eq!(new_run.metadata.sample_count, 1);
    }

    fn assert_stale_revision(result: SetParameterResult) {
        let SetParameterResult::Failure { diagnostics } = result else {
            panic!("stale revision update should fail");
        };
        assert!(
            diagnostics
                .iter()
                .any(|item| item.code == DiagnosticCode::StaleDocumentRevision)
        );
    }

    fn test_compile_request() -> CompileRequest {
        CompileRequest {
            schematic: serde_json::from_str(include_str!(
                "../../../tests/golden/stage-1-divider.schematic.json"
            ))
            .unwrap(),
            geometry: serde_json::from_str(include_str!(
                "../../../tests/golden/stage-1-divider.geometry.json"
            ))
            .unwrap(),
            document_revision: "revision-1".to_owned(),
        }
    }
}
