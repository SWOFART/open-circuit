//! Public-step state machine for Stage 14 time relays.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DelayMode {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TimerState {
    pub(crate) energized: bool,
    pub(crate) output_on: bool,
    pending_since: Option<u64>,
    pub(crate) transition_step: Option<u64>,
}

impl TimerState {
    pub(crate) fn timing(self) -> bool {
        self.pending_since.is_some()
    }

    pub(crate) fn elapsed_steps(self, step: u64) -> u64 {
        self.pending_since.map_or(0, |start| step - start)
    }

    /// Apply a settled detector sample before checking the deadline at that same step.
    pub(crate) fn sample(
        &mut self,
        mode: DelayMode,
        current_amps: f64,
        pickup_amps: f64,
        dropout_amps: f64,
        delay_steps: u64,
        step: u64,
    ) -> bool {
        let current = current_amps.abs();
        self.energized = if self.energized {
            current > dropout_amps
        } else {
            current >= pickup_amps
        };

        let delayed_value = mode == DelayMode::On;
        let next_output = if self.energized != delayed_value {
            self.pending_since = None;
            self.energized
        } else if self.output_on == delayed_value {
            self.pending_since = None;
            self.output_on
        } else {
            let start = *self.pending_since.get_or_insert(step);
            if step - start >= delay_steps {
                self.pending_since = None;
                delayed_value
            } else {
                self.output_on
            }
        };
        if next_output == self.output_on {
            return false;
        }
        self.output_on = next_output;
        self.transition_step = Some(step);
        true
    }
}

/// Recognize exact whole milliseconds despite multiplication rounding.
pub(crate) fn delay_steps(seconds: f64) -> u64 {
    let steps = seconds * 1_000.0;
    let nearest = steps.round();
    if seconds == nearest / 1_000.0 {
        nearest as u64
    } else {
        steps.ceil() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(timer: &mut TimerState, mode: DelayMode, current: f64, step: u64) -> bool {
        timer.sample(mode, current, 0.08, 0.04, delay_steps(0.0035), step)
    }

    #[test]
    fn whole_and_fractional_delays_use_public_step_deadlines() {
        assert_eq!(delay_steps(0.0), 0);
        assert_eq!(delay_steps(0.003), 3);
        assert_eq!(delay_steps(0.0035), 4);
        let mut timer = TimerState::default();
        assert!(!sample(&mut timer, DelayMode::On, 0.08, 0));
        assert!(!sample(&mut timer, DelayMode::On, 0.05, 3));
        assert_eq!(timer.elapsed_steps(3), 3);
        assert!(sample(&mut timer, DelayMode::On, 0.05, 4));
        assert_eq!(timer.transition_step, Some(4));
        assert!(!timer.timing());
        assert!(sample(&mut timer, DelayMode::On, 0.04, 5));
    }

    #[test]
    fn opposite_input_at_the_deadline_cancels_pending_transition() {
        let mut on = TimerState::default();
        assert!(!sample(&mut on, DelayMode::On, 0.1, 0));
        assert!(!sample(&mut on, DelayMode::On, 0.0, 4));
        assert!(!on.timing());
        assert!(!on.output_on);

        let mut off = TimerState::default();
        assert!(sample(&mut off, DelayMode::Off, -0.1, 0));
        assert!(!sample(&mut off, DelayMode::Off, 0.0, 1));
        assert!(!sample(&mut off, DelayMode::Off, 0.1, 5));
        assert!(off.output_on);
        assert!(!off.timing());
        assert!(!sample(&mut off, DelayMode::Off, 0.0, 6));
        assert!(sample(&mut off, DelayMode::Off, 0.0, 10));
        assert!(!off.output_on);
    }

    #[test]
    fn zero_delay_settles_at_step_zero() {
        let mut timer = TimerState::default();
        assert!(timer.sample(DelayMode::On, 0.08, 0.08, 0.04, 0, 0));
        assert_eq!(timer.transition_step, Some(0));
        assert!(timer.output_on);
    }
}
