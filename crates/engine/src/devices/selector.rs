use crate::{
    Diagnostic, DiagnosticCode, StableId,
    mna::{MnaSystem, NodeVariables},
};

use super::{Device, DeviceResult, Terminals, component_diagnostic};

pub(crate) struct SelectorDevice {
    pub(crate) id: StableId,
    pub(crate) throws: [Terminals; 2],
    pub(crate) selected: Option<usize>,
    pub(crate) resistance: f64,
}

impl Device for SelectorDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        if let Some(index) = self.selected {
            let pins = self.throws[index];
            if !system.stamp_conductance(variables, pins.pin1, pins.pin2, 1.0 / self.resistance) {
                return Err(component_diagnostic(
                    DiagnosticCode::InvalidParameter,
                    &self.id,
                ));
            }
        }
        Ok(())
    }

    fn result(&self, voltages: &[f64], _: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let branches = self
            .throws
            .map(|pins| voltages[pins.pin1] - voltages[pins.pin2]);
        let currents = std::array::from_fn::<_, 2, _>(|index| {
            if self.selected == Some(index) {
                branches[index] / self.resistance
            } else {
                0.0
            }
        });
        if !branches.into_iter().chain(currents).all(f64::is_finite) {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.id,
            ));
        }
        let id = self.id.as_str();
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: branches[0],
            current_amps: currents[0],
            extra_values: vec![
                (format!("{id}/throw1VoltageVolts"), branches[0]),
                (format!("{id}/throw1CurrentAmps"), currents[0]),
                (format!("{id}/throw2VoltageVolts"), branches[1]),
                (format!("{id}/throw2CurrentAmps"), currents[1]),
            ],
            state: None,
        })
    }
}
