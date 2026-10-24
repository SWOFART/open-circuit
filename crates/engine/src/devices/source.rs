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
            || !system.stamp_conductance(
                variables,
                self.terminals.pin1,
                self.terminals.pin2,
                conductance,
            )
            || !system.stamp_current_injection(
                variables,
                self.terminals.pin1,
                self.terminals.pin2,
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
        })
    }
}
