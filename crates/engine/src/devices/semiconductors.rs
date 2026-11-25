//! The Stage 14 semiconductor and controlled-source devices (docs/contracts/stage-14-more-parts.md,
//! "Semiconductor and controlled sources"): the LED, the zener diode, the N-channel MOSFET and the
//! operational amplifier. They are memoryless: each is a law of node voltages that the Stage 7
//! Newton iteration linearizes, and the reported state is read from the settled solution.
//!
//! The MOSFET and the amplifier are not exponential, so they give the iteration voltage ports
//! (`Junction::port`) in place of junctions. A tiny conductance of 1 pS keeps the nodes of an open
//! gate and of the amplifier's inputs defined, the same floor the diode and the BJT use.

use super::{Device, DeviceResult, DiodeDevice, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};
use crate::newton::{
    Evaluation, Junction, NonlinearDevice, THERMAL_VOLTAGE, TerminalCurrent, junction_exponential,
};
use crate::{DeviceState, Diagnostic, DiagnosticCode, OperationalAmplifierState, StableId};

/// The conductance that keeps an otherwise open node defined: 1 pS.
const GMIN: f64 = 1e-12;

fn finite(id: &StableId, values: &[f64]) -> Result<(), Diagnostic> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(component_diagnostic(DiagnosticCode::NonFiniteState, id))
    }
}

// ---- the LED -------------------------------------------------------------------------------

/// The Stage 7 diode with a lit state: forward current at or above `litCurrentAmps`.
pub(crate) struct LedDevice {
    diode: DiodeDevice,
    lit_current_amps: f64,
}

impl LedDevice {
    pub(crate) fn new(
        id: StableId,
        terminals: Terminals,
        saturation_current: f64,
        ideality_factor: f64,
        lit_current_amps: f64,
    ) -> Self {
        Self {
            diode: DiodeDevice::new(id, terminals, saturation_current, ideality_factor),
            lit_current_amps,
        }
    }
}

impl NonlinearDevice for LedDevice {
    fn id(&self) -> &StableId {
        NonlinearDevice::id(&self.diode)
    }
    fn junctions(&self) -> Vec<Junction> {
        self.diode.junctions()
    }
    fn evaluate(&self, voltages: &[f64]) -> Evaluation {
        self.diode.evaluate(voltages)
    }
}

impl Device for LedDevice {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        Some(self)
    }
    fn stamp(&self, _: &mut MnaSystem, _: &NodeVariables) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn result(&self, nodes: &[f64], solution: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let mut result = self.diode.result(nodes, solution)?;
        result.state = Some(DeviceState::Led {
            component_id: result.component_id.clone(),
            lit: result.current_amps >= self.lit_current_amps,
        });
        Ok(result)
    }
}

// ---- the zener diode -----------------------------------------------------------------------

/// A Shockley diode with a reverse breakdown branch `min(0, (V + Vz) / Rz)`.
pub(crate) struct ZenerDevice {
    id: StableId,
    terminals: Terminals,
    saturation_current: f64,
    thermal_voltage: f64,
    breakdown_volts: f64,
    breakdown_resistance_ohms: f64,
}

impl ZenerDevice {
    pub(crate) fn new(
        id: StableId,
        terminals: Terminals,
        saturation_current: f64,
        ideality_factor: f64,
        breakdown_volts: f64,
        breakdown_resistance_ohms: f64,
    ) -> Self {
        Self {
            id,
            terminals,
            saturation_current,
            thermal_voltage: ideality_factor * THERMAL_VOLTAGE,
            breakdown_volts,
            breakdown_resistance_ohms,
        }
    }

    /// The forward current, the leakage, the breakdown current and the total slope.
    fn terms(&self, voltage: f64) -> (f64, f64, f64, f64) {
        let (exponential, derivative) = junction_exponential(voltage / self.thermal_voltage);
        let beyond = voltage + self.breakdown_volts;
        let (breakdown, breakdown_slope) = if beyond < 0.0 {
            (
                beyond / self.breakdown_resistance_ohms,
                1.0 / self.breakdown_resistance_ohms,
            )
        } else {
            (0.0, 0.0)
        };
        (
            self.saturation_current * exponential,
            1e-12 * voltage,
            breakdown,
            self.saturation_current / self.thermal_voltage * derivative + 1e-12 + breakdown_slope,
        )
    }
}

impl NonlinearDevice for ZenerDevice {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        vec![Junction {
            positive: self.terminals.pin1,
            negative: self.terminals.pin2,
            thermal_voltage: self.thermal_voltage,
            saturation_current: self.saturation_current,
            exponential: true,
        }]
    }
    fn evaluate(&self, voltages: &[f64]) -> Evaluation {
        let (forward, leakage, breakdown, slope) = self.terms(voltages[0]);
        let (a, b) = (self.terminals.pin1, self.terminals.pin2);
        Evaluation {
            currents: vec![
                TerminalCurrent {
                    node: a,
                    terms: vec![forward, leakage, breakdown],
                },
                TerminalCurrent {
                    node: b,
                    terms: vec![-forward, -leakage, -breakdown],
                },
            ],
            derivatives: vec![(a, a, slope), (a, b, -slope), (b, a, -slope), (b, b, slope)],
        }
    }
}

impl Device for ZenerDevice {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        Some(self)
    }
    fn stamp(&self, _: &mut MnaSystem, _: &NodeVariables) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn result(&self, nodes: &[f64], _: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let voltage = nodes[self.terminals.pin1] - nodes[self.terminals.pin2];
        let (forward, leakage, breakdown, _) = self.terms(voltage);
        let current = forward + leakage + breakdown;
        finite(&self.id, &[voltage, current])?;
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: voltage,
            current_amps: current,
            extra_values: Vec::new(),
            state: Some(DeviceState::ZenerDiode {
                component_id: self.id.clone(),
                in_breakdown: voltage < -self.breakdown_volts,
            }),
        })
    }
}

// ---- the N-channel MOSFET ------------------------------------------------------------------

/// The square-law channel of the contract, for `vds >= 0`: the current, `dI/dvgs`, `dI/dvds`
/// and whether the channel conducts (`vgs` above the threshold).
fn channel(vgs: f64, vds: f64, threshold: f64, gain: f64) -> (f64, f64, f64, bool) {
    let overdrive = (vgs - threshold).max(0.0);
    if overdrive == 0.0 {
        return (0.0, 0.0, 0.0, false);
    }
    if vds < overdrive {
        (
            gain * (overdrive * vds - vds * vds / 2.0),
            gain * vds,
            gain * (overdrive - vds),
            true,
        )
    } else {
        (
            gain * overdrive * overdrive / 2.0,
            gain * overdrive,
            0.0,
            true,
        )
    }
}

/// The channel current into the drain for any sign of `vds`: the source and the drain swap for a
/// negative `vds`, and the current changes sign.
fn drain_law(vgs: f64, vds: f64, threshold: f64, gain: f64) -> (f64, f64, f64, bool) {
    if vds >= 0.0 {
        channel(vgs, vds, threshold, gain)
    } else {
        let (current, d_gate, d_drain, conducting) = channel(vgs - vds, -vds, threshold, gain);
        (-current, -d_gate, d_gate + d_drain, conducting)
    }
}

pub(crate) struct MosfetDevice {
    id: StableId,
    /// Gate, drain and source.
    nodes: [usize; 3],
    threshold_volts: f64,
    gain: f64,
}

impl MosfetDevice {
    pub(crate) fn new(
        id: StableId,
        nodes: [usize; 3],
        threshold_volts: f64,
        gain_amps_per_volt_squared: f64,
    ) -> Self {
        Self {
            id,
            nodes,
            threshold_volts,
            gain: gain_amps_per_volt_squared,
        }
    }
}

impl NonlinearDevice for MosfetDevice {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        let [g, d, s] = self.nodes;
        vec![Junction::port(g, s), Junction::port(d, s)]
    }
    fn evaluate(&self, voltages: &[f64]) -> Evaluation {
        let [g, d, s] = self.nodes;
        let (vgs, vds) = (voltages[0], voltages[1]);
        let (current, gm, gds, _) = drain_law(vgs, vds, self.threshold_volts, self.gain);
        Evaluation {
            currents: vec![
                TerminalCurrent {
                    node: g,
                    terms: vec![GMIN * vgs],
                },
                TerminalCurrent {
                    node: d,
                    terms: vec![current, GMIN * vds],
                },
                TerminalCurrent {
                    node: s,
                    terms: vec![-current, -GMIN * vds, -GMIN * vgs],
                },
            ],
            derivatives: vec![
                (g, g, GMIN),
                (g, s, -GMIN),
                (d, g, gm),
                (d, d, gds + GMIN),
                (d, s, -gm - gds - GMIN),
                (s, g, -gm - GMIN),
                (s, d, -gds - GMIN),
                (s, s, gm + gds + 2.0 * GMIN),
            ],
        }
    }
}

impl Device for MosfetDevice {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        Some(self)
    }
    fn stamp(&self, _: &mut MnaSystem, _: &NodeVariables) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn result(&self, nodes: &[f64], _: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let [g, d, s] = self.nodes.map(|node| nodes[node]);
        let (vgs, vds) = (g - s, d - s);
        let (current, _, _, conducting) = drain_law(vgs, vds, self.threshold_volts, self.gain);
        let drain_current = current + GMIN * vds;
        finite(&self.id, &[vgs, vds, drain_current])?;
        let id = self.id.as_str();
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: vgs,
            current_amps: drain_current,
            extra_values: vec![
                (format!("{id}/gateSourceVoltageVolts"), vgs),
                (format!("{id}/drainSourceVoltageVolts"), vds),
                (format!("{id}/drainCurrentAmps"), drain_current),
            ],
            state: Some(DeviceState::MosfetN {
                component_id: self.id.clone(),
                conducting,
            }),
        })
    }
}

// ---- the operational amplifier -------------------------------------------------------------

pub(crate) struct OpAmpDevice {
    id: StableId,
    /// Plus input, minus input, output and the output reference.
    nodes: [usize; 4],
    open_loop_gain: f64,
    limit_volts: f64,
    output_resistance_ohms: f64,
}

impl OpAmpDevice {
    pub(crate) fn new(
        id: StableId,
        nodes: [usize; 4],
        open_loop_gain: f64,
        limit_volts: f64,
        output_resistance_ohms: f64,
    ) -> Self {
        Self {
            id,
            nodes,
            open_loop_gain,
            limit_volts,
            output_resistance_ohms,
        }
    }

    /// The open-circuit output, its slope against the differential input, and the state.
    fn source(&self, differential: f64) -> (f64, f64, OperationalAmplifierState) {
        let raw = self.open_loop_gain * differential;
        if raw > self.limit_volts {
            (
                self.limit_volts,
                0.0,
                OperationalAmplifierState::ClippedHigh,
            )
        } else if raw < -self.limit_volts {
            (
                -self.limit_volts,
                0.0,
                OperationalAmplifierState::ClippedLow,
            )
        } else {
            (raw, self.open_loop_gain, OperationalAmplifierState::Linear)
        }
    }
}

impl NonlinearDevice for OpAmpDevice {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        let [plus, minus, out, reference] = self.nodes;
        // The differential input, the output, and the common mode of the inputs.
        vec![
            Junction::port(plus, minus),
            Junction::port(out, reference),
            Junction::port(minus, reference),
        ]
    }
    fn evaluate(&self, voltages: &[f64]) -> Evaluation {
        let [p, m, o, r] = self.nodes;
        let (differential, output, common) = (voltages[0], voltages[1], voltages[2]);
        let (source, slope, _) = self.source(differential);
        let conductance = 1.0 / self.output_resistance_ohms;
        let current = (output - source) * conductance;
        let gain = slope * conductance;
        Evaluation {
            currents: vec![
                TerminalCurrent {
                    node: p,
                    terms: vec![GMIN * differential],
                },
                TerminalCurrent {
                    node: m,
                    terms: vec![-GMIN * differential, GMIN * common],
                },
                TerminalCurrent {
                    node: o,
                    terms: vec![current],
                },
                TerminalCurrent {
                    node: r,
                    terms: vec![-current, -GMIN * common],
                },
            ],
            derivatives: vec![
                (p, p, GMIN),
                (p, m, -GMIN),
                (m, p, -GMIN),
                (m, m, 2.0 * GMIN),
                (m, r, -GMIN),
                (o, p, -gain),
                (o, m, gain),
                (o, o, conductance),
                (o, r, -conductance),
                (r, p, gain),
                (r, m, -gain - GMIN),
                (r, o, -conductance),
                (r, r, conductance + GMIN),
            ],
        }
    }
}

impl Device for OpAmpDevice {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        Some(self)
    }
    fn stamp(&self, _: &mut MnaSystem, _: &NodeVariables) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn result(&self, nodes: &[f64], _: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let [p, m, o, r] = self.nodes.map(|node| nodes[node]);
        let (differential, output) = (p - m, o - r);
        let (source, _, state) = self.source(differential);
        let current = (output - source) / self.output_resistance_ohms;
        finite(&self.id, &[differential, output, current])?;
        let id = self.id.as_str();
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: output,
            current_amps: current,
            extra_values: vec![
                (format!("{id}/differentialVoltageVolts"), differential),
                (format!("{id}/outputVoltageVolts"), output),
                (format!("{id}/outputCurrentAmps"), current),
            ],
            state: Some(DeviceState::OperationalAmplifier {
                component_id: self.id.clone(),
                state,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> StableId {
        StableId::parse("00000000-0000-4000-8000-000000000001").unwrap()
    }

    #[test]
    fn the_channel_is_continuous_with_continuous_slopes_across_its_regions() {
        let (threshold, gain) = (2.0, 0.1);
        // Cutoff to triode at vgs = threshold, triode to saturation at vds = vgs - threshold.
        for (vgs, vds) in [(2.0, 1.0), (4.0, 2.0), (6.0, 4.0), (3.0, 0.5)] {
            let eps = 1e-7;
            let below = drain_law(vgs - eps, vds - eps, threshold, gain);
            let above = drain_law(vgs + eps, vds + eps, threshold, gain);
            assert!((below.0 - above.0).abs() < 1e-6, "{vgs} {vds}");
            assert!((below.1 - above.1).abs() < 1e-5, "{vgs} {vds}");
            assert!((below.2 - above.2).abs() < 1e-5, "{vgs} {vds}");
        }
    }

    #[test]
    fn the_channel_derivatives_match_differences_in_every_region_and_both_signs_of_vds() {
        let (threshold, gain) = (2.0, 0.1);
        for vgs in [0.0, 1.5, 2.5, 4.0, 8.0] {
            for vds in [-6.0, -1.0, -0.2, 0.2, 1.0, 3.0, 10.0] {
                let h = 1e-6;
                let (_, gm, gds, _) = drain_law(vgs, vds, threshold, gain);
                let numerical_gm = (drain_law(vgs + h, vds, threshold, gain).0
                    - drain_law(vgs - h, vds, threshold, gain).0)
                    / (2.0 * h);
                let numerical_gds = (drain_law(vgs, vds + h, threshold, gain).0
                    - drain_law(vgs, vds - h, threshold, gain).0)
                    / (2.0 * h);
                assert!((gm - numerical_gm).abs() < 1e-6, "gm {vgs} {vds}");
                assert!((gds - numerical_gds).abs() < 1e-6, "gds {vgs} {vds}");
            }
        }
    }

    #[test]
    fn a_negative_vds_swaps_source_and_drain_and_negates_the_current() {
        let (current, ..) = drain_law(5.0, 1.0, 2.0, 0.1);
        // The gate is 5 V above the old source; with the drain at -1 V the gate is 6 V above it.
        let (swapped, ..) = drain_law(5.0, -1.0, 2.0, 0.1);
        let (mirror, ..) = channel(6.0, 1.0, 2.0, 0.1);
        assert_eq!(swapped, -mirror);
        assert!(current > 0.0 && swapped < 0.0);
    }

    #[test]
    fn the_amplifier_current_balances_and_clips_at_the_limits() {
        let amplifier = OpAmpDevice::new(id(), [0, 1, 2, 3], 1000.0, 12.0, 1.0);
        assert_eq!(amplifier.source(0.005).2, OperationalAmplifierState::Linear);
        assert_eq!(amplifier.source(0.013).0, 12.0);
        assert_eq!(
            amplifier.source(0.013).2,
            OperationalAmplifierState::ClippedHigh
        );
        assert_eq!(
            amplifier.source(-0.013).2,
            OperationalAmplifierState::ClippedLow
        );
        // Kirchhoff: the currents of an evaluation sum to zero over the four nodes.
        for voltages in [[0.003, 5.0, 0.1], [0.02, -3.0, 0.0], [-0.02, 12.0, -1.0]] {
            let evaluation = amplifier.evaluate(&voltages);
            let sum: f64 = evaluation
                .currents
                .iter()
                .flat_map(|current| current.terms.iter())
                .sum();
            assert!(sum.abs() < 1e-12, "{voltages:?}: {sum}");
        }
    }

    #[test]
    fn the_zener_slope_includes_the_breakdown_branch_only_beyond_the_knee() {
        let zener = ZenerDevice::new(id(), Terminals { pin1: 0, pin2: 1 }, 1e-14, 1.0, 5.1, 5.0);
        let (_, _, above, slope_above) = zener.terms(-5.0);
        let (_, _, below, slope_below) = zener.terms(-6.1);
        assert_eq!(above, 0.0);
        assert!(slope_above < 1e-9);
        assert!((below + 0.2).abs() < 1e-12);
        assert!((slope_below - 0.2).abs() < 1e-9);
    }
}
