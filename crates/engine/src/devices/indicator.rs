//! The indicator loads of Stage 14 (docs/contracts/stage-14-more-parts.md, "Indicators and
//! actuators"): a lamp, an indicator and a buzzer are fixed resistors whose reported state is read
//! from the settled endpoint. The state never feeds back into the circuit.

use crate::{DeviceState, StableId};

use super::{Device, DeviceResult, ResistorDevice, Terminals};
use crate::mna::{MnaSystem, NodeVariables};

/// The electrical power at which a lamp is lit, in watts.
pub(crate) const LAMP_LIT_POWER_WATTS: f64 = 0.25;

/// What makes the reported state true.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum IndicatorRule {
    /// A lamp is lit at `V × I >= 0.25 W`, and reports the power.
    LampPower,
    /// An indicator is lit at `|V| >= thresholdVolts`.
    IndicatorVoltage(f64),
    /// A buzzer is active at `|V| >= thresholdVolts`.
    BuzzerVoltage(f64),
}

pub(crate) struct IndicatorDevice {
    resistor: ResistorDevice,
    rule: IndicatorRule,
}

impl IndicatorDevice {
    pub(crate) fn new(
        component_id: StableId,
        terminals: Terminals,
        resistance_ohms: f64,
        rule: IndicatorRule,
    ) -> Self {
        Self {
            resistor: ResistorDevice::new(component_id, terminals, resistance_ohms),
            rule,
        }
    }
}

impl Device for IndicatorDevice {
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
        let component_id = result.component_id.clone();
        let power = result.voltage_volts * result.current_amps;
        result.state = Some(match self.rule {
            IndicatorRule::LampPower => {
                result
                    .extra_values
                    .push((format!("{}/powerWatts", component_id.as_str()), power));
                DeviceState::Lamp {
                    component_id,
                    lit: power >= LAMP_LIT_POWER_WATTS,
                }
            }
            IndicatorRule::IndicatorVoltage(threshold_volts) => DeviceState::Indicator {
                component_id,
                lit: result.voltage_volts.abs() >= threshold_volts,
            },
            IndicatorRule::BuzzerVoltage(threshold_volts) => DeviceState::Buzzer {
                component_id,
                active: result.voltage_volts.abs() >= threshold_volts,
            },
        });
        Ok(result)
    }
}
