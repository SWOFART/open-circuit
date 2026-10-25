use crate::{Diagnostic, DiagnosticCode, StableId};

use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

#[derive(Clone, Copy)]
pub(crate) enum CapacitorMode {
    Open,
    Initial {
        voltage_volts: f64,
        branch_variable: usize,
    },
    Transient {
        step_seconds: f64,
        start_voltage_volts: f64,
        start_current_amps: f64,
    },
}

pub(crate) struct CapacitorDevice {
    component_id: StableId,
    terminals: Terminals,
    capacitance_farads: f64,
    mode: CapacitorMode,
}

impl CapacitorDevice {
    pub(crate) fn new(
        component_id: StableId,
        terminals: Terminals,
        capacitance_farads: f64,
        mode: CapacitorMode,
    ) -> Self {
        Self {
            component_id,
            terminals,
            capacitance_farads,
            mode,
        }
    }

    fn companion(&self) -> Result<(f64, f64), Diagnostic> {
        let CapacitorMode::Transient {
            step_seconds,
            start_voltage_volts,
            start_current_amps,
        } = self.mode
        else {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        };
        let conductance = 2.0 * self.capacitance_farads / step_seconds;
        let history_current = conductance * start_voltage_volts + start_current_amps;
        if !conductance.is_finite() || conductance <= 0.0 || !history_current.is_finite() {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        Ok((conductance, history_current))
    }
}

impl Device for CapacitorDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        match self.mode {
            CapacitorMode::Open => Ok(()),
            CapacitorMode::Initial {
                voltage_volts,
                branch_variable,
            } => {
                if !system.stamp_ideal_voltage_source(
                    variables,
                    self.terminals.pin1,
                    self.terminals.pin2,
                    branch_variable,
                    voltage_volts,
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
                Ok(())
            }
            CapacitorMode::Transient { .. } => {
                let (conductance, history_current) = self.companion()?;
                if !system.stamp_conductance(
                    variables,
                    self.terminals.pin1,
                    self.terminals.pin2,
                    conductance,
                ) || !system.stamp_current_injection(
                    variables,
                    self.terminals.pin1,
                    self.terminals.pin2,
                    history_current,
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
                Ok(())
            }
        }
    }

    fn result(
        &self,
        node_voltages: &[f64],
        solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let voltage_volts = node_voltages[self.terminals.pin1] - node_voltages[self.terminals.pin2];
        let current_amps = match self.mode {
            CapacitorMode::Open => 0.0,
            CapacitorMode::Initial {
                branch_variable, ..
            } => *solution_values.get(branch_variable).ok_or_else(|| {
                component_diagnostic(DiagnosticCode::InvalidParameter, &self.component_id)
            })?,
            CapacitorMode::Transient { .. } => {
                let (conductance, history_current) = self.companion()?;
                conductance * voltage_volts - history_current
            }
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
