//! Transient-state handling and deterministic internal-step selection.

use std::collections::{HashMap, HashSet};

use crate::{
    ComponentSpec, Diagnostic, DiagnosticCode, GeometryEnvelope, SIMULATION_TIMESTEP_SECONDS,
    Schematic, StableId, devices::DeviceResult, netlist::derive_netlist,
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

#[derive(Clone, Default)]
pub(super) struct CircuitStates {
    pub(super) capacitors: HashMap<StableId, CapacitorState>,
    pub(super) inductors: HashMap<StableId, InductorState>,
}

#[derive(Clone, Copy)]
pub(super) enum SolveMode<'a> {
    OperatingPoint,
    InitialState(&'a CircuitStates),
    Transient {
        step_seconds: f64,
        states: &'a CircuitStates,
    },
}

pub(super) fn circuit_states_from_results(
    schematic: &Schematic,
    results: &[DeviceResult],
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
    CircuitStates {
        capacitors,
        inductors,
    }
}

pub(super) fn advance_public_steps(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
    mut states: CircuitStates,
    internal_substeps: u32,
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
    for _ in 0..public_step_count {
        step_start_states = states.clone();
        for _ in 0..internal_substeps {
            let (step_results, step_diagnostics) = super::compute_circuit(
                schematic,
                geometry,
                SolveMode::Transient {
                    step_seconds: internal_step_seconds,
                    states: &states,
                },
            )?;
            states = circuit_states_from_results(schematic, &step_results);
            results = step_results;
            diagnostics.extend(step_diagnostics);
        }
    }
    sort_diagnostics(&mut diagnostics);
    Ok((results, states, step_start_states, diagnostics))
}

pub(super) fn choose_internal_substeps(
    schematic: &Schematic,
    geometry: &GeometryEnvelope,
) -> (u32, bool) {
    const MAX_SUBSTEPS: u32 = 64;
    let omega_hat = estimate_transient_rate_bound(schematic, geometry).unwrap_or(f64::MAX);
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
) -> Option<f64> {
    let netlist = derive_netlist(schematic, geometry).ok()?;
    let specs = super::build_device_specs(schematic, &netlist).ok()?;
    let mut maximum_rate: f64 = 0.0;

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
            .filter(|spec| matches!(spec.model, DeviceModel::Inductor { .. }))
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
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
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

        let mut state_matrix =
            vec![vec![0.0; dynamic.len() + l_values.len()]; dynamic.len() + l_values.len()];
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

        let scales = dynamic
            .iter()
            .map(|index| capacitance[*index][*index].sqrt())
            .chain(l_values.iter().map(|inductance| inductance.sqrt()))
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
