//! Stage 1 device behavior, separate from the serialized schematic variants.

mod resistor;
mod source;

use crate::{Diagnostic, StableId};

use super::mna::{MnaSystem, NodeVariables};

pub(crate) use resistor::ResistorDevice;
pub(crate) use source::DcVoltageSourceDevice;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Terminals {
    pub pin1: usize,
    pub pin2: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DeviceResult {
    pub component_id: StableId,
    pub voltage_volts: f64,
    pub current_amps: f64,
}

pub(crate) trait Device {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic>;
    fn result(
        &self,
        node_voltages: &[f64],
        solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic>;
}

pub(crate) fn component_diagnostic(code: crate::DiagnosticCode, id: &StableId) -> Diagnostic {
    Diagnostic::error(
        code,
        vec![crate::DiagnosticReference::Component {
            component_id: id.clone(),
        }],
    )
}
