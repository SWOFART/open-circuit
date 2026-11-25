use crate::{DeviceState, Diagnostic, DiagnosticCode, RelayContactState, StableId};

use super::relay::{RelayCoilMode, coil_companion};
use super::{Device, DeviceResult, LinkedContacts, LinkedPole, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};
use crate::{pin_groups::CONTACTOR_COIL, slots::group_slots};

#[derive(Clone, Copy)]
pub(crate) enum ContactorPoles {
    /// A coil without contacts: a solenoid or a valve.
    None,
    Auxiliaries([LinkedPole; 2]),
    PowerAndAuxiliaries([LinkedPole; 5]),
}

impl ContactorPoles {
    pub(crate) fn as_slice(&self) -> &[LinkedPole] {
        match self {
            Self::None => &[],
            Self::Auxiliaries(poles) => poles,
            Self::PowerAndAuxiliaries(poles) => poles,
        }
    }
}

/// A relay coil with two galvanically isolated auxiliary contacts: a normally open one that
/// conducts when the contactor is picked and a normally closed one that conducts when it is
/// released (docs/contracts/stage-6-control-circuits.md section 3.2).
pub(crate) struct ContactorDevice {
    component_id: StableId,
    coil: Terminals,
    poles: ContactorPoles,
    coil_resistance_ohms: f64,
    coil_inductance_henries: f64,
    closed_contact_resistance_ohms: f64,
    contact_state: RelayContactState,
    last_transition_time_seconds: Option<f64>,
    mode: RelayCoilMode,
    detector: Option<f64>,
}

impl ContactorDevice {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        component_id: StableId,
        coil: Terminals,
        poles: ContactorPoles,
        coil_resistance_ohms: f64,
        coil_inductance_henries: f64,
        closed_contact_resistance_ohms: f64,
        contact_state: RelayContactState,
        last_transition_time_seconds: Option<f64>,
        mode: RelayCoilMode,
        detector: Option<f64>,
    ) -> Self {
        Self {
            component_id,
            coil,
            poles,
            coil_resistance_ohms,
            coil_inductance_henries,
            closed_contact_resistance_ohms,
            contact_state,
            last_transition_time_seconds,
            mode,
            detector,
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

    fn contacts(&self) -> LinkedContacts<'_> {
        LinkedContacts {
            poles: self.poles.as_slice(),
            active: self.contact_state == RelayContactState::Picked,
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
                if !system.stamp_norton(
                    variables,
                    self.coil.pin1,
                    self.coil.pin2,
                    conductance,
                    -history_current,
                ) {
                    return Err(invalid());
                }
            }
        }
        self.contacts().stamp(
            &self.component_id,
            self.closed_contact_resistance_ohms,
            system,
            variables,
        )
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
        if !coil_voltage.is_finite() || !coil_current.is_finite() {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.component_id,
            ));
        }
        let id = self.component_id.as_str();
        let [voltage_slot, current_slot] = group_slots(CONTACTOR_COIL);
        let mut extra_values = vec![
            (voltage_slot.key(id), coil_voltage),
            (current_slot.key(id), coil_current),
        ];
        if matches!(self.poles, ContactorPoles::PowerAndAuxiliaries(_)) {
            let squared = self.detector.unwrap_or(coil_current * coil_current);
            if !squared.is_finite() {
                return Err(component_diagnostic(
                    DiagnosticCode::NonFiniteState,
                    &self.component_id,
                ));
            }
            extra_values.push((format!("{id}/coil/current_squared"), squared));
        }
        self.contacts().append_results(
            &self.component_id,
            self.closed_contact_resistance_ohms,
            node_voltages,
            &mut extra_values,
        )?;
        Ok(DeviceResult {
            component_id: self.component_id.clone(),
            voltage_volts: coil_voltage,
            current_amps: coil_current,
            extra_values,
            state: Some(DeviceState::Contactor {
                component_id: self.component_id.clone(),
                state: self.contact_state,
                last_transition_time_seconds: self.last_transition_time_seconds,
            }),
        })
    }
}
