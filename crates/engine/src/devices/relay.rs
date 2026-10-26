use crate::{DeviceState, Diagnostic, DiagnosticCode, RelayContactState, StableId};

use super::{Device, DeviceResult, IntegrationMethod, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

#[derive(Clone, Copy)]
pub(crate) enum RelayCoilMode {
    OperatingPoint,
    Initial {
        current_amps: f64,
    },
    Transient {
        step_seconds: f64,
        start_current_amps: f64,
        start_voltage_volts: f64,
        integration_method: IntegrationMethod,
    },
}

pub(crate) struct RelayDevice {
    component_id: StableId,
    coil: Terminals,
    common_node: usize,
    normally_open_node: usize,
    normally_closed_node: usize,
    coil_resistance_ohms: f64,
    coil_inductance_henries: f64,
    closed_contact_resistance_ohms: f64,
    contact_state: RelayContactState,
    last_transition_time_seconds: Option<f64>,
    mode: RelayCoilMode,
}

impl RelayDevice {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        component_id: StableId,
        coil: Terminals,
        common_node: usize,
        normally_open_node: usize,
        normally_closed_node: usize,
        coil_resistance_ohms: f64,
        coil_inductance_henries: f64,
        closed_contact_resistance_ohms: f64,
        contact_state: RelayContactState,
        last_transition_time_seconds: Option<f64>,
        mode: RelayCoilMode,
    ) -> Self {
        Self {
            component_id,
            coil,
            common_node,
            normally_open_node,
            normally_closed_node,
            coil_resistance_ohms,
            coil_inductance_henries,
            closed_contact_resistance_ohms,
            contact_state,
            last_transition_time_seconds,
            mode,
        }
    }

    fn transient_companion(&self) -> Result<(f64, f64), Diagnostic> {
        let RelayCoilMode::Transient {
            step_seconds,
            start_current_amps,
            start_voltage_volts,
            integration_method,
        } = self.mode
        else {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        };
        let (conductance, history_current) = match integration_method {
            IntegrationMethod::Trapezoidal => {
                let inductive = 2.0 * self.coil_inductance_henries / step_seconds;
                let conductance = 1.0 / (self.coil_resistance_ohms + inductive);
                let history_current = conductance
                    * ((inductive - self.coil_resistance_ohms) * start_current_amps
                        + start_voltage_volts);
                (conductance, history_current)
            }
            IntegrationMethod::BackwardEuler => {
                let inductive = self.coil_inductance_henries / step_seconds;
                let conductance = 1.0 / (self.coil_resistance_ohms + inductive);
                (conductance, conductance * inductive * start_current_amps)
            }
        };
        if !conductance.is_finite() || conductance <= 0.0 || !history_current.is_finite() {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        Ok((conductance, history_current))
    }
}

impl Device for RelayDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        match self.mode {
            RelayCoilMode::OperatingPoint => {
                if !system.stamp_conductance(
                    variables,
                    self.coil.pin1,
                    self.coil.pin2,
                    1.0 / self.coil_resistance_ohms,
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
            }
            RelayCoilMode::Initial { current_amps } => {
                if !system.stamp_current_injection(
                    variables,
                    self.coil.pin1,
                    self.coil.pin2,
                    -current_amps,
                ) {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
            }
            RelayCoilMode::Transient { .. } => {
                let (conductance, history_current) = self.transient_companion()?;
                if !system.stamp_conductance(variables, self.coil.pin1, self.coil.pin2, conductance)
                    || !system.stamp_current_injection(
                        variables,
                        self.coil.pin1,
                        self.coil.pin2,
                        -history_current,
                    )
                {
                    return Err(component_diagnostic(
                        DiagnosticCode::InvalidParameter,
                        &self.component_id,
                    ));
                }
            }
        }

        let active_node = match self.contact_state {
            RelayContactState::Released => self.normally_closed_node,
            RelayContactState::Picked => self.normally_open_node,
        };
        if !system.stamp_conductance(
            variables,
            self.common_node,
            active_node,
            1.0 / self.closed_contact_resistance_ohms,
        ) {
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
        _solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let coil_voltage = node_voltages[self.coil.pin1] - node_voltages[self.coil.pin2];
        let coil_current = match self.mode {
            RelayCoilMode::OperatingPoint => coil_voltage / self.coil_resistance_ohms,
            RelayCoilMode::Initial { current_amps } => current_amps,
            RelayCoilMode::Transient { .. } => {
                let (conductance, history_current) = self.transient_companion()?;
                conductance * coil_voltage + history_current
            }
        };
        let no_voltage = node_voltages[self.common_node] - node_voltages[self.normally_open_node];
        let nc_voltage = node_voltages[self.common_node] - node_voltages[self.normally_closed_node];
        let no_current = if self.contact_state == RelayContactState::Picked {
            no_voltage / self.closed_contact_resistance_ohms
        } else {
            0.0
        };
        let nc_current = if self.contact_state == RelayContactState::Released {
            nc_voltage / self.closed_contact_resistance_ohms
        } else {
            0.0
        };
        if [
            coil_voltage,
            coil_current,
            no_voltage,
            no_current,
            nc_voltage,
            nc_current,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        let id = self.component_id.as_str();
        Ok(DeviceResult {
            component_id: self.component_id.clone(),
            voltage_volts: coil_voltage,
            current_amps: coil_current,
            extra_values: vec![
                (format!("{id}/coil/voltage"), coil_voltage),
                (format!("{id}/coil/current"), coil_current),
                (format!("{id}/normally_open/voltage"), no_voltage),
                (format!("{id}/normally_open/current"), no_current),
                (format!("{id}/normally_closed/voltage"), nc_voltage),
                (format!("{id}/normally_closed/current"), nc_current),
            ],
            state: Some(DeviceState::Relay {
                component_id: self.component_id.clone(),
                state: self.contact_state,
                last_transition_time_seconds: self.last_transition_time_seconds,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backward_euler_coil_companion_damps_the_first_step_after_a_contact_event() {
        let device = RelayDevice::new(
            StableId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
            Terminals { pin1: 0, pin2: 1 },
            2,
            3,
            4,
            100.0,
            0.01,
            0.1,
            RelayContactState::Picked,
            Some(0.0),
            RelayCoilMode::Transient {
                step_seconds: 0.001,
                start_current_amps: 0.01,
                start_voltage_volts: 100.0,
                integration_method: IntegrationMethod::BackwardEuler,
            },
        );
        let (conductance, history_current) = device.transient_companion().unwrap();
        assert!((conductance - 1.0 / 110.0).abs() < 1e-15);
        assert!((history_current - 0.000_909_090_909_090_909_1).abs() < 1e-15);
    }
}
