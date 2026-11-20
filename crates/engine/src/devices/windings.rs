use crate::pin_groups::PinGroup;
use crate::{
    Diagnostic, DiagnosticCode, StableId,
    mna::{MnaSystem, NodeVariables},
    slots::group_slots,
};

use super::{Device, DeviceResult, IntegrationMethod, Terminals, component_diagnostic};

#[derive(Clone, Copy)]
pub(crate) struct Winding {
    pub terminals: Terminals,
    pub resistance: f64,
    pub inductance: f64,
}

#[derive(Clone, Copy)]
pub(crate) enum WindingNetwork {
    Transformer {
        windings: [Winding; 6],
        mutual_inductance: f64,
    },
    Load {
        windings: [Winding; 3],
        delta: bool,
    },
}

impl WindingNetwork {
    fn groups(self) -> &'static [PinGroup] {
        use crate::pin_groups::{DELTA_LOAD_WINDINGS, STAR_LOAD_WINDINGS, TRANSFORMER_WINDINGS};
        match self {
            Self::Transformer { .. } => &TRANSFORMER_WINDINGS,
            Self::Load { delta: true, .. } => &DELTA_LOAD_WINDINGS,
            Self::Load { delta: false, .. } => &STAR_LOAD_WINDINGS,
        }
    }
    pub(crate) fn windings(&self) -> &[Winding] {
        match self {
            Self::Transformer { windings, .. } => windings,
            Self::Load { windings, .. } => windings,
        }
    }

    pub(crate) fn inductance(self, row: usize, column: usize) -> f64 {
        if row == column {
            self.windings()[row].inductance
        } else if let Self::Transformer {
            mutual_inductance, ..
        } = self
            && row.abs_diff(column) == 3
        {
            mutual_inductance
        } else {
            0.0
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct WindingHistory {
    pub currents: [f64; 6],
    pub voltages: [f64; 6],
}

#[derive(Clone, Copy)]
pub(crate) enum WindingMode {
    OperatingPoint,
    Initial(WindingHistory),
    Transient {
        history: WindingHistory,
        step_seconds: f64,
        integration: IntegrationMethod,
        variables: [usize; 6],
    },
}

pub(crate) struct WindingDevice {
    pub id: StableId,
    pub network: WindingNetwork,
    pub mode: WindingMode,
}

impl WindingDevice {
    pub(crate) fn history(&self, voltages: &[f64], solution: &[f64]) -> WindingHistory {
        let mut history = WindingHistory::default();
        for (index, winding) in self.network.windings().iter().enumerate() {
            let voltage = voltages[winding.terminals.pin1] - voltages[winding.terminals.pin2];
            history.voltages[index] = voltage;
            history.currents[index] = match self.mode {
                WindingMode::Transient { variables, .. } if winding.inductance > 0.0 => {
                    solution[variables[index]]
                }
                WindingMode::Initial(history) if winding.inductance > 0.0 => {
                    history.currents[index]
                }
                _ => voltage / winding.resistance,
            };
        }
        history
    }
}

impl Device for WindingDevice {
    fn stamp(&self, system: &mut MnaSystem, nodes: &NodeVariables) -> Result<(), Diagnostic> {
        let invalid = || component_diagnostic(DiagnosticCode::NonFiniteState, &self.id);
        let windings = self.network.windings();
        for (index, winding) in windings.iter().enumerate() {
            let Terminals { pin1, pin2 } = winding.terminals;
            let stamped = match self.mode {
                WindingMode::Initial(history) if winding.inductance > 0.0 => {
                    system.stamp_current_injection(nodes, pin1, pin2, -history.currents[index])
                }
                WindingMode::Transient {
                    history,
                    step_seconds,
                    integration,
                    variables,
                } if winding.inductance > 0.0 => {
                    let variable = variables[index];
                    if !system.stamp_branch_current(nodes, pin1, pin2, variable) {
                        return Err(invalid());
                    }
                    let h = match integration {
                        IntegrationMethod::Trapezoidal => step_seconds / 2.0,
                        IntegrationMethod::BackwardEuler => step_seconds,
                    };
                    let mut rhs = (step_seconds - h)
                        * (history.voltages[index] - winding.resistance * history.currents[index]);
                    let mut terms = Vec::with_capacity(5);
                    for (column, variable) in
                        variables.iter().take(windings.len()).copied().enumerate()
                    {
                        let inductance = self.network.inductance(index, column);
                        if inductance != 0.0 {
                            terms.push((variable, inductance));
                            rhs += inductance * history.currents[column];
                        }
                    }
                    terms.push((variable, h * winding.resistance));
                    if let Some(column) = nodes.voltage_variable(pin1) {
                        terms.push((column, -h));
                    }
                    if let Some(column) = nodes.voltage_variable(pin2) {
                        terms.push((column, h));
                    }
                    system.stamp_linear_equation(variable, &terms, rhs)
                }
                _ => system.stamp_conductance(nodes, pin1, pin2, winding.resistance.recip()),
            };
            if !stamped {
                return Err(invalid());
            }
        }
        Ok(())
    }

    fn result(&self, node_voltages: &[f64], solution: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let history = self.history(node_voltages, solution);
        let mut values = Vec::new();
        for (index, group) in self.network.groups().iter().enumerate() {
            let [v, i] = group_slots(*group);
            values.push((v.key(self.id.as_str()), history.voltages[index]));
            values.push((i.key(self.id.as_str()), history.currents[index]));
        }
        if let WindingNetwork::Load { delta, .. } = self.network {
            let [a, b, c, ..] = history.currents;
            if delta {
                for (line, current) in [("l1", a - c), ("l2", b - a), ("l3", c - b)] {
                    values.push((format!("{}/{line}/current", self.id.as_str()), current));
                }
            } else {
                values.push((format!("{}/n/current", self.id.as_str()), -(a + b + c)));
            }
        }
        if values.iter().any(|(_, value)| !value.is_finite()) {
            return Err(component_diagnostic(
                DiagnosticCode::NonFiniteState,
                &self.id,
            ));
        }
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: 0.0,
            current_amps: 0.0,
            extra_values: values,
            state: None,
        })
    }
}
