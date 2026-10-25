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

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, Diagnostic, DiagnosticCode, Engine,
    SetParameterRequest, SetParameterResult, StableId, TraceRequest,
};

use crate::frame::{MAX_REVISION_BYTES, encode_frame};
use crate::trace::{MAX_TRACE_POINTS, encode_trace};

/// The fixed physics step. It equals `SIMULATION_TIMESTEP_SECONDS` (checked by a test).
pub const PHYSICS_STEP: Duration = Duration::from_millis(1);

/// How long a caller waits for the simulation thread to answer a request.
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// Where encoded frames go. Returning an error means the receiver is gone for good.
pub trait FrameSink: Send + 'static {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed>;
}

/// Where one encoded, bounded trace response goes. Returning an error means the receiver is gone.
pub trait TraceSink: Send + 'static {
    fn send(&mut self, response: Vec<u8>) -> Result<(), SinkClosed>;
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

struct PendingTrace {
    run_id: StableId,
    sequence: u64,
    sent_at: Instant,
}

struct PendingTraceRequest {
    sequence: Option<u64>,
    sent_at: Option<Instant>,
    early_acknowledgement: Option<u64>,
    completed: Receiver<()>,
    timed_out: bool,
}

pub struct SimulationCore {
    engine: Engine,
    config: SimulationConfig,
    stream: Option<Stream>,
    pending_trace: Option<PendingTrace>,
    next_trace_sequence: u64,
}

impl SimulationCore {
    pub fn new(config: SimulationConfig) -> Self {
        Self {
            engine: Engine::default(),
            config,
            stream: None,
            pending_trace: None,
            next_trace_sequence: 1,
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
            self.pending_trace = None;
            self.next_trace_sequence = 1;
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

    /// Sends one bounded trace response. A second response waits for acknowledgement or timeout.
    pub fn read_traces(
        &mut self,
        mut request: TraceRequest,
        mut sink: Box<dyn TraceSink>,
        now: Instant,
    ) -> Result<u64, Vec<Diagnostic>> {
        let entered_at = Instant::now();
        if self.pending_trace.as_ref().is_some_and(|pending| {
            now.saturating_duration_since(pending.sent_at) < self.config.ack_timeout
        }) {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        }
        self.pending_trace = None;
        request.max_points = request.max_points.min(MAX_TRACE_POINTS as u32);
        let batch = self.engine.read_traces(request.clone())?;
        let sequence = self.next_trace_sequence;
        let Some(next_sequence) = sequence.checked_add(1) else {
            return Err(vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]);
        };
        let response = encode_trace(&batch, sequence).map_err(|_| {
            vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]
        })?;
        sink.send(response).map_err(|_| {
            vec![Diagnostic::error(
                DiagnosticCode::InvalidParameter,
                Vec::new(),
            )]
        })?;
        let sent_at = now.checked_add(entered_at.elapsed()).unwrap_or(now);
        self.pending_trace = Some(PendingTrace {
            run_id: request.run_id,
            sequence,
            sent_at,
        });
        self.next_trace_sequence = next_sequence;
        Ok(sequence)
    }

    /// Releases the matching outstanding trace response; late acknowledgements are ignored.
    pub fn acknowledge_trace(&mut self, run_id: &StableId, trace_sequence: u64) {
        if self
            .pending_trace
            .as_ref()
            .is_some_and(|pending| &pending.run_id == run_id && pending.sequence == trace_sequence)
        {
            self.pending_trace = None;
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
            ..
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
    ReadTraces(
        TraceRequest,
        Box<dyn TraceSink>,
        Sender<()>,
        Sender<Result<u64, Vec<Diagnostic>>>,
    ),
    AcknowledgeTrace(StableId, u64),
    Shutdown,
}

/// Runs a [`SimulationCore`] on its own thread. Cloning is not needed: share it in an `Arc`.
pub struct SimulationService {
    sender: Sender<Message>,
    worker: Mutex<Option<JoinHandle<()>>>,
    active_run_id: Mutex<Option<StableId>>,
    pending_trace_requests: Mutex<HashMap<StableId, PendingTraceRequest>>,
    trace_ack_timeout: Duration,
    reply_timeout: Duration,
}

impl SimulationService {
    pub fn spawn(config: SimulationConfig) -> Self {
        Self::spawn_with_reply_timeout(config, REPLY_TIMEOUT)
    }

    fn spawn_with_reply_timeout(config: SimulationConfig, reply_timeout: Duration) -> Self {
        let trace_ack_timeout = config.ack_timeout;
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("open-circuit-simulation".to_owned())
            .spawn(move || run_worker(SimulationCore::new(config), receiver))
            .expect("the operating system must be able to start the simulation thread");

        Self {
            sender,
            worker: Mutex::new(Some(worker)),
            active_run_id: Mutex::new(None),
            pending_trace_requests: Mutex::new(HashMap::new()),
            trace_ack_timeout,
            reply_timeout,
        }
    }

    pub fn compile(&self, request: CompileRequest) -> Result<CompileResult, ServiceError> {
        let result = self.request(|reply| Message::Compile(request, reply))?;
        if let CompileResult::Success { run_id, .. } = &result {
            *self
                .active_run_id
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(run_id.clone());
            self.pending_trace_requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clear();
        }
        Ok(result)
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

    pub fn read_traces(
        &self,
        request: TraceRequest,
        sink: Box<dyn TraceSink>,
    ) -> Result<Result<u64, Vec<Diagnostic>>, ServiceError> {
        let run_id = request.run_id.clone();
        let active_run_id = self
            .active_run_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active_run_id.as_ref() != Some(&run_id) {
            return Ok(Err(vec![Diagnostic::error(
                DiagnosticCode::StaleRun,
                Vec::new(),
            )]));
        }
        drop(active_run_id);

        let now = Instant::now();
        let (completion, completed) = mpsc::channel();
        {
            let mut pending = self
                .pending_trace_requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            pending.retain(|_, request| {
                if request.timed_out {
                    return matches!(request.completed.try_recv(), Err(mpsc::TryRecvError::Empty));
                }
                request.sent_at.is_none_or(|sent_at| {
                    now.saturating_duration_since(sent_at) < self.trace_ack_timeout
                })
            });
            if pending.contains_key(&run_id) {
                return Ok(Err(vec![Diagnostic::error(
                    DiagnosticCode::InvalidParameter,
                    Vec::new(),
                )]));
            }
            pending.insert(
                run_id.clone(),
                PendingTraceRequest {
                    sequence: None,
                    sent_at: None,
                    early_acknowledgement: None,
                    completed,
                    timed_out: false,
                },
            );
        }
        let result = self.request(|reply| Message::ReadTraces(request, sink, completion, reply));
        self.finish_trace_request(run_id, result)
    }

    fn finish_trace_request(
        &self,
        run_id: StableId,
        result: Result<Result<u64, Vec<Diagnostic>>, ServiceError>,
    ) -> Result<Result<u64, Vec<Diagnostic>>, ServiceError> {
        match &result {
            Ok(Ok(sequence)) => {
                let mut pending = self
                    .pending_trace_requests
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let remove = if let Some(pending_request) = pending.get_mut(&run_id) {
                    if pending_request.early_acknowledgement == Some(*sequence) {
                        true
                    } else {
                        pending_request.sequence = Some(*sequence);
                        pending_request.sent_at = Some(Instant::now());
                        pending_request.early_acknowledgement = None;
                        false
                    }
                } else {
                    false
                };
                if remove {
                    pending.remove(&run_id);
                }
            }
            Ok(Err(_)) | Err(ServiceError::Stopped) => {
                self.pending_trace_requests
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&run_id);
            }
            Err(ServiceError::Timeout) => {
                if let Some(pending_request) = self
                    .pending_trace_requests
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .get_mut(&run_id)
                {
                    // Retain admission until the worker confirms this timed-out request
                    // has completed; otherwise retries could fill the worker's queue.
                    pending_request.timed_out = true;
                    pending_request.early_acknowledgement = None;
                }
            }
        }
        result
    }

    pub fn acknowledge_trace(
        &self,
        run_id: StableId,
        trace_sequence: u64,
    ) -> Result<(), ServiceError> {
        self.sender
            .send(Message::AcknowledgeTrace(run_id.clone(), trace_sequence))
            .map_err(|_| ServiceError::Stopped)?;
        let mut pending = self
            .pending_trace_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let remove = if let Some(pending_request) = pending.get_mut(&run_id) {
            if pending_request.sequence == Some(trace_sequence) {
                true
            } else if pending_request.sequence.is_none() {
                pending_request.early_acknowledgement = Some(trace_sequence);
                false
            } else {
                false
            }
        } else {
            false
        };
        if remove {
            pending.remove(&run_id);
        }
        Ok(())
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
            .recv_timeout(self.reply_timeout)
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
            Ok(Message::ReadTraces(request, sink, completion, reply)) => {
                let result = core.read_traces(request, sink, Instant::now());
                let _ = completion.send(());
                let _ = reply.send(result);
            }
            Ok(Message::AcknowledgeTrace(run_id, trace_sequence)) => {
                core.acknowledge_trace(&run_id, trace_sequence);
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        core.tick(Instant::now());
    }

    // Dropping the core drops the stream and its sink, releasing the receiver.
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::time::Duration;

    use open_circuit_engine::{CompileRequest, CompileResult, TraceQuery, TraceRequest};

    use super::{ServiceError, SimulationConfig, SimulationService, SinkClosed, TraceSink};

    const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
    const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");

    struct BlockingSink {
        started: Sender<()>,
        release: Receiver<()>,
        output: Sender<Vec<u8>>,
    }

    impl TraceSink for BlockingSink {
        fn send(&mut self, response: Vec<u8>) -> Result<(), SinkClosed> {
            self.started.send(()).map_err(|_| SinkClosed)?;
            self.release.recv().map_err(|_| SinkClosed)?;
            self.output.send(response).map_err(|_| SinkClosed)
        }
    }

    struct ChannelSink(Sender<Vec<u8>>);

    impl TraceSink for ChannelSink {
        fn send(&mut self, response: Vec<u8>) -> Result<(), SinkClosed> {
            self.0.send(response).map_err(|_| SinkClosed)
        }
    }

    fn compile_request() -> CompileRequest {
        CompileRequest {
            schematic: serde_json::from_str(SCHEMATIC).expect("schematic fixture"),
            geometry: serde_json::from_str(GEOMETRY).expect("geometry fixture"),
            document_revision: "revision-1".to_owned(),
        }
    }

    #[test]
    fn timed_out_trace_request_stays_single_until_worker_completes_and_ack_expires() {
        let ack_timeout = Duration::from_millis(80);
        let service = SimulationService::spawn_with_reply_timeout(
            SimulationConfig {
                ack_timeout,
                ..SimulationConfig::default()
            },
            Duration::from_millis(20),
        );
        let CompileResult::Success { run_id, layout, .. } =
            service.compile(compile_request()).expect("worker replies")
        else {
            panic!("the divider should compile");
        };
        let request = TraceRequest {
            run_id: run_id.clone(),
            layout_version: layout.layout_version,
            signal_keys: vec![layout.slots[0].key.clone()],
            query: TraceQuery::History {
                start_step: 0,
                end_step: 0,
            },
            max_points: 8,
        };
        let (started_sender, started_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let (first_output_sender, first_output_receiver) = mpsc::channel();

        let result = service.read_traces(
            request.clone(),
            Box::new(BlockingSink {
                started: started_sender,
                release: release_receiver,
                output: first_output_sender,
            }),
        );
        assert!(matches!(result, Err(ServiceError::Timeout)));
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("worker entered the blocked trace sink");
        assert!(
            service
                .pending_trace_requests
                .lock()
                .unwrap()
                .get(&run_id)
                .is_some_and(|pending| pending.timed_out)
        );

        std::thread::sleep(ack_timeout + Duration::from_millis(20));
        let (blocked_sender, _blocked_receiver) = mpsc::channel();
        assert!(matches!(
            service.read_traces(request.clone(), Box::new(ChannelSink(blocked_sender))),
            Ok(Err(_))
        ));

        release_sender.send(()).expect("release blocked worker");
        first_output_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("timed-out response eventually leaves the worker");
        let (early_retry_sender, _early_retry_receiver) = mpsc::channel();
        assert!(matches!(
            service.read_traces(request.clone(), Box::new(ChannelSink(early_retry_sender))),
            Ok(Err(_))
        ));
        std::thread::sleep(ack_timeout + Duration::from_millis(20));

        let (next_sender, next_receiver) = mpsc::channel();
        service
            .read_traces(request, Box::new(ChannelSink(next_sender)))
            .expect("worker responds after the timeout cooldown")
            .expect("new trace request is admitted");
        assert_eq!(
            &next_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("new response arrives")[..4],
            b"OCT1"
        );
        service.shutdown();
    }
}
