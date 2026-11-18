use crate::{DiagnosticCode, StableId};

use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

/// A wattmeter has two galvanically independent passive branches: its voltage circuit is
/// connected across V1/V2 and its current circuit is a burden resistor between I1/I2.
pub(crate) struct WattmeterDevice {
    component_id: StableId,
    voltage_terminals: Terminals,
    current_terminals: Terminals,
    input_resistance_ohms: f64,
    burden_resistance_ohms: f64,
}

impl WattmeterDevice {
    pub(crate) fn new(
        component_id: StableId,
        voltage_terminals: Terminals,
        current_terminals: Terminals,
        input_resistance_ohms: f64,
        burden_resistance_ohms: f64,
    ) -> Self {
        Self {
            component_id,
            voltage_terminals,
            current_terminals,
            input_resistance_ohms,
            burden_resistance_ohms,
        }
    }
}

impl Device for WattmeterDevice {
    fn stamp(
        &self,
        system: &mut MnaSystem,
        variables: &NodeVariables,
    ) -> Result<(), crate::Diagnostic> {
        let voltage_conductance = 1.0 / self.input_resistance_ohms;
        let current_conductance = 1.0 / self.burden_resistance_ohms;
        if !voltage_conductance.is_finite()
            || !current_conductance.is_finite()
            || voltage_conductance <= 0.0
            || current_conductance <= 0.0
            || !system.stamp_conductance(
                variables,
                self.voltage_terminals.pin1,
                self.voltage_terminals.pin2,
                voltage_conductance,
            )
            || !system.stamp_conductance(
                variables,
                self.current_terminals.pin1,
                self.current_terminals.pin2,
                current_conductance,
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
        _solution_values: &[f64],
    ) -> Result<DeviceResult, crate::Diagnostic> {
        let voltage_volts =
            node_voltages[self.voltage_terminals.pin1] - node_voltages[self.voltage_terminals.pin2];
        let voltage_circuit_current_amps = voltage_volts / self.input_resistance_ohms;
        let current_volts =
            node_voltages[self.current_terminals.pin1] - node_voltages[self.current_terminals.pin2];
        let current_amps = current_volts / self.burden_resistance_ohms;
        if !voltage_volts.is_finite()
            || !voltage_circuit_current_amps.is_finite()
            || !current_volts.is_finite()
            || !current_amps.is_finite()
        {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        Ok(DeviceResult {
            component_id: self.component_id.clone(),
            voltage_volts,
            current_amps,
            extra_values: vec![
                (format!("{}/voltage", self.component_id), voltage_volts),
                (format!("{}/current", self.component_id), current_amps),
            ],
            state: None,
        })
    }
}
