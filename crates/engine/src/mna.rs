//! Modified nodal analysis matrix assembly for Stage 1 devices.

use crate::solver::{DenseLuSolver, Solver, SolverError, SolverSolution};

pub(crate) struct NodeVariables {
    node_to_variable: Vec<Option<usize>>,
    voltage_variable_count: usize,
}

impl NodeVariables {
    pub(crate) fn for_island(
        node_count: usize,
        island_nodes: &[usize],
        reference_node: usize,
        additional_reference_nodes: &std::collections::HashSet<usize>,
    ) -> Self {
        let mut node_to_variable = vec![None; node_count];
        let mut voltage_variable_count = 0;
        for node in island_nodes.iter().copied() {
            if node != reference_node && !additional_reference_nodes.contains(&node) {
                node_to_variable[node] = Some(voltage_variable_count);
                voltage_variable_count += 1;
            }
        }
        Self {
            node_to_variable,
            voltage_variable_count,
        }
    }

    pub(crate) fn voltage_variable(&self, node: usize) -> Option<usize> {
        self.node_to_variable[node]
    }

    pub(crate) const fn voltage_variable_count(&self) -> usize {
        self.voltage_variable_count
    }
}

pub(crate) struct MnaSystem {
    matrix: Vec<Vec<f64>>,
    rhs: Vec<f64>,
}

impl MnaSystem {
    pub(crate) fn new(variable_count: usize) -> Self {
        Self {
            matrix: vec![vec![0.0; variable_count]; variable_count],
            rhs: vec![0.0; variable_count],
        }
    }

    /// Stamp conductance between two nodes: `G·(Vp−Vn)` enters pin 1 and leaves pin 2.
    pub(crate) fn stamp_conductance(
        &mut self,
        variables: &NodeVariables,
        pin1_node: usize,
        pin2_node: usize,
        conductance_siemens: f64,
    ) -> bool {
        if !conductance_siemens.is_finite() || conductance_siemens < 0.0 {
            return false;
        }
        let pin1 = variables.voltage_variable(pin1_node);
        let pin2 = variables.voltage_variable(pin2_node);
        if pin1 == pin2 {
            return true;
        }
        if let Some(pin1) = pin1
            && !self.add_matrix(pin1, pin1, conductance_siemens)
        {
            return false;
        }
        if let Some(pin2) = pin2
            && !self.add_matrix(pin2, pin2, conductance_siemens)
        {
            return false;
        }
        if let (Some(pin1), Some(pin2)) = (pin1, pin2)
            && (!self.add_matrix(pin1, pin2, -conductance_siemens)
                || !self.add_matrix(pin2, pin1, -conductance_siemens))
        {
            return false;
        }
        true
    }

    /// Stamp current injected into pin 1 and drawn from pin 2 on the right-hand side.
    pub(crate) fn stamp_current_injection(
        &mut self,
        variables: &NodeVariables,
        pin1_node: usize,
        pin2_node: usize,
        current_into_pin1_amps: f64,
    ) -> bool {
        if !current_into_pin1_amps.is_finite() {
            return false;
        }
        let pin1 = variables.voltage_variable(pin1_node);
        let pin2 = variables.voltage_variable(pin2_node);
        if pin1 == pin2 {
            return true;
        }
        if let Some(pin1) = pin1
            && !self.add_rhs(pin1, current_into_pin1_amps)
        {
            return false;
        }
        if let Some(pin2) = pin2
            && !self.add_rhs(pin2, -current_into_pin1_amps)
        {
            return false;
        }
        true
    }

    /// Stamp an ideal source with branch current positive into pin 1.
    pub(crate) fn stamp_ideal_voltage_source(
        &mut self,
        variables: &NodeVariables,
        pin1_node: usize,
        pin2_node: usize,
        branch_variable: usize,
        voltage_volts: f64,
    ) -> bool {
        if !voltage_volts.is_finite() || branch_variable >= self.rhs.len() {
            return false;
        }

        let pin1 = variables.voltage_variable(pin1_node);
        let pin2 = variables.voltage_variable(pin2_node);
        if let Some(pin1) = pin1
            && (!self.add_matrix(pin1, branch_variable, 1.0)
                || !self.add_matrix(branch_variable, pin1, 1.0))
        {
            return false;
        }
        if let Some(pin2) = pin2
            && (!self.add_matrix(pin2, branch_variable, -1.0)
                || !self.add_matrix(branch_variable, pin2, -1.0))
        {
            return false;
        }
        self.add_rhs(branch_variable, voltage_volts)
    }

    pub(crate) fn solve(&self) -> Result<SolverSolution, SolverError> {
        DenseLuSolver::default().solve(&self.matrix, &self.rhs)
    }

    fn add_matrix(&mut self, row: usize, column: usize, value: f64) -> bool {
        self.matrix[row][column] += value;
        self.matrix[row][column].is_finite()
    }

    fn add_rhs(&mut self, row: usize, value: f64) -> bool {
        self.rhs[row] += value;
        self.rhs[row].is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conductance_stamp_uses_reference_node_and_symmetric_signs() {
        let variables = NodeVariables::for_island(2, &[0, 1], 1, &std::collections::HashSet::new());
        let mut system = MnaSystem::new(1);
        assert!(system.stamp_conductance(&variables, 0, 1, 0.5));
        let solved = system.solve().unwrap();
        assert_eq!(solved.values, vec![0.0]);
    }

    #[test]
    fn ideal_source_stamp_sets_terminal_voltage_with_pin1_current_sign() {
        let variables = NodeVariables::for_island(2, &[0, 1], 1, &std::collections::HashSet::new());
        let mut system = MnaSystem::new(2);
        assert!(system.stamp_conductance(&variables, 0, 1, 1.0));
        assert!(system.stamp_ideal_voltage_source(&variables, 0, 1, 1, 5.0));
        let solved = system.solve().unwrap();
        assert!((solved.values[0] - 5.0).abs() < 1e-12);
        assert!((solved.values[1] + 5.0).abs() < 1e-12);
    }
}
