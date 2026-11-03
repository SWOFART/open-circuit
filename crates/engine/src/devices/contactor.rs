use crate::{DeviceState, Diagnostic, DiagnosticCode, RelayContactState, StableId};

use super::relay::{RelayCoilMode, coil_companion};
use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

/// A relay coil with two galvanically isolated auxiliary contacts: a normally open one that
/// conducts when the contactor is picked and a normally closed one that conducts when it is
/// released (docs/contracts/stage-6-control-circuits.md section 3.2).
pub(crate) struct ContactorDevice {
    component_id: StableId,
    coil: Terminals,
    normally_open: Terminals,
    normally_closed: Terminals,
    coil_resistance_ohms: f64,
    coil_inductance_henries: f64,
    closed_contact_resistance_ohms: f64,
    contact_state: RelayContactState,
    last_transition_time_seconds: Option<f64>,
    mode: RelayCoilMode,
}

impl ContactorDevice {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        component_id: StableId,
        coil: Terminals,
        normally_open: Terminals,
        normally_closed: Terminals,
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
            normally_open,
            normally_closed,
            coil_resistance_ohms,
            coil_inductance_henries,
            closed_contact_resistance_ohms,
            contact_state,
            last_transition_time_seconds,
            mode,
        }
    }

    fn companion(&self) -> Result<(f64, f64), Diagnostic> {
        coil_companion(
            &self.component_id,
            self.mode,
            self.coil_resistance_ohms,
            self.coil_inductance_henries,
        )
    }

    fn stamp_conducting_contact(
        &self,
        system: &mut MnaSystem,
        variables: &NodeVariables,
    ) -> Result<(), Diagnostic> {
        let conducting = match self.contact_state {
            RelayContactState::Released => self.normally_closed,
            RelayContactState::Picked => self.normally_open,
        };
        if system.stamp_conductance(
            variables,
            conducting.pin1,
            conducting.pin2,
            1.0 / self.closed_contact_resistance_ohms,
        ) {
            Ok(())
        } else {
            Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ))
        }
    }
}

impl Device for ContactorDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        let invalid = || component_diagnostic(DiagnosticCode::InvalidParameter, &self.component_id);
        match self.mode {
            RelayCoilMode::OperatingPoint => {
                if !system.stamp_conductance(
                    variables,
                    self.coil.pin1,
                    self.coil.pin2,
                    1.0 / self.coil_resistance_ohms,
                ) {
                    return Err(invalid());
                }
            }
            RelayCoilMode::Initial { current_amps } => {
                if !system.stamp_current_injection(
                    variables,
                    self.coil.pin1,
                    self.coil.pin2,
                    -current_amps,
                ) {
                    return Err(invalid());
                }
            }
            RelayCoilMode::Transient { .. } => {
                let (conductance, history_current) = self.companion()?;
                if !system.stamp_conductance(variables, self.coil.pin1, self.coil.pin2, conductance)
                    || !system.stamp_current_injection(
                        variables,
                        self.coil.pin1,
                        self.coil.pin2,
                        -history_current,
                    )
                {
                    return Err(invalid());
                }
            }
        }
        self.stamp_conducting_contact(system, variables)
    }

    fn result(
        &self,
        node_voltages: &[f64],
        _solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let across =
            |terminals: Terminals| node_voltages[terminals.pin1] - node_voltages[terminals.pin2];
        let coil_voltage = across(self.coil);
        let coil_current = match self.mode {
            RelayCoilMode::OperatingPoint => coil_voltage / self.coil_resistance_ohms,
            RelayCoilMode::Initial { current_amps } => current_amps,
            RelayCoilMode::Transient { .. } => {
                let (conductance, history_current) = self.companion()?;
                conductance * coil_voltage + history_current
            }
        };
        let no_voltage = across(self.normally_open);
        let nc_voltage = across(self.normally_closed);
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
                DiagnosticCode::NonFiniteState,
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
                (format!("{id}/no/voltage"), no_voltage),
                (format!("{id}/no/current"), no_current),
                (format!("{id}/nc/voltage"), nc_voltage),
                (format!("{id}/nc/current"), nc_current),
            ],
            state: Some(DeviceState::Contactor {
                component_id: self.component_id.clone(),
                state: self.contact_state,
                last_transition_time_seconds: self.last_transition_time_seconds,
            }),
        })
    }
}
