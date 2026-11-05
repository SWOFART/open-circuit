//! Generic transport Ebers–Moll, Stage 7 §§3.3–3.4. No region switches/state,
//! Early effect, charge storage, breakdown or parasitic resistances. Fixed 27 °C.
//! Pins [collector,base,emitter], all reported currents enter the named terminal.
//! p=+1 NPN / -1 PNP; u=p*(Vb-Ve), w=p*(Vb-Vc). Normalized terminal currents:
//! Ic=If-Ir-Ir/br-gmin*w; Ib=If/bf+Ir/br+gmin*(u+w); Ie=-(Ic+Ib).
//! Actual currents are p times these; both chain-rule polarity factors cancel
//! in dI/dV. The generic solver owns limiting/range validation and state commits.
use super::{Device, DeviceResult, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};
use crate::newton::{
    Evaluation, Junction, NonlinearDevice, THERMAL_VOLTAGE, TerminalCurrent, junction_exponential,
};
use crate::{Diagnostic, DiagnosticCode, StableId};

pub(crate) struct BjtDevice {
    id: StableId,
    nodes: [usize; 3],
    polarity: f64,
    saturation: f64,
    forward_beta: f64,
    reverse_beta: f64,
}
impl BjtDevice {
    pub(crate) fn new(
        id: StableId,
        nodes: [usize; 3],
        pnp: bool,
        saturation: f64,
        forward_beta: f64,
        reverse_beta: f64,
    ) -> Self {
        Self {
            id,
            nodes,
            polarity: if pnp { -1.0 } else { 1.0 },
            saturation,
            forward_beta,
            reverse_beta,
        }
    }
    fn law(&self, u: f64, w: f64) -> ([Vec<f64>; 3], [[f64; 2]; 3]) {
        let (ef, df) = junction_exponential(u / THERMAL_VOLTAGE);
        let (er, dr) = junction_exponential(w / THERMAL_VOLTAGE);
        let (f, r) = (self.saturation * ef, self.saturation * er);
        let (gf, gr) = (
            self.saturation / THERMAL_VOLTAGE * df,
            self.saturation / THERMAL_VOLTAGE * dr,
        );
        let c = [gf, -(1.0 + 1.0 / self.reverse_beta) * gr - 1e-12];
        let b = [
            gf / self.forward_beta + 1e-12,
            gr / self.reverse_beta + 1e-12,
        ];
        let e = [-(c[0] + b[0]), -(c[1] + b[1])];
        let p = self.polarity;
        (
            [
                vec![p * f, -p * r, -p * r / self.reverse_beta, -p * 1e-12 * w],
                vec![
                    p * f / self.forward_beta,
                    p * r / self.reverse_beta,
                    p * 1e-12 * u,
                    p * 1e-12 * w,
                ],
                vec![-p * f, -p * f / self.forward_beta, p * r, -p * 1e-12 * u],
            ],
            [c, b, e],
        )
    }
}
impl NonlinearDevice for BjtDevice {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        let [c, b, e] = self.nodes;
        [e, c]
            .map(|terminal| {
                let (positive, negative) = if self.polarity > 0.0 {
                    (b, terminal)
                } else {
                    (terminal, b)
                };
                Junction {
                    positive,
                    negative,
                    thermal_voltage: THERMAL_VOLTAGE,
                    saturation_current: self.saturation,
                }
            })
            .to_vec()
    }
    fn evaluate(&self, voltages: &[f64]) -> Evaluation {
        let (terms, jacobian) = self.law(voltages[0], voltages[1]);
        let [c, b, e] = self.nodes;
        let mut derivatives = Vec::with_capacity(9);
        for (row, [du, dw]) in self.nodes.into_iter().zip(jacobian) {
            derivatives.extend([(row, b, du + dw), (row, e, -du), (row, c, -dw)]);
        }
        Evaluation {
            currents: self
                .nodes
                .into_iter()
                .zip(terms)
                .map(|(node, terms)| TerminalCurrent { node, terms })
                .collect(),
            derivatives,
        }
    }
}
impl Device for BjtDevice {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        Some(self)
    }
    fn stamp(&self, _: &mut MnaSystem, _: &NodeVariables) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn result(&self, nodes: &[f64], _: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let [c, b, e] = self.nodes.map(|node| nodes[node]);
        let (vbe, vce) = (b - e, c - e);
        let (terms, _) = self.law(self.polarity * vbe, self.polarity * (b - c));
        let ic = terms[0].iter().sum::<f64>();
        let ib = terms[1].iter().sum::<f64>();
        let ie = -(ic + ib);
        if ![vbe, vce, ic, ib, ie].iter().all(|v| v.is_finite()) {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.id,
            ));
        }
        let extra_values = [
            ("base_emitter/voltage", vbe),
            ("collector_emitter/voltage", vce),
            ("collector/current", ic),
            ("base/current", ib),
            ("emitter/current", ie),
        ]
        .map(|(key, value)| (format!("{}/{key}", self.id), value))
        .to_vec();
        // Legacy internal fields are never exposed: all five public slots use
        // their explicit named extra_values above, just as multi-port relays do.
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: vbe,
            current_amps: ic,
            extra_values,
            state: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_jacobian_matches_terminal_differences_for_both_polarities_and_limits() {
        for pnp in [false, true] {
            for (is, bf, br) in [
                (1e-18, 1.0, 100.0),
                (1e-14, 100.0, 1.0),
                (1e-9, 1000.0, 0.01),
            ] {
                let d = BjtDevice::new(
                    StableId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
                    [0, 1, 2],
                    pnp,
                    is,
                    bf,
                    br,
                );
                for (u, w) in [
                    (-10.0, -10.0),
                    (0.0, 0.0),
                    (0.65, -9.0),
                    (0.75, 0.7),
                    (-9.0, 0.65),
                    (1.9, 1.9),
                ] {
                    let potentials = [d.polarity * (u - w), d.polarity * u, 0.0];
                    let e = d.evaluate(&[u, w]);
                    let values = e
                        .currents
                        .iter()
                        .map(|c| c.terms.iter().sum::<f64>())
                        .collect::<Vec<_>>();
                    let scale = values.iter().map(|v| v.abs()).sum::<f64>();
                    assert!(values.iter().sum::<f64>().abs() <= 1e-14 * scale + 1e-27);
                    let power = potentials
                        .iter()
                        .zip(&values)
                        .map(|(v, i)| v * i)
                        .sum::<f64>();
                    assert!(power >= -1e-14 * scale);
                    for column in 0..3 {
                        let h = 1e-7;
                        let mut plus = potentials;
                        plus[column] += h;
                        let mut minus = potentials;
                        minus[column] -= h;
                        let eval = |v: [f64; 3]| {
                            d.evaluate(&[d.polarity * (v[1] - v[2]), d.polarity * (v[1] - v[0])])
                        };
                        let ep = eval(plus);
                        let em = eval(minus);
                        for row in 0..3 {
                            // Difference corresponding physical terms before summing:
                            // a tiny output conductance must not be lost subtracting
                            // two large, unchanged transport currents.
                            let numerical = ep.currents[row]
                                .terms
                                .iter()
                                .zip(&em.currents[row].terms)
                                .map(|(a, b)| (a - b) / (2.0 * h))
                                .sum::<f64>();
                            let analytic = e
                                .derivatives
                                .iter()
                                .filter(|(r, c, _)| *r == row && *c == column)
                                .map(|(_, _, g)| g)
                                .sum::<f64>();
                            assert!(
                                (analytic - numerical).abs()
                                    <= 1e-5 * analytic.abs().max(numerical.abs()) + 1e-15,
                                "pnp={pnp} ({u},{w}) ({row},{column}): {analytic} vs {numerical}"
                            );
                        }
                    }
                }
            }
        }
    }
}
