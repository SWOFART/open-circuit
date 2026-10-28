//! Running a check for the desktop shell (docs/contracts/stage-5-assignments.md sections 4.6
//! and 6).
//!
//! A check has an Engine instance of its own and never touches the simulation thread, so it
//! cannot delay a live frame or change the run the user is watching. The bridge adds the
//! bounds the Engine leaves to its caller: at most 64 criteria, and one check at a time, so
//! that repeated requests queue instead of using every core.

use std::sync::Mutex;

use open_circuit_engine::{CheckRequest, CheckResult, MAX_CHECK_CRITERIA, evaluate_checks};

/// Runs checks one at a time.
#[derive(Default)]
pub struct CheckService {
    running: Mutex<()>,
}

impl CheckService {
    /// Evaluates a check. The result carries the request's document revision, so that the
    /// caller can discard it when the document has moved on. A request over the limits is
    /// refused here and not evaluated.
    pub fn run(&self, request: CheckRequest) -> Result<CheckResult, String> {
        if request.checks.len() > MAX_CHECK_CRITERIA {
            return Err(format!(
                "A check can have at most {MAX_CHECK_CRITERIA} criteria."
            ));
        }
        // The guard protects no data, so a panic in an earlier check does not matter.
        let _running = self
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(evaluate_checks(request))
    }
}
