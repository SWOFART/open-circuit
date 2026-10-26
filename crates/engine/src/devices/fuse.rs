use crate::{DeviceState, Diagnostic, DiagnosticCode, FuseState, StableId};

use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

pub(crate) struct FuseDevice {
    component_id: StableId,
    terminals: Terminals,
    intact_resistance_ohms: f64,
    accumulated_i2t_ampere_squared_seconds: f64,
    state: FuseState,
    last_transition_time_seconds: Option<f64>,
}

impl FuseDevice {
    pub(crate) fn new(
        component_id: StableId,
        terminals: Terminals,
        intact_resistance_ohms: f64,
        accumulated_i2t_ampere_squared_seconds: f64,
        state: FuseState,
        last_transition_time_seconds: Option<f64>,
    ) -> Self {
        Self {
            component_id,
            terminals,
            intact_resistance_ohms,
            accumulated_i2t_ampere_squared_seconds,
            state,
            last_transition_time_seconds,
        }
    }
}

impl Device for FuseDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        if self.state == FuseState::Tripped {
            return Ok(());
        }
        if !system.stamp_conductance(
            variables,
            self.terminals.pin1,
            self.terminals.pin2,
            1.0 / self.intact_resistance_ohms,
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
        let voltage_volts = node_voltages[self.terminals.pin1] - node_voltages[self.terminals.pin2];
        let current_amps = if self.state == FuseState::Intact {
            voltage_volts / self.intact_resistance_ohms
        } else {
            0.0
        };
        if !voltage_volts.is_finite()
            || !current_amps.is_finite()
            || !self.accumulated_i2t_ampere_squared_seconds.is_finite()
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
            extra_values: vec![(
                format!("{}/accumulated_i2t", self.component_id),
                self.accumulated_i2t_ampere_squared_seconds,
            )],
            state: Some(DeviceState::Fuse {
                component_id: self.component_id.clone(),
                state: self.state,
                last_transition_time_seconds: self.last_transition_time_seconds,
            }),
        })
    }
}
