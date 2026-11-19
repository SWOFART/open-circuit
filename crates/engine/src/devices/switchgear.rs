use crate::{OperatorPosition, RelayContactState, SwitchPosition};

/// Held-input solution of the first-order squared-current detector.
pub(crate) fn detector_step(
    previous_squared_amps: f64,
    start_amps: f64,
    end_amps: f64,
    step_seconds: f64,
    sense_seconds: f64,
) -> Option<f64> {
    let input = 0.5 * start_amps * start_amps + 0.5 * end_amps * end_amps;
    let weight = -(-step_seconds / sense_seconds).exp_m1();
    let next = previous_squared_amps + (input - previous_squared_amps) * weight;
    next.is_finite().then_some(next.max(0.0))
}

pub(crate) fn coil_contact_state(
    state: RelayContactState,
    squared_current: f64,
    pickup: f64,
    dropout: f64,
) -> RelayContactState {
    match state {
        RelayContactState::Released if squared_current >= pickup * pickup => {
            RelayContactState::Picked
        }
        RelayContactState::Picked if squared_current <= dropout * dropout => {
            RelayContactState::Released
        }
        _ => state,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BreakerState {
    pub(crate) position: SwitchPosition,
    pub(crate) tripped: bool,
    pub(crate) last_transition_time_seconds: Option<f64>,
}

impl BreakerState {
    pub(crate) fn conducting(self) -> bool {
        self.position == SwitchPosition::Closed && !self.tripped
    }

    pub(crate) fn actuated(
        mut self,
        position: OperatorPosition,
        time_seconds: f64,
    ) -> Option<Self> {
        let was_conducting = self.conducting();
        match position {
            OperatorPosition::Open => {
                self.position = SwitchPosition::Open;
                self.tripped = false;
            }
            OperatorPosition::Closed if !self.tripped => self.position = SwitchPosition::Closed,
            _ => return None,
        }
        if was_conducting != self.conducting() {
            self.last_transition_time_seconds = Some(time_seconds);
        }
        Some(self)
    }

    pub(crate) fn trip_if_needed(
        mut self,
        currents_amps: [f64; 3],
        threshold_amps: f64,
        time_seconds: f64,
    ) -> Self {
        if self.conducting()
            && currents_amps
                .iter()
                .any(|current| current.abs() >= threshold_amps)
        {
            self.tripped = true;
            self.last_transition_time_seconds = Some(time_seconds);
        }
        self
    }
}

pub(crate) struct BreakerDevice {
    pub(crate) id: crate::StableId,
    pub(crate) poles: [super::LinkedPole; 3],
    pub(crate) resistance: f64,
    pub(crate) state: BreakerState,
}

impl super::Device for BreakerDevice {
    fn stamp(
        &self,
        system: &mut crate::mna::MnaSystem,
        variables: &crate::mna::NodeVariables,
    ) -> Result<(), crate::Diagnostic> {
        super::LinkedContacts {
            poles: &self.poles,
            active: self.state.conducting(),
        }
        .stamp(&self.id, self.resistance, system, variables)
    }

    fn result(
        &self,
        node_voltages: &[f64],
        _solution_values: &[f64],
    ) -> Result<super::DeviceResult, crate::Diagnostic> {
        let mut extra_values = Vec::with_capacity(7);
        let conducting = self.state.conducting();
        super::LinkedContacts {
            poles: &self.poles,
            active: conducting,
        }
        .append_results(&self.id, self.resistance, node_voltages, &mut extra_values)?;
        extra_values.push((
            format!("{}/tripped", self.id.as_str()),
            f64::from(self.state.tripped),
        ));
        Ok(super::DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: 0.0,
            current_amps: 0.0,
            extra_values,
            state: Some(crate::DeviceState::Switch {
                component_id: self.id.clone(),
                position: self.state.position,
                conducting,
                last_transition_time_seconds: self.state.last_transition_time_seconds,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_coil_detector_reaches_the_held_squared_current() {
        let value = detector_step(0.0, 2.0, -2.0, 0.02, 0.02).unwrap();
        assert!((value - 4.0 * (1.0 - (-1.0_f64).exp())).abs() < 1e-14);
        assert_eq!(
            coil_contact_state(RelayContactState::Released, value, 1.0, 0.5),
            RelayContactState::Picked
        );
        assert_eq!(
            coil_contact_state(RelayContactState::Picked, 0.25, 1.0, 0.5),
            RelayContactState::Released
        );
    }

    #[test]
    fn any_pole_can_trip_the_breaker_and_open_resets_the_latch() {
        let open = BreakerState {
            position: SwitchPosition::Open,
            tripped: false,
            last_transition_time_seconds: None,
        };
        let closed = open.actuated(OperatorPosition::Closed, 0.005).unwrap();
        assert!(closed.conducting());
        let tripped = closed.trip_if_needed([1.0, -5.0, 2.0], 5.0, 0.006);
        assert!(tripped.tripped);
        assert!(!tripped.conducting());
        assert_eq!(tripped.last_transition_time_seconds, Some(0.006));
        assert!(tripped.actuated(OperatorPosition::Closed, 0.007).is_none());
        let reset = tripped.actuated(OperatorPosition::Open, 0.008).unwrap();
        assert!(!reset.tripped);
        assert_eq!(reset.last_transition_time_seconds, Some(0.006));
        assert!(
            reset
                .actuated(OperatorPosition::Closed, 0.009)
                .unwrap()
                .conducting()
        );
    }
}
