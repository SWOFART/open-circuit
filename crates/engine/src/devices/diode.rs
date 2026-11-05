//! Generic memoryless junction (Stage 7 §3.2), fixed 27 °C, no breakdown/storage.
//! v=V(anode)-V(cathode), current enters anode. Only supported x<=80 is reported;
//! the shared exponential tangent and limiting are iteration aids, never a state.
use super::{Device, DeviceResult, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};
use crate::newton::{
    Evaluation, Junction, NonlinearDevice, THERMAL_VOLTAGE, TerminalCurrent, junction_exponential,
};
use crate::{Diagnostic, DiagnosticCode, StableId};

pub(crate) struct DiodeDevice {
    id: StableId,
    terminals: Terminals,
    saturation_current: f64,
    thermal_voltage: f64,
}
impl DiodeDevice {
    pub(crate) fn new(
        id: StableId,
        terminals: Terminals,
        saturation_current: f64,
        emission: f64,
    ) -> Self {
        Self {
            id,
            terminals,
            saturation_current,
            thermal_voltage: emission * THERMAL_VOLTAGE,
        }
    }
    fn terms(&self, voltage: f64) -> (f64, f64, f64) {
        let (e, derivative) = junction_exponential(voltage / self.thermal_voltage);
        (
            self.saturation_current * e,
            1e-12 * voltage,
            self.saturation_current / self.thermal_voltage * derivative + 1e-12,
        )
    }
}
impl NonlinearDevice for DiodeDevice {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        vec![Junction {
            positive: self.terminals.pin1,
            negative: self.terminals.pin2,
            thermal_voltage: self.thermal_voltage,
            saturation_current: self.saturation_current,
        }]
    }
    fn evaluate(&self, voltages: &[f64]) -> Evaluation {
        let (i, leakage, g) = self.terms(voltages[0]);
        let (a, b) = (self.terminals.pin1, self.terminals.pin2);
        Evaluation {
            currents: vec![
                TerminalCurrent {
                    node: a,
                    terms: vec![i, leakage],
                },
                TerminalCurrent {
                    node: b,
                    terms: vec![-i, -leakage],
                },
            ],
            derivatives: vec![(a, a, g), (a, b, -g), (b, a, -g), (b, b, g)],
        }
    }
}
impl Device for DiodeDevice {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        Some(self)
    }
    fn stamp(&self, _: &mut MnaSystem, _: &NodeVariables) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn result(&self, nodes: &[f64], _: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let v = nodes[self.terminals.pin1] - nodes[self.terminals.pin2];
        let (i, leakage, _) = self.terms(v);
        let current = i + leakage;
        if !v.is_finite() || !current.is_finite() {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.id,
            ));
        }
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: v,
            current_amps: current,
            extra_values: vec![],
            state: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytic_derivative_and_passivity_cover_parameter_limits() {
        for is in [1e-18, 1e-14, 1e-9] {
            for n in [1.0, 2.0] {
                let d = DiodeDevice::new(
                    StableId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
                    Terminals { pin1: 0, pin2: 1 },
                    is,
                    n,
                );
                for x in [-1e6, -2.0, 0.0, 0.1, 20.0, 79.0] {
                    let v = x * d.thermal_voltage;
                    let (i, l, g) = d.terms(v);
                    assert!(g.is_finite() && g > 0.0);
                    assert!(v * (i + l) >= 0.0);
                    let h = d.thermal_voltage * 1e-5;
                    let plus = d.terms(v + h);
                    let minus = d.terms(v - h);
                    let numerical = ((plus.0 + plus.1) - (minus.0 + minus.1)) / (2.0 * h);
                    assert!(
                        (g - numerical).abs() <= 1e-5 * g + 1e-20,
                        "{x}: {g} vs {numerical}"
                    );
                    let e = d.evaluate(&[v]);
                    assert_eq!(
                        e.currents
                            .iter()
                            .map(|c| c.terms.iter().sum::<f64>())
                            .sum::<f64>(),
                        0.0
                    );
                }
            }
        }
    }
}
