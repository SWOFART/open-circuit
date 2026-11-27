use std::{collections::HashSet, f64::consts::TAU};

use crate::{
    ComponentSpec, DiagnosticCode, GeometryEnvelope, Schematic, StableId,
    mna::NodeVariables,
    netlist::{PinRef, derive_netlist},
    solver::{DenseLuSolver, Solver, SolverError},
};

use super::{
    AnalysisError, MAX_ANALYSIS_UNKNOWNS, check_cancelled, invalid_parameter, validate_snapshot,
};
use crate::engine::{
    DeviceModel, DeviceSpec, SolveMode, build_device_specs, diagnostic_for_island,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrequencySpacing {
    Linear,
    Logarithmic,
}

#[derive(Debug)]
pub struct FrequencyResponseRequest<'a> {
    pub schematic: &'a Schematic,
    pub geometry: &'a GeometryEnvelope,
    pub document_revision: &'a str,
    pub source_id: &'a StableId,
    pub output: [&'a PinRef; 2],
    pub start_hz: f64,
    pub stop_hz: f64,
    pub point_count: usize,
    pub spacing: FrequencySpacing,
}

#[derive(Debug, PartialEq)]
pub struct FrequencyResponsePoint {
    pub frequency_hz: f64,
    pub magnitude: f64,
    pub phase_degrees: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub struct FrequencyResponseResult {
    pub document_revision: String,
    pub source_id: StableId,
    pub output: [PinRef; 2],
    pub points: Vec<FrequencyResponsePoint>,
}

/// Solve the complex small-signal response of a linear circuit.
pub fn frequency_response(
    request: FrequencyResponseRequest<'_>,
    is_cancelled: impl Fn() -> bool,
) -> Result<FrequencyResponseResult, AnalysisError> {
    check_cancelled(&is_cancelled)?;
    validate_snapshot(
        request.schematic,
        request.geometry,
        request.document_revision,
    )?;
    if !(2..=257).contains(&request.point_count)
        || !(0.01..=100_000.0).contains(&request.start_hz)
        || !(0.01..=100_000.0).contains(&request.stop_hz)
        || request.start_hz >= request.stop_hz
    {
        return Err(invalid_parameter());
    }

    let mut unsupported: Vec<_> = request
        .schematic
        .components
        .iter()
        .filter(|component| {
            !matches!(
                component,
                ComponentSpec::DcVoltageSource { .. }
                    | ComponentSpec::Resistor { .. }
                    | ComponentSpec::Capacitor { .. }
                    | ComponentSpec::Inductor { .. }
            )
        })
        .map(|component| component.id().clone())
        .collect();
    if !unsupported.is_empty() {
        unsupported.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        return Err(AnalysisError::UnsupportedCircuit(unsupported));
    }

    let netlist = derive_netlist(request.schematic, request.geometry)
        .map_err(AnalysisError::InvalidRequest)?;
    let specs = build_device_specs(request.schematic, &netlist, SolveMode::OperatingPoint)
        .map_err(AnalysisError::InvalidRequest)?;
    if !specs.iter().any(|spec| {
        spec.component_id == *request.source_id
            && matches!(spec.model, DeviceModel::DcVoltageSource { .. })
    }) {
        return Err(invalid_parameter());
    }
    let island = netlist
        .islands
        .iter()
        .find(|island| island.component_ids.contains(request.source_id))
        .ok_or_else(invalid_parameter)?;
    let reference = island.reference_node.ok_or_else(|| {
        AnalysisError::InvalidRequest(vec![diagnostic_for_island(
            DiagnosticCode::FloatingNode,
            island,
        )])
    })?;
    let find_output_node = |pin: &PinRef| {
        island
            .node_indices
            .iter()
            .copied()
            .find(|&node| netlist.nodes[node].pins.contains(pin))
            .ok_or_else(invalid_parameter)
    };
    let [positive, negative] = request.output;
    let output = [find_output_node(positive)?, find_output_node(negative)?];
    let variables = NodeVariables::for_island(
        netlist.nodes.len(),
        &island.node_indices,
        reference,
        &HashSet::new(),
    );
    let specs: Vec<_> = specs
        .iter()
        .filter(|spec| island.component_ids.contains(&spec.component_id))
        .collect();
    let voltage_count = variables.voltage_variable_count();
    let size = voltage_count
        + specs
            .iter()
            .filter(|spec| {
                matches!(
                    spec.model,
                    DeviceModel::DcVoltageSource { .. } | DeviceModel::Inductor { .. }
                )
            })
            .count();
    if size > MAX_ANALYSIS_UNKNOWNS {
        return Err(AnalysisError::InvalidRequest(vec![diagnostic_for_island(
            DiagnosticCode::InvalidGeometry,
            island,
        )]));
    }

    let node_output = output.map(|node| variables.voltage_variable(node));
    let mut points = Vec::with_capacity(request.point_count);
    let mut last_hz = None;
    for point_index in 0..request.point_count {
        check_cancelled(&is_cancelled)?;
        let frequency_hz = frequency_at(&request, point_index);
        if last_hz.is_some_and(|previous| frequency_hz <= previous) {
            return Err(invalid_parameter());
        }
        let mut system = ComplexSystem::new(size, voltage_count);
        for spec in &specs {
            system.stamp(spec, &variables, request.source_id, frequency_hz);
        }
        let response = system.solve(node_output).map_err(|error| {
            let code = match error {
                SolverError::NonFinite => DiagnosticCode::NonFiniteState,
                SolverError::ResidualTooLarge => DiagnosticCode::NonConvergence,
                _ => DiagnosticCode::SingularSystem,
            };
            AnalysisError::SolveFailed {
                point_index,
                diagnostics: vec![diagnostic_for_island(code, island)],
            }
        })?;
        points.push(response.point(frequency_hz));
        last_hz = Some(frequency_hz);
    }
    check_cancelled(&is_cancelled)?;
    Ok(FrequencyResponseResult {
        document_revision: request.document_revision.to_owned(),
        source_id: request.source_id.clone(),
        output: request.output.map(Clone::clone),
        points,
    })
}

fn frequency_at(request: &FrequencyResponseRequest<'_>, index: usize) -> f64 {
    let last = request.point_count - 1;
    if index == 0 {
        return request.start_hz;
    }
    if index == last {
        return request.stop_hz;
    }
    let fraction = index as f64 / last as f64;
    match request.spacing {
        FrequencySpacing::Linear => {
            request.start_hz + (request.stop_hz - request.start_hz) * fraction
        }
        FrequencySpacing::Logarithmic => {
            request.start_hz * (request.stop_hz / request.start_hz).powf(fraction)
        }
    }
}

struct ComplexVoltage {
    real: f64,
    imaginary: f64,
    magnitude: f64,
}

impl ComplexVoltage {
    fn point(self, frequency_hz: f64) -> FrequencyResponsePoint {
        FrequencyResponsePoint {
            frequency_hz,
            magnitude: self.magnitude,
            phase_degrees: (self.magnitude != 0.0).then(|| {
                let degrees = self.imaginary.atan2(self.real).to_degrees();
                if degrees <= -180.0 { 180.0 } else { degrees }
            }),
        }
    }
}

/// Store the complex MNA matrix as real blocks [Re -Im; Im Re] for dense LU.
struct ComplexSystem {
    size: usize,
    matrix: Vec<Vec<f64>>,
    rhs: Vec<f64>,
    next_branch: usize,
}

impl ComplexSystem {
    fn new(size: usize, next_branch: usize) -> Self {
        Self {
            size,
            matrix: vec![vec![0.0; 2 * size]; 2 * size],
            rhs: vec![0.0; 2 * size],
            next_branch,
        }
    }

    fn add(&mut self, row: usize, column: usize, real: f64, imaginary: f64) {
        let size = self.size;
        self.matrix[row][column] += real;
        self.matrix[row + size][column + size] += real;
        self.matrix[row][column + size] -= imaginary;
        self.matrix[row + size][column] += imaginary;
    }

    fn stamp(&mut self, spec: &DeviceSpec, variables: &NodeVariables, source: &StableId, hz: f64) {
        let plus = variables.voltage_variable(spec.terminals.pin1);
        let minus = variables.voltage_variable(spec.terminals.pin2);
        let terminals = [(plus, 1.0), (minus, -1.0)];
        let omega = TAU * hz;
        let (real, imaginary) = match spec.model {
            DeviceModel::Resistor { resistance_ohms } => (1.0 / resistance_ohms, 0.0),
            DeviceModel::Capacitor {
                capacitance_farads, ..
            } => (0.0, omega * capacitance_farads),
            DeviceModel::DcVoltageSource {
                internal_resistance_ohms,
                ..
            } => {
                let excitation = if spec.component_id == *source {
                    1.0
                } else {
                    0.0
                };
                self.stamp_branch(terminals, internal_resistance_ohms, 0.0, excitation);
                return;
            }
            DeviceModel::Inductor {
                inductance_henries, ..
            } => {
                self.stamp_branch(terminals, 0.0, omega * inductance_henries, 0.0);
                return;
            }
            _ => unreachable!("validated linear circuit"),
        };
        if plus != minus {
            for (row, sign) in terminals {
                for (column, other_sign) in terminals {
                    if let (Some(row), Some(column)) = (row, column) {
                        self.add(
                            row,
                            column,
                            real * sign * other_sign,
                            imaginary * sign * other_sign,
                        );
                    }
                }
            }
        }
    }

    fn stamp_branch(
        &mut self,
        terminals: [(Option<usize>, f64); 2],
        resistance: f64,
        reactance: f64,
        excitation: f64,
    ) {
        let branch = self.next_branch;
        self.next_branch += 1;
        for (node, sign) in terminals {
            if let Some(node) = node {
                self.add(node, branch, sign, 0.0);
                self.add(branch, node, sign, 0.0);
            }
        }
        self.add(branch, branch, -resistance, -reactance);
        self.rhs[branch] = excitation;
    }

    fn solve(&self, [plus, minus]: [Option<usize>; 2]) -> Result<ComplexVoltage, SolverError> {
        let values = DenseLuSolver::default()
            .solve(&self.matrix, &self.rhs)?
            .values;
        let at = |node: Option<usize>, offset| node.map_or(0.0, |node| values[node + offset]);
        let real = at(plus, 0) - at(minus, 0);
        let imaginary = at(plus, self.size) - at(minus, self.size);
        let magnitude = real.hypot(imaginary);
        if !magnitude.is_finite() {
            return Err(SolverError::NonFinite);
        }
        Ok(ComplexVoltage {
            real,
            imaginary,
            magnitude,
        })
    }
}
