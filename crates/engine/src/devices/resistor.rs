use crate::{DiagnosticCode, StableId};

use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

pub(crate) struct ResistorDevice {
    component_id: StableId,
    terminals: Terminals,
    resistance_ohms: f64,
}

impl ResistorDevice {
    pub(crate) fn new(component_id: StableId, terminals: Terminals, resistance_ohms: f64) -> Self {
        Self {
            component_id,
            terminals,
            resistance_ohms,
        }
    }
}

impl Device for ResistorDevice {
    /// Stamp `G = 1/R` between the two terminal nodes, preserving the passive sign convention.
    fn stamp(
        &self,
        system: &mut MnaSystem,
        variables: &NodeVariables,
    ) -> Result<(), crate::Diagnostic> {
        let conductance = 1.0 / self.resistance_ohms;
        if !conductance.is_finite()
            || conductance <= 0.0
            || !system.stamp_conductance(
                variables,
                self.terminals.pin1,
                self.terminals.pin2,
                conductance,
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
        let voltage_volts = node_voltages[self.terminals.pin1] - node_voltages[self.terminals.pin2];
        let current_amps = voltage_volts / self.resistance_ohms;
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

/// The exposed part: a resistor to the reference of its island (docs/contracts/
/// stage-13-protection-and-safety.md section 7.1) whose result also says whether that reference is
/// the earth node, so the touch voltage measurement knows whether it can be read.
pub(crate) struct ExposedPartDevice {
    resistor: ResistorDevice,
    earth_joined: bool,
}

impl ExposedPartDevice {
    pub(crate) fn new(
        component_id: StableId,
        terminals: Terminals,
        resistance_ohms: f64,
        earth_joined: bool,
    ) -> Self {
        Self {
            resistor: ResistorDevice::new(component_id, terminals, resistance_ohms),
            earth_joined,
        }
    }
}

impl Device for ExposedPartDevice {
    fn stamp(
        &self,
        system: &mut MnaSystem,
        variables: &NodeVariables,
    ) -> Result<(), crate::Diagnostic> {
        self.resistor.stamp(system, variables)
    }

    fn result(
        &self,
        node_voltages: &[f64],
        solution_values: &[f64],
    ) -> Result<DeviceResult, crate::Diagnostic> {
        let mut result = self.resistor.result(node_voltages, solution_values)?;
        // Read by the measurement update, which has no netlist; not a layout slot.
        result.extra_values.push((
            format!("{}/earth_joined", result.component_id.as_str()),
            f64::from(self.earth_joined),
        ));
        Ok(result)
    }
}
