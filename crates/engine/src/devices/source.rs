use crate::{Diagnostic, DiagnosticCode, StableId};

use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

pub(crate) struct DcVoltageSourceDevice {
    component_id: StableId,
    terminals: Terminals,
    voltage_volts: f64,
    internal_resistance_ohms: f64,
    ideal_branch_variable: Option<usize>,
}

pub(crate) struct ThreePhaseSourceDevice {
    component_id: StableId,
    windings: [Terminals; 3],
    winding_voltages: [f64; 3],
    internal_resistance_ohms: f64,
    star: bool,
    ideal_branch_variables: Option<[usize; 3]>,
}

impl ThreePhaseSourceDevice {
    pub(crate) fn new(
        component_id: StableId,
        windings: [Terminals; 3],
        winding_voltages: [f64; 3],
        internal_resistance_ohms: f64,
        star: bool,
        ideal_branch_variables: Option<[usize; 3]>,
    ) -> Self {
        Self {
            component_id,
            windings,
            winding_voltages,
            internal_resistance_ohms,
            star,
            ideal_branch_variables,
        }
    }

    fn winding_currents(
        &self,
        node_voltages: &[f64],
        solution_values: &[f64],
    ) -> Result<[f64; 3], Diagnostic> {
        let currents = if self.internal_resistance_ohms == 0.0 {
            let Some(branch_variables) = self.ideal_branch_variables else {
                return Err(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &self.component_id,
                ));
            };
            branch_variables.map(|variable| solution_values[variable])
        } else {
            std::array::from_fn(|index| {
                let terminals = self.windings[index];
                (node_voltages[terminals.pin1]
                    - node_voltages[terminals.pin2]
                    - self.winding_voltages[index])
                    / self.internal_resistance_ohms
            })
        };
        if currents.iter().all(|current| current.is_finite()) {
            Ok(currents)
        } else {
            Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.component_id,
            ))
        }
    }
}

impl Device for ThreePhaseSourceDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        if self.internal_resistance_ohms == 0.0 {
            let Some(branch_variables) = self.ideal_branch_variables else {
                return Err(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &self.component_id,
                ));
            };
            for (index, (terminals, branch_variable)) in
                self.windings.iter().zip(branch_variables).enumerate()
            {
                if !system.stamp_ideal_voltage_source(
                    variables,
                    terminals.pin1,
                    terminals.pin2,
                    branch_variable,
                    self.winding_voltages[index],
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
            }
            return Ok(());
        }

        let conductance = 1.0 / self.internal_resistance_ohms;
        if !conductance.is_finite() {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        for (terminals, voltage) in self.windings.iter().zip(self.winding_voltages) {
            let source_current = voltage / self.internal_resistance_ohms;
            if !source_current.is_finite()
                || !system.stamp_norton(
                    variables,
                    terminals.pin1,
                    terminals.pin2,
                    conductance,
                    source_current,
                )
            {
                return Err(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &self.component_id,
                ));
            }
        }
        Ok(())
    }

    fn result(
        &self,
        node_voltages: &[f64],
        solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let winding_voltages = self
            .windings
            .map(|terminals| node_voltages[terminals.pin1] - node_voltages[terminals.pin2]);
        let winding_currents = self.winding_currents(node_voltages, solution_values)?;
        let (voltage_volts, current_amps, extra_values) = if self.star {
            let neutral_current = -winding_currents.iter().sum::<f64>();
            (
                winding_voltages[0],
                winding_currents[0],
                vec![
                    (
                        format!("{}/l1/voltage", self.component_id),
                        winding_voltages[0],
                    ),
                    (
                        format!("{}/l1/current", self.component_id),
                        winding_currents[0],
                    ),
                    (
                        format!("{}/l2/voltage", self.component_id),
                        winding_voltages[1],
                    ),
                    (
                        format!("{}/l2/current", self.component_id),
                        winding_currents[1],
                    ),
                    (
                        format!("{}/l3/voltage", self.component_id),
                        winding_voltages[2],
                    ),
                    (
                        format!("{}/l3/current", self.component_id),
                        winding_currents[2],
                    ),
                    (format!("{}/n/current", self.component_id), neutral_current),
                ],
            )
        } else {
            let terminal_currents = [
                winding_currents[0] - winding_currents[2],
                winding_currents[1] - winding_currents[0],
                winding_currents[2] - winding_currents[1],
            ];
            (
                winding_voltages[0],
                terminal_currents[0],
                vec![
                    (
                        format!("{}/l12/voltage", self.component_id),
                        winding_voltages[0],
                    ),
                    (
                        format!("{}/l23/voltage", self.component_id),
                        winding_voltages[1],
                    ),
                    (
                        format!("{}/l31/voltage", self.component_id),
                        winding_voltages[2],
                    ),
                    (
                        format!("{}/l1/current", self.component_id),
                        terminal_currents[0],
                    ),
                    (
                        format!("{}/l2/current", self.component_id),
                        terminal_currents[1],
                    ),
                    (
                        format!("{}/l3/current", self.component_id),
                        terminal_currents[2],
                    ),
                ],
            )
        };
        if !voltage_volts.is_finite()
            || !current_amps.is_finite()
            || extra_values.iter().any(|(_, value)| !value.is_finite())
        {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.component_id,
            ));
        }
        Ok(DeviceResult {
            component_id: self.component_id.clone(),
            voltage_volts,
            current_amps,
            extra_values,
            state: None,
        })
    }
}

impl DcVoltageSourceDevice {
    pub(crate) fn new(
        component_id: StableId,
        terminals: Terminals,
        voltage_volts: f64,
        internal_resistance_ohms: f64,
        ideal_branch_variable: Option<usize>,
    ) -> Self {
        Self {
            component_id,
            terminals,
            voltage_volts,
            internal_resistance_ohms,
            ideal_branch_variable,
        }
    }
}

impl Device for DcVoltageSourceDevice {
    /// A finite Thevenin source stamps as `G=1/Rs` in parallel with `Is=Vs/Rs`.
    /// At `Rs=0`, it stamps the ideal voltage constraint and its MNA branch-current unknown.
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        if self.internal_resistance_ohms == 0.0 {
            let Some(branch_variable) = self.ideal_branch_variable else {
                return Err(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &self.component_id,
                ));
            };
            if !system.stamp_ideal_voltage_source(
                variables,
                self.terminals.pin1,
                self.terminals.pin2,
                branch_variable,
                self.voltage_volts,
            ) {
                return Err(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &self.component_id,
                ));
            }
            return Ok(());
        }

        let conductance = 1.0 / self.internal_resistance_ohms;
        let source_current = self.voltage_volts / self.internal_resistance_ohms;
        if !conductance.is_finite()
            || !source_current.is_finite()
            || !system.stamp_norton(
                variables,
                self.terminals.pin1,
                self.terminals.pin2,
                conductance,
                source_current,
            )
        {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        Ok(())
    }

    fn result(
        &self,
        node_voltages: &[f64],
        solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let voltage_volts = node_voltages[self.terminals.pin1] - node_voltages[self.terminals.pin2];
        let current_amps = if self.internal_resistance_ohms == 0.0 {
            let Some(branch_variable) = self.ideal_branch_variable else {
                return Err(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &self.component_id,
                ));
            };
            solution_values[branch_variable]
        } else {
            (voltage_volts - self.voltage_volts) / self.internal_resistance_ohms
        };
        if !voltage_volts.is_finite() || !current_amps.is_finite() {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        Ok(DeviceResult {
            component_id: self.component_id.clone(),
            voltage_volts,
            current_amps,
            extra_values: Vec::new(),
            state: None,
        })
    }
}
