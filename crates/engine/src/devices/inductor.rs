use crate::{Diagnostic, DiagnosticCode, StableId};

use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

#[derive(Clone, Copy)]
pub(crate) enum InductorMode {
    Short {
        branch_variable: usize,
    },
    Initial {
        current_amps: f64,
    },
    Transient {
        step_seconds: f64,
        start_current_amps: f64,
        start_voltage_volts: f64,
    },
}

pub(crate) struct InductorDevice {
    component_id: StableId,
    terminals: Terminals,
    inductance_henries: f64,
    mode: InductorMode,
}

impl InductorDevice {
    pub(crate) fn new(
        component_id: StableId,
        terminals: Terminals,
        inductance_henries: f64,
        mode: InductorMode,
    ) -> Self {
        Self {
            component_id,
            terminals,
            inductance_henries,
            mode,
        }
    }

    fn companion(&self) -> Result<(f64, f64), Diagnostic> {
        let InductorMode::Transient {
            step_seconds,
            start_current_amps,
            start_voltage_volts,
        } = self.mode
        else {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        };
        let conductance = step_seconds / (2.0 * self.inductance_henries);
        let history_current = start_current_amps + conductance * start_voltage_volts;
        if !conductance.is_finite() || conductance <= 0.0 || !history_current.is_finite() {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        Ok((conductance, history_current))
    }
}

impl Device for InductorDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        match self.mode {
            InductorMode::Short { branch_variable } => {
                if !system.stamp_ideal_voltage_source(
                    variables,
                    self.terminals.pin1,
                    self.terminals.pin2,
                    branch_variable,
                    0.0,
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
            }
            InductorMode::Initial { current_amps } => {
                // Positive component current enters pin1, so the equivalent
                // current source draws current from pin1 in the MNA RHS.
                if !system.stamp_current_injection(
                    variables,
                    self.terminals.pin1,
                    self.terminals.pin2,
                    -current_amps,
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
            }
            InductorMode::Transient { .. } => {
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
                    -history_current,
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
            }
        }
        Ok(())
    }

    fn result(
        &self,
        node_voltages: &[f64],
        solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let voltage_volts = node_voltages[self.terminals.pin1] - node_voltages[self.terminals.pin2];
        let current_amps = match self.mode {
            InductorMode::Short { branch_variable } => {
                *solution_values.get(branch_variable).ok_or_else(|| {
                    component_diagnostic(DiagnosticCode::InvalidParameter, &self.component_id)
                })?
            }
            InductorMode::Initial { current_amps } => current_amps,
            InductorMode::Transient { .. } => {
                let (conductance, history_current) = self.companion()?;
                conductance * voltage_volts + history_current
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
