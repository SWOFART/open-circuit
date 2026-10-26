use crate::{Diagnostic, DiagnosticCode, StableId};

use super::{Device, DeviceResult, IntegrationMethod, Terminals, component_diagnostic};
use crate::mna::{MnaSystem, NodeVariables};

#[derive(Clone, Copy)]
pub(crate) enum PmdcMotorMode {
    OperatingPoint {
        armature_current_variable: usize,
        angular_velocity_radians_per_second: f64,
    },
    Initial {
        armature_current_amps: f64,
        angular_velocity_radians_per_second: f64,
    },
    Transient {
        step_seconds: f64,
        start_armature_current_amps: f64,
        start_angular_velocity_radians_per_second: f64,
        start_voltage_volts: f64,
        integration_method: IntegrationMethod,
        armature_current_variable: usize,
        angular_velocity_variable: usize,
    },
}

pub(crate) struct PmdcMotorDevice {
    component_id: StableId,
    terminals: Terminals,
    armature_resistance_ohms: f64,
    armature_inductance_henries: f64,
    motor_constant_newton_meters_per_amp: f64,
    rotor_inertia_kilogram_meters_squared: f64,
    viscous_friction_newton_meter_seconds_per_radian: f64,
    load_torque_newton_meters: f64,
    mode: PmdcMotorMode,
}

impl PmdcMotorDevice {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        component_id: StableId,
        terminals: Terminals,
        armature_resistance_ohms: f64,
        armature_inductance_henries: f64,
        motor_constant_newton_meters_per_amp: f64,
        rotor_inertia_kilogram_meters_squared: f64,
        viscous_friction_newton_meter_seconds_per_radian: f64,
        load_torque_newton_meters: f64,
        mode: PmdcMotorMode,
    ) -> Self {
        Self {
            component_id,
            terminals,
            armature_resistance_ohms,
            armature_inductance_henries,
            motor_constant_newton_meters_per_amp,
            rotor_inertia_kilogram_meters_squared,
            viscous_friction_newton_meter_seconds_per_radian,
            load_torque_newton_meters,
            mode,
        }
    }

    fn stamp_armature_equation(
        &self,
        system: &mut MnaSystem,
        variables: &NodeVariables,
        current_variable: usize,
        speed_variable: Option<usize>,
        current_coefficient: f64,
        right_hand_side: f64,
    ) -> bool {
        if !system.stamp_branch_current(
            variables,
            self.terminals.pin1,
            self.terminals.pin2,
            current_variable,
        ) {
            return false;
        }
        let mut terms = vec![(current_variable, current_coefficient)];
        if let Some(pin1) = variables.voltage_variable(self.terminals.pin1) {
            terms.push((pin1, -1.0));
        }
        if let Some(pin2) = variables.voltage_variable(self.terminals.pin2) {
            terms.push((pin2, 1.0));
        }
        if let Some(speed_variable) = speed_variable {
            terms.push((speed_variable, self.motor_constant_newton_meters_per_amp));
        }
        system.stamp_linear_equation(current_variable, &terms, right_hand_side)
    }

    fn transient_coefficients(
        &self,
        step_seconds: f64,
        integration_method: IntegrationMethod,
    ) -> Result<(f64, f64), Diagnostic> {
        let inductive = match integration_method {
            IntegrationMethod::Trapezoidal => 2.0 * self.armature_inductance_henries / step_seconds,
            IntegrationMethod::BackwardEuler => self.armature_inductance_henries / step_seconds,
        };
        let current_coefficient = self.armature_resistance_ohms + inductive;
        if !inductive.is_finite() || !current_coefficient.is_finite() || current_coefficient <= 0.0
        {
            return Err(component_diagnostic(
                DiagnosticCode::InvalidParameter,
                &self.component_id,
            ));
        }
        Ok((inductive, current_coefficient))
    }
}

impl Device for PmdcMotorDevice {
    fn stamp(&self, system: &mut MnaSystem, variables: &NodeVariables) -> Result<(), Diagnostic> {
        let valid = match self.mode {
            PmdcMotorMode::OperatingPoint {
                armature_current_variable,
                angular_velocity_radians_per_second,
            } => self.stamp_armature_equation(
                system,
                variables,
                armature_current_variable,
                None,
                self.armature_resistance_ohms,
                -self.motor_constant_newton_meters_per_amp * angular_velocity_radians_per_second,
            ),
            PmdcMotorMode::Initial {
                armature_current_amps,
                ..
            } => system.stamp_current_injection(
                variables,
                self.terminals.pin1,
                self.terminals.pin2,
                -armature_current_amps,
            ),
            PmdcMotorMode::Transient {
                step_seconds,
                start_armature_current_amps,
                start_angular_velocity_radians_per_second,
                start_voltage_volts,
                integration_method,
                armature_current_variable,
                angular_velocity_variable,
            } => {
                let (inductive, current_coefficient) =
                    self.transient_coefficients(step_seconds, integration_method)?;
                let armature_rhs = match integration_method {
                    IntegrationMethod::Trapezoidal => {
                        start_voltage_volts
                            + (inductive - self.armature_resistance_ohms)
                                * start_armature_current_amps
                            - self.motor_constant_newton_meters_per_amp
                                * start_angular_velocity_radians_per_second
                    }
                    IntegrationMethod::BackwardEuler => inductive * start_armature_current_amps,
                };
                let mechanical_inertia = match integration_method {
                    IntegrationMethod::Trapezoidal => {
                        2.0 * self.rotor_inertia_kilogram_meters_squared / step_seconds
                    }
                    IntegrationMethod::BackwardEuler => {
                        self.rotor_inertia_kilogram_meters_squared / step_seconds
                    }
                };
                let speed_coefficient =
                    self.viscous_friction_newton_meter_seconds_per_radian + mechanical_inertia;
                let mechanical_rhs = match integration_method {
                    IntegrationMethod::Trapezoidal => {
                        (mechanical_inertia - self.viscous_friction_newton_meter_seconds_per_radian)
                            * start_angular_velocity_radians_per_second
                            + self.motor_constant_newton_meters_per_amp
                                * start_armature_current_amps
                            + 2.0 * self.load_torque_newton_meters
                    }
                    IntegrationMethod::BackwardEuler => {
                        mechanical_inertia * start_angular_velocity_radians_per_second
                            + self.load_torque_newton_meters
                    }
                };
                if !speed_coefficient.is_finite()
                    || speed_coefficient <= 0.0
                    || !armature_rhs.is_finite()
                    || !mechanical_rhs.is_finite()
                {
                    return Err(component_diagnostic(
                        DiagnosticCode::NonFiniteState,
                        &self.component_id,
                    ));
                }
                self.stamp_armature_equation(
                    system,
                    variables,
                    armature_current_variable,
                    Some(angular_velocity_variable),
                    current_coefficient,
                    armature_rhs,
                ) && system.stamp_linear_equation(
                    angular_velocity_variable,
                    &[
                        (
                            armature_current_variable,
                            -self.motor_constant_newton_meters_per_amp,
                        ),
                        (angular_velocity_variable, speed_coefficient),
                    ],
                    mechanical_rhs,
                )
            }
        };
        if !valid {
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
        solution_values: &[f64],
    ) -> Result<DeviceResult, Diagnostic> {
        let voltage_volts = node_voltages[self.terminals.pin1] - node_voltages[self.terminals.pin2];
        let (current_amps, angular_velocity_radians_per_second) = match self.mode {
            PmdcMotorMode::OperatingPoint {
                armature_current_variable,
                angular_velocity_radians_per_second,
            } => (
                *solution_values
                    .get(armature_current_variable)
                    .ok_or_else(|| {
                        component_diagnostic(DiagnosticCode::InvalidParameter, &self.component_id)
                    })?,
                angular_velocity_radians_per_second,
            ),
            PmdcMotorMode::Initial {
                armature_current_amps,
                angular_velocity_radians_per_second,
            } => (armature_current_amps, angular_velocity_radians_per_second),
            PmdcMotorMode::Transient {
                armature_current_variable,
                angular_velocity_variable,
                ..
            } => (
                *solution_values
                    .get(armature_current_variable)
                    .ok_or_else(|| {
                        component_diagnostic(DiagnosticCode::InvalidParameter, &self.component_id)
                    })?,
                *solution_values
                    .get(angular_velocity_variable)
                    .ok_or_else(|| {
                        component_diagnostic(DiagnosticCode::InvalidParameter, &self.component_id)
                    })?,
            ),
        };
        if !voltage_volts.is_finite()
            || !current_amps.is_finite()
            || !angular_velocity_radians_per_second.is_finite()
        {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.component_id,
            ));
        }
        Ok(DeviceResult {
            component_id: self.component_id.clone(),
            voltage_volts,
            current_amps,
            extra_values: vec![(
                format!("{}/angular_velocity", self.component_id),
                angular_velocity_radians_per_second,
            )],
            state: None,
        })
    }
}
