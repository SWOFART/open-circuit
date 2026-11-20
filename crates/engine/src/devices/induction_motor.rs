//! Flux-linkage equations for a linear squirrel-cage induction motor.

use super::{Device, DeviceResult, IntegrationMethod, Terminals, component_diagnostic};
use crate::{
    Diagnostic, DiagnosticCode, StableId,
    mna::{MnaSystem, NodeVariables},
    newton::{BilinearTerm, Evaluation, Junction, NonlinearDevice},
    pin_groups::INDUCTION_WINDINGS,
    slots::group_slots,
};

const A: f64 = 0.816_496_580_927_726;
const B: f64 = std::f64::consts::FRAC_1_SQRT_2;
const C: f64 = 0.577_350_269_189_625_8;
const CLARKE: [[f64; 3]; 3] = [[A, -A / 2.0, -A / 2.0], [0.0, B, -B], [C; 3]];

#[derive(Clone, Copy)]
pub(crate) struct MotorParameters {
    pub stator_resistance: f64,
    pub rotor_resistance: f64,
    pub stator_leakage: f64,
    pub rotor_leakage: f64,
    pub magnetizing_inductance: f64,
    pub pole_pairs: f64,
    pub inertia: f64,
    pub friction: f64,
    pub load_torque: f64,
}

/// Stator αβ0 flux, rotor αβ flux, and mechanical speed, in that order.
pub(crate) type MotorState = [f64; 6];

#[derive(Clone, Copy, Default)]
pub(crate) struct MotorHistory {
    pub state: MotorState,
    pub voltage: [f64; 3],
}

pub(super) struct MotorEndpoint {
    pub stator_current: [f64; 3],
    pub torque: f64,
    pub derivative: MotorState,
    pub jacobian: [[f64; 6]; 6],
}

impl MotorParameters {
    pub(crate) fn winding_currents(self, state: MotorState) -> [f64; 3] {
        phase_currents(self.endpoint(state, [0.0; 3]).stator_current)
    }

    fn determinant(self) -> f64 {
        self.stator_leakage * (self.rotor_leakage + self.magnetizing_inductance)
            + self.magnetizing_inductance * self.rotor_leakage
    }

    /// Infinity norm in coordinates scaled by the declared flux and speed
    /// limits. This bound assumes the winding voltages are fixed.
    pub(crate) fn fixed_voltage_rate_bound(self) -> f64 {
        let limits = [100.0, 100.0, 100.0, 100.0, 100.0, 3000.0];
        // The absolute maximum of each derivative is achieved at these
        // positive endpoints or is independent of the state.
        let jacobian = self.endpoint(limits, [0.0; 3]).jacobian;
        jacobian
            .into_iter()
            .zip(limits)
            .map(|(row, scale)| {
                row.into_iter()
                    .zip(limits)
                    .map(|(derivative, input_scale)| derivative.abs() * input_scale / scale)
                    .sum::<f64>()
            })
            .fold(0.0_f64, f64::max)
    }

    pub(crate) fn phase_current_gain(self) -> f64 {
        // Sum of the absolute current derivatives with respect to all
        // normalized flux coordinates. The shaft speed does not enter KCL.
        let mut row_sums = [0.0; 3];
        for coordinate in 0..5 {
            let mut basis = [0.0; 6];
            basis[coordinate] = 100.0;
            for (sum, current) in row_sums.iter_mut().zip(phase_currents(
                self.endpoint(basis, [0.0; 3]).stator_current,
            )) {
                *sum += current.abs();
            }
        }
        row_sums.into_iter().fold(0.0_f64, f64::max)
    }

    pub(super) fn endpoint(self, state: MotorState, voltage: [f64; 3]) -> MotorEndpoint {
        let [sa, sb, s0, ra, rb, speed] = state;
        let lm = self.magnetizing_inductance;
        let ls = self.stator_leakage + lm;
        let lr = self.rotor_leakage + lm;
        let delta = self.determinant();
        let stator_self = lr / delta;
        let rotor_self = ls / delta;
        let mutual = -lm / delta;
        let zero = self.stator_leakage.recip();
        let stator_current = [
            stator_self * sa + mutual * ra,
            stator_self * sb + mutual * rb,
            zero * s0,
        ];
        let rotor_current = [mutual * sa + rotor_self * ra, mutual * sb + rotor_self * rb];
        let torque_gain = self.pole_pairs * mutual;
        let torque = torque_gain * (sa * rb - sb * ra);
        let rotation = self.pole_pairs * speed;
        let rs = self.stator_resistance;
        let rr = self.rotor_resistance;
        let derivative = [
            voltage[0] - rs * stator_current[0],
            voltage[1] - rs * stator_current[1],
            voltage[2] - rs * stator_current[2],
            -rr * rotor_current[0] - rotation * rb,
            -rr * rotor_current[1] + rotation * ra,
            (torque - self.friction * speed - self.load_torque) / self.inertia,
        ];
        let gain = torque_gain / self.inertia;
        let jacobian = [
            [-rs * stator_self, 0.0, 0.0, -rs * mutual, 0.0, 0.0],
            [0.0, -rs * stator_self, 0.0, 0.0, -rs * mutual, 0.0],
            [0.0, 0.0, -rs * zero, 0.0, 0.0, 0.0],
            [
                -rr * mutual,
                0.0,
                0.0,
                -rr * rotor_self,
                -rotation,
                -self.pole_pairs * rb,
            ],
            [
                0.0,
                -rr * mutual,
                0.0,
                rotation,
                -rr * rotor_self,
                self.pole_pairs * ra,
            ],
            [
                gain * rb,
                -gain * ra,
                0.0,
                -gain * sb,
                gain * sa,
                -self.friction / self.inertia,
            ],
        ];
        MotorEndpoint {
            stator_current,
            torque,
            derivative,
            jacobian,
        }
    }
}

fn phase_currents(coordinates: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|phase| (0..3).map(|i| CLARKE[i][phase] * coordinates[i]).sum())
}

fn transform(phases: [f64; 3]) -> [f64; 3] {
    CLARKE.map(|row| row.into_iter().zip(phases).map(|(a, b)| a * b).sum())
}

#[derive(Clone, Copy)]
pub(crate) struct MotorInterval {
    pub variables: [usize; 6],
    pub seconds: f64,
    pub method: IntegrationMethod,
}

pub(crate) struct InductionMotorDevice {
    id: StableId,
    windings: [Terminals; 3],
    parameters: MotorParameters,
    history: MotorHistory,
    interval: Option<MotorInterval>,
    products: Vec<BilinearTerm>,
}

impl InductionMotorDevice {
    pub(crate) fn new(
        id: StableId,
        windings: [Terminals; 3],
        parameters: MotorParameters,
        history: MotorHistory,
        interval: Option<MotorInterval>,
    ) -> Self {
        let products = interval.map_or_else(Vec::new, |interval| {
            let h = right_weight(interval);
            let p = parameters.pole_pairs;
            let gain = -p * parameters.magnetizing_inductance
                / parameters.determinant()
                / parameters.inertia;
            Vec::from(
                [
                    (3, 5, 4, h * p),
                    (4, 5, 3, -h * p),
                    (5, 0, 4, -h * gain),
                    (5, 1, 3, h * gain),
                ]
                .map(|(row, left, right, coefficient)| BilinearTerm {
                    row: interval.variables[row],
                    left: interval.variables[left],
                    right: interval.variables[right],
                    coefficient,
                }),
            )
        });
        Self {
            id,
            windings,
            parameters,
            history,
            interval,
            products,
        }
    }
}

fn right_weight(interval: MotorInterval) -> f64 {
    match interval.method {
        IntegrationMethod::Trapezoidal => interval.seconds / 2.0,
        IntegrationMethod::BackwardEuler => interval.seconds,
    }
}

impl NonlinearDevice for InductionMotorDevice {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        Vec::new()
    }
    fn evaluate(&self, _: &[f64]) -> Evaluation {
        Evaluation {
            currents: Vec::new(),
            derivatives: Vec::new(),
        }
    }
    fn bilinear_terms(&self) -> &[BilinearTerm] {
        &self.products
    }
}

impl Device for InductionMotorDevice {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        self.interval.map(|_| self as &dyn NonlinearDevice)
    }

    fn stamp(&self, system: &mut MnaSystem, nodes: &NodeVariables) -> Result<(), Diagnostic> {
        let invalid = || component_diagnostic(DiagnosticCode::NonFiniteState, &self.id);
        let Some(interval) = self.interval else {
            let currents = phase_currents(
                self.parameters
                    .endpoint(self.history.state, [0.0; 3])
                    .stator_current,
            );
            for (terminals, current) in self.windings.iter().zip(currents) {
                if !system.stamp_current_injection(nodes, terminals.pin1, terminals.pin2, -current)
                {
                    return Err(invalid());
                }
            }
            return Ok(());
        };
        // The current map is linear. Its six columns couple the phase KCL
        // rows to the stator and rotor flux unknowns.
        let columns: [[f64; 3]; 6] = std::array::from_fn(|column| {
            let mut basis = [0.0; 6];
            basis[column] = 1.0;
            phase_currents(self.parameters.endpoint(basis, [0.0; 3]).stator_current)
        });
        for (phase, terminals) in self.windings.iter().enumerate() {
            if terminals.pin1 == terminals.pin2 {
                continue;
            }
            for (node, sign) in [(terminals.pin1, 1.0), (terminals.pin2, -1.0)] {
                if let Some(row) = nodes.voltage_variable(node) {
                    let terms = std::array::from_fn::<_, 6, _>(|column| {
                        (interval.variables[column], sign * columns[column][phase])
                    });
                    if !system.stamp_linear_equation(row, &terms, 0.0) {
                        return Err(invalid());
                    }
                }
            }
        }
        let h = right_weight(interval);
        let previous = self
            .parameters
            .endpoint(self.history.state, self.history.voltage);
        let linear = self.parameters.endpoint([0.0; 6], [0.0; 3]);
        for (row, variable) in interval.variables.into_iter().enumerate() {
            let mut terms = vec![(variable, 1.0)];
            terms.extend(
                interval
                    .variables
                    .into_iter()
                    .zip(linear.jacobian[row].map(|value| -h * value)),
            );
            if row < 3 {
                for (phase, terminals) in self.windings.iter().enumerate() {
                    for (node, sign) in [(terminals.pin1, -1.0), (terminals.pin2, 1.0)] {
                        if let Some(column) = nodes.voltage_variable(node) {
                            terms.push((column, sign * h * CLARKE[row][phase]));
                        }
                    }
                }
            }
            let rhs = self.history.state[row]
                + (interval.seconds - h) * previous.derivative[row]
                + h * linear.derivative[row];
            if !system.stamp_linear_equation(variable, &terms, rhs) {
                return Err(invalid());
            }
        }
        Ok(())
    }

    fn result(&self, node_voltages: &[f64], solution: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let state = self.interval.map_or(self.history.state, |interval| {
            interval.variables.map(|index| solution[index])
        });
        if state[..5].iter().any(|flux| flux.abs() > 100.0) || state[5].abs() > 3000.0 {
            return Err(component_diagnostic(
                DiagnosticCode::ModelRangeExceeded,
                &self.id,
            ));
        }
        let voltage = winding_voltages(self.windings, node_voltages);
        let endpoint = self.parameters.endpoint(state, transform(voltage));
        let currents = phase_currents(endpoint.stator_current);
        let mut extra_values = Vec::with_capacity(8);
        for (index, group) in INDUCTION_WINDINGS.into_iter().enumerate() {
            let [v, i] = group_slots(group);
            extra_values.push((v.key(self.id.as_str()), voltage[index]));
            extra_values.push((i.key(self.id.as_str()), currents[index]));
        }
        extra_values.push((format!("{}/angular_velocity", self.id.as_str()), state[5]));
        extra_values.push((
            format!("{}/electromagnetic_torque", self.id.as_str()),
            endpoint.torque,
        ));
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: 0.0,
            current_amps: 0.0,
            extra_values,
            state: None,
        })
    }
}

fn winding_voltages(windings: [Terminals; 3], nodes: &[f64]) -> [f64; 3] {
    windings.map(|terminals| nodes[terminals.pin1] - nodes[terminals.pin2])
}

pub(crate) fn motor_history(
    state: MotorState,
    windings: [Terminals; 3],
    node_voltages: &[f64],
) -> MotorHistory {
    MotorHistory {
        state,
        voltage: transform(winding_voltages(windings, node_voltages)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backward_euler_recovery_uses_the_new_winding_voltage_only() {
        use crate::newton::{UnknownTolerance, solve};
        let device_id = StableId::parse("00000000-0000-4000-8000-000000000001").unwrap();
        let parameters = MotorParameters {
            stator_resistance: 1.0,
            rotor_resistance: 1.0,
            stator_leakage: 0.005,
            rotor_leakage: 0.005,
            magnetizing_inductance: 0.1,
            pole_pairs: 2.0,
            inertia: 0.02,
            friction: 0.0,
            load_torque: 0.0,
        };
        let nodes = NodeVariables::for_island(4, &[0, 1, 2, 3], 0, &Default::default());
        let windings = [1, 2, 3].map(|pin1| Terminals { pin1, pin2: 0 });
        let dt = 0.001;
        for method in [
            IntegrationMethod::BackwardEuler,
            IntegrationMethod::Trapezoidal,
        ] {
            let mut system = MnaSystem::new(12);
            for (i, winding) in windings.iter().enumerate() {
                assert!(system.stamp_ideal_voltage_source(
                    &nodes,
                    winding.pin1,
                    winding.pin2,
                    3 + i,
                    10.0
                ));
            }
            let device = InductionMotorDevice::new(
                device_id.clone(),
                windings,
                parameters,
                MotorHistory::default(),
                Some(MotorInterval {
                    variables: [6, 7, 8, 9, 10, 11],
                    seconds: dt,
                    method,
                }),
            );
            device.stamp(&mut system, &nodes).unwrap();
            let tolerance = [UnknownTolerance {
                update: 1e-9,
                residual: 1e-9,
            }; 12];
            let (solution, _) = solve(
                &system,
                &nodes,
                &[&device],
                &tolerance,
                vec![0.0; 12],
                vec![vec![]],
            )
            .unwrap();
            let h = if method == IntegrationMethod::BackwardEuler {
                dt
            } else {
                dt / 2.0
            };
            let expected_flux = h * 3.0_f64.sqrt() * 10.0 / (1.0 + h / parameters.stator_leakage);
            assert!((solution[8] - expected_flux).abs() < 1e-12);
            assert!(
                solution[6..8]
                    .iter()
                    .chain(&solution[9..])
                    .all(|value| value.abs() < 1e-12)
            );
        }
    }

    #[test]
    fn clarke_rows_preserve_power_and_separate_the_zero_sequence() {
        for (i, row) in CLARKE.iter().enumerate() {
            for (j, other) in CLARKE.iter().enumerate() {
                let dot = row.iter().zip(other).map(|(x, y)| x * y).sum::<f64>();
                assert!((dot - f64::from(i == j)).abs() < 1e-14);
            }
        }
    }

    #[test]
    fn locked_rotor_matches_the_balanced_equivalent_circuit() {
        let p = MotorParameters {
            stator_resistance: 1.0,
            rotor_resistance: 1.0,
            stator_leakage: 0.005,
            rotor_leakage: 0.005,
            magnetizing_inductance: 0.1,
            pole_pairs: 2.0,
            inertia: 0.02,
            friction: 0.01,
            load_torque: 0.0,
        };
        let omega = std::f64::consts::TAU * 50.0;
        let x_leak = omega * 0.005;
        let x_mag = omega * 0.1;
        let branch_norm = 1.0 + x_leak * x_leak;
        // Parallel magnetizing and referred rotor branch admittances.
        let g = 1.0 / branch_norm;
        let b = -x_leak / branch_norm - 1.0 / x_mag;
        let parallel = [g / g.hypot(b).powi(2), -b / g.hypot(b).powi(2)];
        // Select one amp RMS phase current and compute the corresponding
        // air-gap voltage and branch current independently of the flux law.
        let ir_referred = [
            (parallel[0] + x_leak * parallel[1]) / branch_norm,
            (parallel[1] - x_leak * parallel[0]) / branch_norm,
        ];
        let factor = 3.0_f64.sqrt();
        let state = [
            (0.105 - 0.1 * ir_referred[0]) * factor,
            -0.1 * ir_referred[1] * factor,
            0.0,
            (0.1 - 0.105 * ir_referred[0]) * factor,
            -0.105 * ir_referred[1] * factor,
            0.0,
        ];
        let endpoint = p.endpoint(
            state,
            [
                (1.0 + parallel[0]) * factor,
                (x_leak + parallel[1]) * factor,
                0.0,
            ],
        );
        let expected_torque = 3.0 * ir_referred[0].hypot(ir_referred[1]).powi(2) * 2.0 / omega;
        assert!((endpoint.stator_current[0] / factor - 1.0).abs() < 1e-12);
        assert!((endpoint.torque - expected_torque).abs() < 1e-12);
        for (actual, expected) in endpoint.derivative[..5].iter().zip([
            -omega * state[1],
            omega * state[0],
            0.0,
            -omega * state[4],
            omega * state[3],
        ]) {
            assert!((actual - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn endpoint_jacobian_and_machine_power_balance() {
        let p = MotorParameters {
            stator_resistance: 0.9,
            rotor_resistance: 1.3,
            stator_leakage: 0.005,
            rotor_leakage: 0.006,
            magnetizing_inductance: 0.1,
            pole_pairs: 2.0,
            inertia: 0.01,
            friction: 0.02,
            load_torque: 0.1,
        };
        let state = [0.31, 0.19, -0.06, 0.29, 0.21, 90.0];
        let voltage = [14.0, 49.0, -2.0];
        let endpoint = p.endpoint(state, voltage);
        let epsilon = 1e-6;
        for column in 0..state.len() {
            let mut lower = state;
            let mut upper = state;
            lower[column] -= epsilon;
            upper[column] += epsilon;
            let lower = p.endpoint(lower, voltage).derivative;
            let upper = p.endpoint(upper, voltage).derivative;
            for (row, (lower, upper)) in lower.into_iter().zip(upper).enumerate() {
                let analytic = endpoint.jacobian[row][column];
                assert!(((upper - lower) / (2.0 * epsilon) - analytic).abs() < 1e-6);
            }
        }
        let rotor = [0, 1].map(|i| {
            (state[i] - (p.stator_leakage + p.magnetizing_inductance) * endpoint.stator_current[i])
                / p.magnetizing_inductance
        });
        let electrical_input: f64 = voltage
            .iter()
            .zip(endpoint.stator_current)
            .map(|(v, i)| v * i)
            .sum();
        let winding_losses = p.stator_resistance
            * endpoint.stator_current.iter().map(|i| i * i).sum::<f64>()
            + p.rotor_resistance * rotor.iter().map(|i| i * i).sum::<f64>();
        let stored_power: f64 = endpoint
            .stator_current
            .into_iter()
            .chain(rotor)
            .zip(endpoint.derivative)
            .map(|(i, rate)| i * rate)
            .sum();
        assert!(
            (electrical_input - winding_losses - stored_power - endpoint.torque * state[5]).abs()
                < 1e-10
        );
    }
}
