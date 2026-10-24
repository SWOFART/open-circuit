//! Stateful Stage 1 Engine API for compiling and updating resistive circuits.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentSpec, Diagnostic, DiagnosticCode,
    DiagnosticReference, DiagnosticSeverity, GeometryEnvelope, ParameterId, PinId, ResultLayout,
    ResultQuantity, ResultSignConvention, ResultSlot, ResultSnapshot, SIMULATION_TIMESTEP_SECONDS,
    Schematic, SetParameterRequest, SetParameterResult, StableId, Unit,
    devices::{
        DcVoltageSourceDevice, Device, DeviceResult, ResistorDevice, Terminals,
        component_diagnostic,
    },
    mna::{MnaSystem, NodeVariables},
    netlist::{ElectricalIsland, Netlist, PinRef, derive_netlist},
    solver::SolverError,
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
}

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
}

impl Engine {
    /// Validate, compile, and solve the operating point at simulation step zero.
    pub fn compile(&mut self, request: CompileRequest) -> CompileResult {
        let (device_results, diagnostics) =
            match compute_operating_point(&request.schematic, &request.geometry) {
                Ok(computed) => computed,
                Err(diagnostics) => return CompileResult::Failure { diagnostics },
            };
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
        };

        self.compiled = Some(CompiledRun {
            run_id: run_id.clone(),
            schematic: request.schematic,
            geometry: request.geometry,
            layout: layout.clone(),
            snapshot: snapshot.clone(),
        });

        CompileResult::Success {
            run_id,
            layout,
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
        let (device_results, diagnostics) = match compute_operating_point(&candidate, &geometry) {
            Ok(computed) => computed,
            Err(diagnostics) => return SetParameterResult::Failure { diagnostics },
        };
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
        };
        let active = self
            .compiled
            .as_mut()
            .expect("active run was checked above");
        active.schematic = candidate;
        active.snapshot = snapshot.clone();
        SetParameterResult::Success {
            snapshot,
            diagnostics,
        }
    }

    /// Advance the fixed-step transient scaffold. Stage 1 devices are memoryless, so the
    /// operating point is unchanged while simulation metadata advances.
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
        let simulation_time_seconds = simulation_step as f64 * SIMULATION_TIMESTEP_SECONDS;
        if !simulation_time_seconds.is_finite() {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        }

        let mut snapshot = active.snapshot.clone();
        snapshot.metadata.simulation_step = simulation_step;
        snapshot.metadata.simulation_time_seconds = simulation_time_seconds;
        snapshot.metadata.frame_sequence = frame_sequence;
        active.snapshot = snapshot.clone();
        Ok(snapshot)
    }

    pub fn latest_snapshot(&self) -> Option<&ResultSnapshot> {
        self.compiled.as_ref().map(|active| &active.snapshot)
    }

    pub fn result_layout(&self) -> Option<&ResultLayout> {
        self.compiled.as_ref().map(|active| &active.layout)
    }
}

fn compute_operating_point(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
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

    let constraint_errors = check_ideal_source_constraints(&device_specs, netlist.nodes.len());
    if !constraint_errors.is_empty() {
        diagnostics.extend(constraint_errors);
        sort_diagnostics(&mut diagnostics);
        return Err(diagnostics);
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

        let variables =
            NodeVariables::for_island(netlist.nodes.len(), &island.node_indices, reference_node);
        let mut branch_variables = HashMap::new();
        let mut next_variable = variables.voltage_variable_count();
        for spec in &island_specs {
            if matches!(
                spec.model,
                DeviceModel::DcVoltageSource {
                    internal_resistance_ohms: 0.0,
                    ..
                }
            ) {
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
        let pin1 = pin_to_node.get(&PinRef {
            component_id: component.id().clone(),
            pin_id: PinId::Pin1,
        });
        let pin2 = pin_to_node.get(&PinRef {
            component_id: component.id().clone(),
            pin_id: PinId::Pin2,
        });
        let (Some(pin1), Some(pin2)) = (pin1, pin2) else {
            diagnostics.push(component_diagnostic(
                DiagnosticCode::InvalidGeometry,
                component.id(),
            ));
            continue;
        };
        let terminals = Terminals {
            pin1: *pin1,
            pin2: *pin2,
        };

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

fn make_device(spec: &DeviceSpec, branch_variable: Option<usize>) -> Box<dyn Device> {
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
    }
}

fn check_ideal_source_constraints(specs: &[DeviceSpec], node_count: usize) -> Vec<Diagnostic> {
    let mut adjacency = vec![Vec::<(usize, f64, StableId)>::new(); node_count];
    for spec in specs {
        let DeviceModel::DcVoltageSource {
            voltage_volts,
            internal_resistance_ohms: 0.0,
        } = spec.model
        else {
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
    let Some(value_count) = schematic
        .components
        .len()
        .checked_mul(2)
        .and_then(|count| u32::try_from(count).ok())
    else {
        return Err(vec![Diagnostic::error(
            DiagnosticCode::InvalidGeometry,
            Vec::new(),
        )]);
    };

    let mut components = schematic.components.iter().collect::<Vec<_>>();
    components.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
    let mut slots = Vec::with_capacity(value_count as usize);
    for component in components {
        for (quantity, unit, suffix, sign_convention) in [
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
        ] {
            slots.push(ResultSlot {
                key: format!("{}:{suffix}", component.id()),
                component_id: component.id().clone(),
                quantity,
                unit,
                sign_convention,
                offset: slots.len() as u32,
            });
        }
    }
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
        let value = match slot.quantity {
            ResultQuantity::Voltage => result.voltage_volts,
            ResultQuantity::Current => result.current_amps,
        };
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
            ParameterId::ResistanceOhms => return false,
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
