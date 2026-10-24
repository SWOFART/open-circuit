//! The simulation thread and the state machine it runs.
//!
//! `SimulationCore` is a plain state machine: it owns the [`Engine`], advances the fixed
//! 1 ms physics step against a wall clock it is given, and decides when to deliver the
//! latest snapshot. It has no threads, no Tauri types, and takes the current time as an
//! argument, so its timing rules can be tested with a fake clock.
//! `SimulationService` runs one core on a dedicated thread and is what Tauri commands call.
//!
//! Rules, from ARCHITECTURE.md section 4 and the Stage 1 contract:
//! - Physics advances at the fixed step regardless of how fast the UI consumes frames.
//! - Only the latest snapshot is ever delivered; a lagging UI skips frames.
//! - At most one frame per `frame_interval` (about 60 per second).
//! - The UI acknowledges frames. With `max_in_flight` frames unacknowledged, nothing more is
//!   sent, so a slow webview cannot make the backend queue frames without bound.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, Diagnostic, DiagnosticCode, Engine,
    SetParameterRequest, SetParameterResult, StableId,
};

use crate::frame::{MAX_REVISION_BYTES, encode_frame};

/// The fixed physics step. It equals `SIMULATION_TIMESTEP_SECONDS` (checked by a test).
pub const PHYSICS_STEP: Duration = Duration::from_millis(1);

/// How long a caller waits for the simulation thread to answer a request.
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// Where encoded frames go. Returning an error means the receiver is gone for good.
pub trait FrameSink: Send + 'static {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinkClosed;

#[derive(Clone, Debug)]
pub struct SimulationConfig {
    /// Minimum time between delivered frames. Never changes the physics step.
    pub frame_interval: Duration,
    /// Unacknowledged frames allowed before delivery pauses.
    pub max_in_flight: usize,
    /// After this long without an acknowledgement, a frame no longer counts as in flight,
    /// so one lost acknowledgement cannot stall delivery forever.
    pub ack_timeout: Duration,
    /// Cap on physics steps taken at once. After a stall (for example the machine
    /// sleeping) the backlog is dropped instead of being simulated in one burst.
    pub max_steps_per_tick: u32,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            frame_interval: Duration::from_nanos(1_000_000_000 / 60),
            max_in_flight: 2,
            ack_timeout: Duration::from_secs(1),
            max_steps_per_tick: 1_000,
        }
    }
}

fn revision_too_long(revision: &str) -> bool {
    revision.len() > MAX_REVISION_BYTES
}

struct Stream {
    run_id: StableId,
    sink: Box<dyn FrameSink>,
    /// Wall time up to which whole physics steps have been taken.
    advanced_to: Instant,
    next_frame_at: Instant,
    last_sent_sequence: Option<u64>,
    in_flight: VecDeque<(u64, Instant)>,
}

pub struct SimulationCore {
    engine: Engine,
    config: SimulationConfig,
    stream: Option<Stream>,
}

impl SimulationCore {
    pub fn new(config: SimulationConfig) -> Self {
        Self {
            engine: Engine::default(),
            config,
            stream: None,
        }
    }

    /// A successful compile starts a new run, which ends any running stream.
    pub fn compile(&mut self, request: CompileRequest) -> CompileResult {
        // A revision too long for the frame envelope would let the compile succeed and then
        // never produce a frame, so it is refused here, where the UI can be told.
        if revision_too_long(&request.document_revision) {
            return CompileResult::Failure {
                diagnostics: vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    Vec::new(),
                )],
            };
        }

        let result = self.engine.compile(request);

        if matches!(result, CompileResult::Success { .. }) {
            self.stream = None;
        }

        result
    }

    /// Delegates to the Engine, which owns revision and run checks. Frames after a
    /// successful update carry the new revision.
    pub fn set_parameter(&mut self, request: SetParameterRequest) -> SetParameterResult {
        if revision_too_long(&request.document_revision) {
            return SetParameterResult::Failure {
                diagnostics: vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    Vec::new(),
                )],
            };
        }

        self.engine.set_parameter(request)
    }

    /// Begins delivering frames of the active run to `sink`, replacing any earlier stream.
    pub fn start(
        &mut self,
        run_id: StableId,
        sink: Box<dyn FrameSink>,
        now: Instant,
    ) -> Result<(), Vec<Diagnostic>> {
        let active = self
            .engine
            .latest_snapshot()
            .is_some_and(|snapshot| snapshot.metadata.run_id == run_id);

        if !active {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::StaleRun,
                Vec::new(),
            )]);
        }

        self.stream = Some(Stream {
            run_id,
            sink,
            advanced_to: now,
            next_frame_at: now,
            last_sent_sequence: None,
            in_flight: VecDeque::new(),
        });
        Ok(())
    }

    /// Ends the stream and drops its sink, which releases the receiving side. With
    /// `Some(run_id)` only a stream of that run is ended, so a late stop for an earlier
    /// run cannot cut off the stream of a newer one.
    pub fn stop(&mut self, run_id: Option<&StableId>) {
        let matches = match (run_id, self.stream.as_ref()) {
            (Some(wanted), Some(stream)) => &stream.run_id == wanted,
            (None, _) => true,
            (Some(_), None) => false,
        };

        if matches {
            self.stream = None;
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.stream.is_some()
    }

    /// The UI has processed every frame up to and including `frame_sequence`.
    pub fn acknowledge(&mut self, frame_sequence: u64) {
        if let Some(stream) = self.stream.as_mut() {
            while stream
                .in_flight
                .front()
                .is_some_and(|(sequence, _)| *sequence <= frame_sequence)
            {
                stream.in_flight.pop_front();
            }
        }
    }

    /// When `tick` next needs to run, or `None` when there is nothing to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.stream.as_ref().map(|stream| stream.next_frame_at)
    }

    /// Advances physics to `now` and delivers the latest snapshot if one is due.
    pub fn tick(&mut self, now: Instant) {
        let Self {
            engine,
            config,
            stream,
        } = self;
        let Some(active) = stream.as_mut() else {
            return;
        };

        let elapsed = now.saturating_duration_since(active.advanced_to);
        let due = elapsed.as_nanos() / PHYSICS_STEP.as_nanos();

        if due > 0 {
            let steps = due.min(u128::from(config.max_steps_per_tick)) as u32;

            if u128::from(steps) < due {
                active.advanced_to = now;
            } else {
                active.advanced_to += PHYSICS_STEP * steps;
            }

            let advance = AdvanceRequest {
                run_id: active.run_id.clone(),
                step_count: steps,
            };

            if engine.advance(advance).is_err() {
                // The run was replaced or cannot advance any further.
                *stream = None;
                return;
            }
        }

        if now < active.next_frame_at {
            return;
        }

        // The next attempt is always in the future, so a blocked delivery is retried at
        // the frame cadence rather than in a busy loop.
        active.next_frame_at += config.frame_interval;
        if active.next_frame_at <= now {
            active.next_frame_at = now + config.frame_interval;
        }

        while active.in_flight.front().is_some_and(|(_, sent_at)| {
            now.saturating_duration_since(*sent_at) >= config.ack_timeout
        }) {
            active.in_flight.pop_front();
        }

        if active.in_flight.len() >= config.max_in_flight {
            return;
        }

        let Some(snapshot) = engine.latest_snapshot() else {
            return;
        };
        let sequence = snapshot.metadata.frame_sequence;

        if active
            .last_sent_sequence
            .is_some_and(|last| sequence <= last)
        {
            return;
        }

        let delivered = encode_frame(snapshot)
            .ok()
            .is_some_and(|frame| active.sink.send(frame).is_ok());

        if delivered {
            active.last_sent_sequence = Some(sequence);
            active.in_flight.push_back((sequence, now));
        } else {
            // An unencodable snapshot or a closed receiver ends this stream.
            *stream = None;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceError {
    /// The simulation thread has stopped, so the request could not be handled.
    Stopped,
    /// The simulation thread did not answer in time.
    Timeout,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => formatter.write_str("the simulation thread is not running"),
            Self::Timeout => formatter.write_str("the simulation thread did not answer in time"),
        }
    }
}

impl std::error::Error for ServiceError {}

enum Message {
    Compile(CompileRequest, Sender<CompileResult>),
    SetParameter(SetParameterRequest, Sender<SetParameterResult>),
    Start(
        StableId,
        Box<dyn FrameSink>,
        Sender<Result<(), Vec<Diagnostic>>>,
    ),
    Stop(Option<StableId>),
    Acknowledge(u64),
    Shutdown,
}

/// Runs a [`SimulationCore`] on its own thread. Cloning is not needed: share it in an `Arc`.
pub struct SimulationService {
    sender: Sender<Message>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl SimulationService {
    pub fn spawn(config: SimulationConfig) -> Self {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("open-circuit-simulation".to_owned())
            .spawn(move || run_worker(SimulationCore::new(config), receiver))
            .expect("the operating system must be able to start the simulation thread");

        Self {
            sender,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub fn compile(&self, request: CompileRequest) -> Result<CompileResult, ServiceError> {
        self.request(|reply| Message::Compile(request, reply))
    }

    pub fn set_parameter(
        &self,
        request: SetParameterRequest,
    ) -> Result<SetParameterResult, ServiceError> {
        self.request(|reply| Message::SetParameter(request, reply))
    }

    pub fn start(
        &self,
        run_id: StableId,
        sink: Box<dyn FrameSink>,
    ) -> Result<Result<(), Vec<Diagnostic>>, ServiceError> {
        self.request(|reply| Message::Start(run_id, sink, reply))
    }

    /// Ends the stream of `run_id`, or any stream when `None`.
    pub fn stop(&self, run_id: Option<StableId>) -> Result<(), ServiceError> {
        self.sender
            .send(Message::Stop(run_id))
            .map_err(|_| ServiceError::Stopped)
    }

    pub fn acknowledge(&self, frame_sequence: u64) -> Result<(), ServiceError> {
        self.sender
            .send(Message::Acknowledge(frame_sequence))
            .map_err(|_| ServiceError::Stopped)
    }

    /// Stops the thread and waits for it to finish. Safe to call more than once.
    pub fn shutdown(&self) {
        let _ = self.sender.send(Message::Shutdown);
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();

        if let Some(worker) = worker {
            let _ = worker.join();
        }
    }

    fn request<T>(&self, build: impl FnOnce(Sender<T>) -> Message) -> Result<T, ServiceError> {
        let (reply, answer) = mpsc::channel();
        self.sender
            .send(build(reply))
            .map_err(|_| ServiceError::Stopped)?;

        answer
            .recv_timeout(REPLY_TIMEOUT)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => ServiceError::Timeout,
                RecvTimeoutError::Disconnected => ServiceError::Stopped,
            })
    }
}

impl Drop for SimulationService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_worker(mut core: SimulationCore, receiver: Receiver<Message>) {
    loop {
        let message = match core.next_deadline() {
            Some(deadline) => {
                receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };

        match message {
            Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(Message::Compile(request, reply)) => {
                let _ = reply.send(core.compile(request));
            }
            Ok(Message::SetParameter(request, reply)) => {
                let _ = reply.send(core.set_parameter(request));
            }
            Ok(Message::Start(run_id, sink, reply)) => {
                let _ = reply.send(core.start(run_id, sink, Instant::now()));
            }
            Ok(Message::Stop(run_id)) => core.stop(run_id.as_ref()),
            Ok(Message::Acknowledge(frame_sequence)) => core.acknowledge(frame_sequence),
            Err(RecvTimeoutError::Timeout) => {}
        }

        core.tick(Instant::now());
    }

    // Dropping the core drops the stream and its sink, releasing the receiver.
}
