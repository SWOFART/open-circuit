//! The job side of the extra analyses (docs/contracts/stage-17-extra-analyses.md section 5).
//!
//! An analysis (a DC sweep, a frequency response, a harmonic spectrum) can take longer than a
//! command should block, so it runs as a **job** on its own thread, never on the simulation
//! thread: a running simulation is not slowed or stopped by it. The interface starts a job, reads
//! its state and may cancel it; the application keeps, for each window, at most one pending or
//! running job and at most one terminal result.
//!
//! This module has no dependency on Tauri: the window is a label, and the Tauri commands in
//! `main.rs` are a thin layer over it.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use open_circuit_engine::engine::analyses::{
    AnalysisError, AnalysisFailure, AnalysisFailureReason, AnalysisJobReport, AnalysisKind,
    AnalysisOutcome, AnalysisRequest, AnalysisResult, AnalysisStart, MAX_ANALYSIS_PAYLOAD_BYTES,
    failure_from, run_analysis,
};

use crate::simulation::{SimulationService, SpectrumCaptureRequest};

/// What a job runs: the analysis, which asks `is_cancelled` at its own checkpoints.
type Work = Box<dyn FnOnce(&dyn Fn() -> bool) -> Result<AnalysisResult, AnalysisError> + Send>;

#[derive(Clone, Debug, PartialEq)]
enum Stage {
    Pending,
    Running,
    Cancelled,
    Succeeded(Box<AnalysisResult>),
    Failed(AnalysisFailure),
}

struct Job {
    id: u64,
    kind: AnalysisKind,
    revision: String,
    cancel: Arc<AtomicBool>,
    stage: Stage,
}

impl Job {
    fn report(&self) -> AnalysisJobReport {
        AnalysisJobReport {
            job_id: self.id.to_string(),
            analysis: self.kind,
            document_revision: self.revision.clone(),
            outcome: match &self.stage {
                Stage::Pending => AnalysisOutcome::Pending,
                Stage::Running => AnalysisOutcome::Running,
                Stage::Cancelled => AnalysisOutcome::Cancelled,
                Stage::Succeeded(result) => AnalysisOutcome::Succeeded {
                    result: (**result).clone(),
                },
                Stage::Failed(failure) => AnalysisOutcome::Failed {
                    failure: failure.clone(),
                },
            },
        }
    }
}

/// The jobs of one window: the one that is pending or running, and the last terminal one.
#[derive(Default)]
struct WindowJobs {
    active: Option<Job>,
    terminal: Option<Job>,
}

#[derive(Default)]
struct Jobs {
    windows: HashMap<String, WindowJobs>,
}

/// Starts, reads and cancels analysis jobs. Share it in an `Arc`.
pub struct AnalysisService {
    simulation: Arc<SimulationService>,
    jobs: Arc<Mutex<Jobs>>,
    next_id: AtomicU64,
}

fn lock(jobs: &Mutex<Jobs>) -> MutexGuard<'_, Jobs> {
    jobs.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl AnalysisService {
    pub fn new(simulation: Arc<SimulationService>) -> Self {
        Self {
            simulation,
            jobs: Arc::new(Mutex::new(Jobs::default())),
            next_id: AtomicU64::new(1),
        }
    }

    /// Starts an analysis for `window`. A request over the transport's bound is refused before
    /// anything runs; a window that already has a pending or running job gets `Busy`; otherwise
    /// the job starts and replaces the window's earlier terminal result.
    pub fn start(&self, window: &str, request: AnalysisRequest) -> AnalysisStart {
        if serde_json::to_vec(&request)
            .map_or(true, |bytes| bytes.len() > MAX_ANALYSIS_PAYLOAD_BYTES)
        {
            return AnalysisStart::Rejected {
                failure: AnalysisFailure::payload_too_large(),
            };
        }

        let kind = request.kind();
        let revision = request.document_revision().to_owned();
        let simulation = Arc::clone(&self.simulation);
        let work: Work = Box::new(move |is_cancelled| {
            run_analysis(
                &request,
                |capture| {
                    // Only the bounded copy of the history is read on the simulation thread.
                    simulation
                        .capture_spectrum(SpectrumCaptureRequest {
                            run_id: capture.run_id.clone(),
                            layout_version: capture.layout_version,
                            document_revision: capture.document_revision.to_owned(),
                            signal_key: capture.signal_key.to_owned(),
                            end_step: capture.end_step,
                            sample_count: capture.sample_count,
                        })
                        .map_err(|_| AnalysisError::UnavailableHistory)?
                },
                is_cancelled,
            )
        });
        self.start_job(window, kind, revision, work)
    }

    fn start_job(
        &self,
        window: &str,
        kind: AnalysisKind,
        revision: String,
        work: Work,
    ) -> AnalysisStart {
        let id = {
            let mut jobs = lock(&self.jobs);
            let entry = jobs.windows.entry(window.to_owned()).or_default();
            if entry.active.is_some() {
                return AnalysisStart::Busy;
            }

            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            entry.terminal = None;
            entry.active = Some(Job {
                id,
                kind,
                revision,
                cancel: Arc::new(AtomicBool::new(false)),
                stage: Stage::Pending,
            });
            id
        };

        let cancel = self.cancel_flag(window, id);
        let jobs = Arc::clone(&self.jobs);
        let window = window.to_owned();
        let spawned = thread::Builder::new()
            .name("open-circuit-analysis".to_owned())
            .spawn({
                let window = window.clone();
                move || {
                    set_running(&jobs, &window, id);
                    let is_cancelled = || cancel.load(Ordering::SeqCst);
                    // A panic must not leave the job running for ever.
                    let outcome = catch_unwind(AssertUnwindSafe(|| work(&is_cancelled))).unwrap_or(
                        Err(AnalysisError::SolveFailed {
                            point_index: 0,
                            diagnostics: Vec::new(),
                        }),
                    );
                    publish(&jobs, &window, id, outcome);
                }
            });
        if spawned.is_err() {
            // The operating system would not start a thread: the job fails, it does not hang.
            publish(
                &self.jobs,
                &window,
                id,
                Err(AnalysisError::SolveFailed {
                    point_index: 0,
                    diagnostics: Vec::new(),
                }),
            );
        }

        AnalysisStart::Started {
            job_id: id.to_string(),
            analysis: kind,
        }
    }

    fn cancel_flag(&self, window: &str, id: u64) -> Arc<AtomicBool> {
        let jobs = lock(&self.jobs);
        jobs.windows
            .get(window)
            .and_then(|entry| entry.active.as_ref())
            .filter(|job| job.id == id)
            .map(|job| Arc::clone(&job.cancel))
            .unwrap_or_default()
    }

    /// The state of a job of `window`, or nothing for a job the window does not own (or no longer
    /// has: a later start replaces the terminal result).
    pub fn read(&self, window: &str, job_id: &str) -> Option<AnalysisJobReport> {
        let id: u64 = job_id.parse().ok()?;
        let jobs = lock(&self.jobs);
        let entry = jobs.windows.get(window)?;
        [&entry.active, &entry.terminal]
            .into_iter()
            .flatten()
            .find(|job| job.id == id)
            .map(Job::report)
    }

    /// Cancels a pending or running job of `window` and returns its state. A job that has already
    /// finished keeps its outcome, which stays available for reading. A cancel accepted before
    /// the result is published discards the result.
    pub fn cancel(&self, window: &str, job_id: &str) -> Option<AnalysisJobReport> {
        let id: u64 = job_id.parse().ok()?;
        let jobs = lock(&self.jobs);
        let entry = jobs.windows.get(window)?;
        if let Some(job) = entry.active.as_ref().filter(|job| job.id == id) {
            job.cancel.store(true, Ordering::SeqCst);
        }
        drop(jobs);
        self.read(window, job_id)
    }

    /// Cancels every active job and forgets every terminal result: the drawing or the run they
    /// answer has been replaced, so an old result must not be read as a current one.
    pub fn invalidate(&self) {
        let mut jobs = lock(&self.jobs);
        for entry in jobs.windows.values_mut() {
            if let Some(job) = &entry.active {
                job.cancel.store(true, Ordering::SeqCst);
            }
            entry.terminal = None;
        }
    }
}

fn set_running(jobs: &Mutex<Jobs>, window: &str, id: u64) {
    let mut jobs = lock(jobs);
    if let Some(job) = jobs
        .windows
        .get_mut(window)
        .and_then(|entry| entry.active.as_mut())
        .filter(|job| job.id == id && job.stage == Stage::Pending)
    {
        job.stage = Stage::Running;
    }
}

/// Moves the job to its terminal state. The cancel flag is read under the same lock a cancel
/// takes, so a cancel accepted before this point discards the result and one accepted after it
/// finds a terminal job and changes nothing.
fn publish(
    jobs: &Mutex<Jobs>,
    window: &str,
    id: u64,
    outcome: Result<AnalysisResult, AnalysisError>,
) {
    let mut jobs = lock(jobs);
    let Some(entry) = jobs.windows.get_mut(window) else {
        return;
    };
    let Some(mut job) = entry.active.take().filter(|job| job.id == id) else {
        return;
    };

    job.stage = if job.cancel.load(Ordering::SeqCst) {
        Stage::Cancelled
    } else {
        match outcome {
            Ok(result) => match serde_json::to_vec(&result) {
                Ok(bytes) if bytes.len() <= MAX_ANALYSIS_PAYLOAD_BYTES => {
                    Stage::Succeeded(Box::new(result))
                }
                Ok(_) => Stage::Failed(AnalysisFailure::payload_too_large()),
                Err(_) => Stage::Failed(AnalysisFailure {
                    reason: AnalysisFailureReason::SolveFailed,
                    diagnostics: Vec::new(),
                    component_ids: Vec::new(),
                    point_index: None,
                }),
            },
            Err(error) => match failure_from(error) {
                Some(failure) => Stage::Failed(failure),
                None => Stage::Cancelled,
            },
        }
    };
    // A job that was invalidated while it ran ends as cancelled, never as a result.
    entry.terminal = Some(job);
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use open_circuit_engine::engine::analyses::DcSweepPointDto;
    use open_circuit_engine::{StableId, Unit};

    use super::*;
    use crate::simulation::SimulationConfig;

    fn service() -> AnalysisService {
        AnalysisService::new(Arc::new(SimulationService::spawn(
            SimulationConfig::default(),
        )))
    }

    fn result(revision: &str) -> AnalysisResult {
        AnalysisResult::DcSweep {
            document_revision: revision.to_owned(),
            source_id: StableId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
            signal_key: "x:voltage".to_owned(),
            unit: Unit::Volt,
            points: vec![DcSweepPointDto {
                source_volts: 1.0,
                reading: 2.0,
            }],
        }
    }

    /// A job that waits until the test lets it finish.
    fn blocked(
        outcome: Result<AnalysisResult, AnalysisError>,
    ) -> (Work, mpsc::Sender<()>, mpsc::Receiver<()>) {
        let (release, wait) = mpsc::channel::<()>();
        let (started, running) = mpsc::channel::<()>();
        let work: Work = Box::new(move |is_cancelled| {
            started.send(()).unwrap();
            while wait.recv_timeout(Duration::from_millis(5)).is_err() {
                if is_cancelled() {
                    return Err(AnalysisError::Cancelled);
                }
            }
            outcome
        });
        (work, release, running)
    }

    fn wait_for(service: &AnalysisService, window: &str, id: &str) -> AnalysisJobReport {
        for _ in 0..400 {
            let report = service.read(window, id).expect("the job is there");
            if !matches!(
                report.outcome,
                AnalysisOutcome::Pending | AnalysisOutcome::Running
            ) {
                return report;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("the job did not finish");
    }

    fn started(start: AnalysisStart) -> String {
        match start {
            AnalysisStart::Started { job_id, .. } => job_id,
            other => panic!("expected a started job, got {other:?}"),
        }
    }

    #[test]
    fn a_job_runs_and_its_result_is_read_with_the_revision_it_answers() {
        let service = service();
        let (work, release, running) = blocked(Ok(result("rev-1")));

        let id = started(service.start_job("main", AnalysisKind::DcSweep, "rev-1".into(), work));
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        let report = service.read("main", &id).unwrap();
        assert_eq!(report.outcome, AnalysisOutcome::Running);
        assert_eq!(report.analysis, AnalysisKind::DcSweep);
        assert_eq!(report.document_revision, "rev-1");

        release.send(()).unwrap();
        let report = wait_for(&service, "main", &id);
        assert_eq!(
            report.outcome,
            AnalysisOutcome::Succeeded {
                result: result("rev-1")
            }
        );
    }

    #[test]
    fn a_second_start_while_one_is_active_is_busy_and_changes_nothing() {
        let service = service();
        let (work, release, running) = blocked(Ok(result("a")));
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));
        running.recv_timeout(Duration::from_secs(5)).unwrap();

        let (second, _release, _running) = blocked(Ok(result("b")));
        assert_eq!(
            service.start_job("main", AnalysisKind::DcSweep, "b".into(), second),
            AnalysisStart::Busy
        );
        // Another window is not blocked by this one.
        let (other, other_release, other_running) = blocked(Ok(result("c")));
        let other_id =
            started(service.start_job("second", AnalysisKind::DcSweep, "c".into(), other));
        other_running.recv_timeout(Duration::from_secs(5)).unwrap();

        release.send(()).unwrap();
        other_release.send(()).unwrap();
        assert!(matches!(
            wait_for(&service, "main", &id).outcome,
            AnalysisOutcome::Succeeded { .. }
        ));
        assert!(matches!(
            wait_for(&service, "second", &other_id).outcome,
            AnalysisOutcome::Succeeded { .. }
        ));
    }

    #[test]
    fn a_window_reads_only_its_own_jobs() {
        let service = service();
        let (work, release, running) = blocked(Ok(result("a")));
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));
        running.recv_timeout(Duration::from_secs(5)).unwrap();

        assert!(service.read("main", &id).is_some());
        assert!(service.read("other", &id).is_none());
        assert!(service.cancel("other", &id).is_none());
        assert!(service.read("main", "not a number").is_none());
        assert!(service.read("main", "99").is_none());

        release.send(()).unwrap();
        wait_for(&service, "main", &id);
    }

    #[test]
    fn cancelling_a_running_job_ends_it_as_cancelled_and_publishes_no_points() {
        let service = service();
        let (work, _release, running) = blocked(Ok(result("a")));
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));
        running.recv_timeout(Duration::from_secs(5)).unwrap();

        service.cancel("main", &id).unwrap();

        assert_eq!(
            wait_for(&service, "main", &id).outcome,
            AnalysisOutcome::Cancelled
        );
    }

    #[test]
    fn a_cancel_accepted_before_publication_discards_a_finished_result() {
        let service = service();
        // The work ignores the flag and finishes with a result, as one that completes just as
        // the cancel arrives.
        let (release, wait) = mpsc::channel::<()>();
        let (started_tx, running) = mpsc::channel::<()>();
        let work: Work = Box::new(move |_| {
            started_tx.send(()).unwrap();
            wait.recv().unwrap();
            Ok(result("a"))
        });
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));
        running.recv_timeout(Duration::from_secs(5)).unwrap();

        service.cancel("main", &id).unwrap();
        release.send(()).unwrap();

        assert_eq!(
            wait_for(&service, "main", &id).outcome,
            AnalysisOutcome::Cancelled
        );
    }

    #[test]
    fn cancelling_a_finished_job_leaves_its_outcome_readable() {
        let service = service();
        let work: Work = Box::new(|_| Ok(result("a")));
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));
        let finished = wait_for(&service, "main", &id);

        let after = service.cancel("main", &id).unwrap();

        assert_eq!(after, finished);
        assert!(matches!(after.outcome, AnalysisOutcome::Succeeded { .. }));
    }

    #[test]
    fn a_failure_carries_a_reason_and_no_points() {
        let service = service();
        let work: Work = Box::new(|_| {
            Err(AnalysisError::SolveFailed {
                point_index: 7,
                diagnostics: Vec::new(),
            })
        });
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));

        let AnalysisOutcome::Failed { failure } = wait_for(&service, "main", &id).outcome else {
            panic!("the job must fail");
        };

        assert_eq!(failure.reason, AnalysisFailureReason::SolveFailed);
        assert_eq!(failure.point_index, Some(7));
    }

    #[test]
    fn a_panicking_job_fails_instead_of_running_for_ever() {
        let service = service();
        let work: Work = Box::new(|_| panic!("an analysis bug"));
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));

        assert!(matches!(
            wait_for(&service, "main", &id).outcome,
            AnalysisOutcome::Failed { .. }
        ));
        // The window can start again.
        let again: Work = Box::new(|_| Ok(result("b")));
        started(service.start_job("main", AnalysisKind::DcSweep, "b".into(), again));
    }

    #[test]
    fn a_successful_start_replaces_the_earlier_terminal_result() {
        let service = service();
        let first: Work = Box::new(|_| Ok(result("a")));
        let first_id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), first));
        wait_for(&service, "main", &first_id);

        let second: Work = Box::new(|_| Ok(result("b")));
        let second_id =
            started(service.start_job("main", AnalysisKind::DcSweep, "b".into(), second));
        wait_for(&service, "main", &second_id);

        assert!(service.read("main", &first_id).is_none());
        assert!(service.read("main", &second_id).is_some());
    }

    #[test]
    fn invalidating_cancels_active_jobs_and_forgets_old_results() {
        let service = service();
        let done: Work = Box::new(|_| Ok(result("a")));
        let done_id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), done));
        wait_for(&service, "main", &done_id);

        service.invalidate();
        assert!(service.read("main", &done_id).is_none());

        let (work, _release, running) = blocked(Ok(result("b")));
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "b".into(), work));
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        service.invalidate();
        // The cancelled job ends as cancelled and publishes nothing a reader would take as current.
        assert_eq!(
            wait_for_gone_or_cancelled(&service, "main", &id),
            AnalysisOutcome::Cancelled
        );
    }

    fn wait_for_gone_or_cancelled(
        service: &AnalysisService,
        window: &str,
        id: &str,
    ) -> AnalysisOutcome {
        for _ in 0..400 {
            match service.read(window, id).map(|report| report.outcome) {
                Some(AnalysisOutcome::Pending | AnalysisOutcome::Running) | None => {
                    thread::sleep(Duration::from_millis(5));
                }
                Some(outcome) => return outcome,
            }
        }
        panic!("the job did not end");
    }

    #[test]
    fn a_response_over_the_bound_is_a_structured_failure_not_a_truncated_result() {
        let service = service();
        let work: Work = Box::new(|_| {
            Ok(AnalysisResult::DcSweep {
                document_revision: "a".repeat(MAX_ANALYSIS_PAYLOAD_BYTES + 1),
                source_id: StableId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
                signal_key: "x".into(),
                unit: Unit::Volt,
                points: Vec::new(),
            })
        });
        let id = started(service.start_job("main", AnalysisKind::DcSweep, "a".into(), work));

        let AnalysisOutcome::Failed { failure } = wait_for(&service, "main", &id).outcome else {
            panic!("the response is too large");
        };

        assert_eq!(failure.reason, AnalysisFailureReason::PayloadTooLarge);
    }
}
