//! The simplified frequency converter of Stage 14 (docs/contracts/stage-14-more-parts.md,
//! "Semiconductor and controlled sources"): a DC enable path and, while it is enabled, three
//! Thevenin outputs to the neutral, in the phase order of a three-phase source. There is no power
//! balance between the input and the outputs.

use crate::{DeviceState, Diagnostic, DiagnosticCode, StableId};

use super::{Device, DeviceResult, Terminals, ThreePhaseSourceDevice, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

pub(crate) struct FrequencyConverterDevice {
    component_id: StableId,
    input: Terminals,
    input_resistance_ohms: f64,
    outputs: [Terminals; 3],
    output_resistance_ohms: f64,
    enabled: bool,
    source: ThreePhaseSourceDevice,
    winding_voltages: [f64; 3],
}

impl FrequencyConverterDevice {
    pub(crate) fn new(
        component_id: StableId,
        input: Terminals,
        input_resistance_ohms: f64,
        outputs: [Terminals; 3],
        winding_voltages: [f64; 3],
        output_resistance_ohms: f64,
        enabled: bool,
    ) -> Self {
        Self {
            source: ThreePhaseSourceDevice::new(
                component_id.clone(),
                outputs,
                winding_voltages,
                output_resistance_ohms,
                true,
                None,
            ),
            component_id,
            input,
            input_resistance_ohms,
            outputs,
            output_resistance_ohms,
            enabled,
            winding_voltages,
        }
    }
}

impl Device for FrequencyConverterDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        if !system.stamp_conductance(
            variables,
            self.input.pin1,
            self.input.pin2,
            1.0 / self.input_resistance_ohms,
        ) {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        if self.enabled {
            self.source.stamp(system, variables)?;
        }
        Ok(())
    }

    fn result(
        &self,
        node_voltages: &[f64],
        _solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let across =
            |terminals: Terminals| node_voltages[terminals.pin1] - node_voltages[terminals.pin2];
        let input_voltage = across(self.input);
        let input_current = input_voltage / self.input_resistance_ohms;
        let output_voltages = self.outputs.map(across);
        // Positive current enters the output pin, so a source that delivers power has a negative
        // current. An open output carries none.
        let output_currents: [f64; 3] = std::array::from_fn(|index| {
            if self.enabled {
                (output_voltages[index] - self.winding_voltages[index])
                    / self.output_resistance_ohms
            } else {
                0.0
            }
        });
        let all = [input_voltage, input_current]
            .into_iter()
            .chain(output_voltages)
            .chain(output_currents);
        if !all.into_iter().all(f64::is_finite) {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.component_id,
            ));
        }
        let id = self.component_id.as_str();
        let mut extra_values = vec![
            (format!("{id}/inputVoltageVolts"), input_voltage),
            (format!("{id}/inputCurrentAmps"), input_current),
        ];
        for (index, name) in ["u", "v", "w"].into_iter().enumerate() {
            extra_values.push((format!("{id}/{name}VoltageVolts"), output_voltages[index]));
            extra_values.push((format!("{id}/{name}CurrentAmps"), output_currents[index]));
        }
        Ok(DeviceResult {
            component_id: self.component_id.clone(),
            voltage_volts: input_voltage,
            current_amps: input_current,
            extra_values,
            state: Some(DeviceState::FrequencyConverter {
                component_id: self.component_id.clone(),
                enabled: self.enabled,
            }),
        })
    }
}
