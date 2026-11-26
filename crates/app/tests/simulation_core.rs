//! Timing, backpressure, and lifecycle rules of the simulation state machine, driven with a
//! fake clock against the real Engine and the canonical divider fixture.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::simulation::{
    FrameSink, PHYSICS_STEP, PlaybackCommand, SimulationConfig, SimulationCore, SinkClosed,
    TraceSink,
};
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, OperatorPosition, ParameterId, ResultSnapshot,
    SIMULATION_TIMESTEP_SECONDS, SetOperatorStateRequest, SetOperatorStateResult,
    SetParameterRequest, SetParameterResult, StableId, TraceQuery, TraceRequest,
};

const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const EXPECTED: &str = include_str!("../../../tests/golden/stage-1-divider.expected.json");
const RESISTOR_1: &str = "00000000-0000-4000-8000-000000000002";

/// A sink that records frames and notices when it is dropped.
#[derive(Clone, Default)]
struct RecordingSink {
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    dropped: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    drop_count: Arc<AtomicUsize>,
}

impl RecordingSink {
    fn decoded(&self) -> Vec<ResultSnapshot> {
        self.frames
            .lock()
            .expect("lock")
            .iter()
            .map(|frame| decode_frame(frame).expect("every sent frame decodes"))
            .collect()
    }

    fn count(&self) -> usize {
        self.frames.lock().expect("lock").len()
    }
}

struct SinkHandle(RecordingSink);

impl FrameSink for SinkHandle {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        if self.0.closed.load(Ordering::SeqCst) {
            return Err(SinkClosed);
        }
        self.0.frames.lock().expect("lock").push(frame);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct TraceRecordingSink {
    responses: Arc<Mutex<Vec<Vec<u8>>>>,
}

struct TraceSinkHandle(TraceRecordingSink);

impl TraceSink for TraceSinkHandle {
    fn send(&mut self, response: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.responses.lock().expect("lock").push(response);
        Ok(())
    }
}

struct DelayedTraceSink(Duration);

impl TraceSink for DelayedTraceSink {
    fn send(&mut self, _response: Vec<u8>) -> Result<(), SinkClosed> {
        std::thread::sleep(self.0);
        Ok(())
    }
}

impl Drop for SinkHandle {
    fn drop(&mut self) {
        self.0.dropped.store(true, Ordering::SeqCst);
        self.0.drop_count.fetch_add(1, Ordering::SeqCst);
    }
}

fn request(revision: &str) -> CompileRequest {
    CompileRequest {
        schematic: serde_json::from_str(SCHEMATIC).expect("schematic fixture"),
        geometry: serde_json::from_str(GEOMETRY).expect("geometry fixture"),
        document_revision: revision.to_owned(),
        faults: None,
    }
}

fn compiled_core(config: SimulationConfig) -> (SimulationCore, StableId) {
    let mut core = SimulationCore::new(config);
    let CompileResult::Success { run_id, .. } = core.compile(request("rev-1")) else {
        panic!("the canonical divider must compile");
    };
    (core, run_id)
}

/// Drives `core` with `tick_every` between ticks for `total`, acknowledging every frame
/// immediately when `ack` is set. Returns the final fake time.
/// The configuration of every test here: they drive the core with a fake clock, so the real-time
/// yield of a catch-up batch (`catch_up_budget`) is switched off. Otherwise a slow or loaded
/// machine would compute fewer steps per tick and the step counts would depend on its speed.
fn fake_clock() -> SimulationConfig {
    SimulationConfig {
        catch_up_budget: Duration::from_secs(3600),
        ..SimulationConfig::default()
    }
}

fn drive(
    core: &mut SimulationCore,
    sink: &RecordingSink,
    start: Instant,
    total: Duration,
    tick_every: Duration,
    ack: bool,
) -> Instant {
    let mut elapsed = Duration::ZERO;
    let mut acknowledged = 0;

    while elapsed < total {
        elapsed += tick_every;
        core.tick(start + elapsed);

        if ack {
            for snapshot in sink.decoded().iter().skip(acknowledged) {
                core.acknowledge(snapshot.metadata.frame_sequence);
            }
            acknowledged = sink.count();
        }
    }

    start + elapsed
}

#[test]
fn the_physics_step_matches_the_contract_timestep() {
    assert_eq!(PHYSICS_STEP.as_secs_f64(), SIMULATION_TIMESTEP_SECONDS);
}

#[test]
fn start_refuses_a_run_that_is_not_active() {
    let (mut core, _) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    let stale = StableId::parse("00000000-0000-4000-8000-0000000000ff").expect("valid ID");

    let error = core
        .start(stale, Box::new(SinkHandle(sink.clone())), Instant::now())
        .expect_err("a run that was never compiled cannot be started");

    assert_eq!(error[0].code, DiagnosticCode::StaleRun);
    assert!(!core.is_streaming());
    assert!(
        sink.dropped.load(Ordering::SeqCst),
        "the refused sink is released"
    );
}

#[test]
fn start_refuses_when_nothing_was_compiled() {
    let mut core = SimulationCore::new(fake_clock());
    let run_id = StableId::parse("00000000-0000-4000-8000-0000000000a0").expect("valid ID");

    assert!(
        core.start(
            run_id,
            Box::new(SinkHandle(RecordingSink::default())),
            Instant::now()
        )
        .is_err()
    );
}

#[test]
fn delivers_at_most_sixty_frames_per_second_while_physics_runs_at_the_fixed_step() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    // Tick every millisecond for one simulated second, acknowledging at once.
    drive(
        &mut core,
        &sink,
        start,
        Duration::from_secs(1),
        Duration::from_millis(1),
        true,
    );

    let frames = sink.decoded();
    assert!(
        (55..=61).contains(&frames.len()),
        "expected about 60 frames in one second, got {}",
        frames.len()
    );

    let last = frames.last().expect("frames were delivered");
    assert_eq!(
        last.metadata.simulation_step, 1000,
        "one second is 1000 steps"
    );
    assert_eq!(last.metadata.simulation_time_seconds, 1.0);

    for pair in frames.windows(2) {
        assert!(pair[1].metadata.frame_sequence > pair[0].metadata.frame_sequence);
        assert!(pair[1].metadata.simulation_step >= pair[0].metadata.simulation_step);
    }
}

#[test]
fn a_fast_ui_does_not_speed_physics_up_or_slow_it_down() {
    let physics_after = |tick_every: Duration| {
        let (mut core, run_id) = compiled_core(fake_clock());
        let sink = RecordingSink::default();
        let start = Instant::now();
        core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
            .expect("start");
        drive(
            &mut core,
            &sink,
            start,
            Duration::from_secs(2),
            tick_every,
            true,
        );
        sink.decoded()
            .last()
            .expect("frames")
            .metadata
            .simulation_step
    };

    // Frames are bounded by the frame interval, so a 1 ms and a 16 ms tick both reach
    // the same simulation time within one frame interval's worth of steps.
    let fine = physics_after(Duration::from_millis(1));
    let coarse = physics_after(Duration::from_millis(16));

    assert_eq!(fine, 2000);
    assert!(
        (1980..=2000).contains(&coarse),
        "coarse run reached step {coarse}"
    );
}

#[test]
fn a_consumer_that_never_acknowledges_gets_a_bounded_number_of_frames() {
    let config = SimulationConfig {
        ack_timeout: Duration::from_secs(60),
        ..fake_clock()
    };
    let max_in_flight = config.max_in_flight;
    let (mut core, run_id) = compiled_core(config);
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    drive(
        &mut core,
        &sink,
        start,
        Duration::from_secs(5),
        Duration::from_millis(1),
        false,
    );

    assert_eq!(
        sink.count(),
        max_in_flight,
        "no more than max_in_flight frames may be sent without acknowledgements"
    );
}

#[test]
fn after_a_stall_only_the_latest_snapshot_is_delivered_skipping_the_rest() {
    let config = SimulationConfig {
        ack_timeout: Duration::from_secs(60),
        ..fake_clock()
    };
    let (mut core, run_id) = compiled_core(config);
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    // The consumer stalls for three seconds of simulated time, then catches up.
    let stalled_until = drive(
        &mut core,
        &sink,
        start,
        Duration::from_secs(3),
        Duration::from_millis(1),
        false,
    );
    let sent_while_stalled = sink.count();
    let last_delivered = sink.decoded().last().expect("frames").metadata.clone();

    for snapshot in sink.decoded() {
        core.acknowledge(snapshot.metadata.frame_sequence);
    }
    core.tick(stalled_until + Duration::from_millis(20));

    let frames = sink.decoded();
    assert_eq!(
        frames.len(),
        sent_while_stalled + 1,
        "exactly one catch-up frame"
    );

    let caught_up = frames.last().expect("frames").metadata.clone();
    assert!(caught_up.simulation_step > last_delivered.simulation_step + 2000);
    assert!(
        caught_up.frame_sequence > last_delivered.frame_sequence + 1_000,
        "the intermediate snapshots were skipped, not queued"
    );
}

#[test]
fn a_lost_acknowledgement_does_not_stall_delivery_forever() {
    let config = SimulationConfig {
        ack_timeout: Duration::from_millis(500),
        ..fake_clock()
    };
    let (mut core, run_id) = compiled_core(config);
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    drive(
        &mut core,
        &sink,
        start,
        Duration::from_millis(300),
        Duration::from_millis(1),
        false,
    );
    assert_eq!(
        sink.count(),
        2,
        "blocked while acknowledgements are outstanding"
    );

    drive(
        &mut core,
        &sink,
        start + Duration::from_millis(300),
        Duration::from_millis(700),
        Duration::from_millis(1),
        false,
    );
    assert!(
        sink.count() > 2,
        "delivery resumed after the acknowledgement timeout"
    );
}

#[test]
fn acknowledging_a_later_frame_releases_every_earlier_one() {
    let (mut core, run_id) = compiled_core(SimulationConfig {
        ack_timeout: Duration::from_secs(60),
        ..fake_clock()
    });
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");
    drive(
        &mut core,
        &sink,
        start,
        Duration::from_millis(100),
        Duration::from_millis(1),
        false,
    );
    assert_eq!(sink.count(), 2);

    let newest = sink
        .decoded()
        .last()
        .expect("frames")
        .metadata
        .frame_sequence;
    core.acknowledge(newest);
    core.tick(start + Duration::from_millis(150));

    assert_eq!(
        sink.count(),
        3,
        "both slots were released by one acknowledgement"
    );
}

#[test]
fn a_parameter_update_reaches_the_next_frame_with_the_new_revision_and_values() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id.clone(), Box::new(SinkHandle(sink.clone())), start)
        .expect("start");
    let now = drive(
        &mut core,
        &sink,
        start,
        Duration::from_millis(100),
        Duration::from_millis(1),
        true,
    );

    let SetParameterResult::Success { snapshot, .. } = core.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "rev-1".to_owned(),
        document_revision: "rev-2".to_owned(),
        component_id: StableId::parse(RESISTOR_1).expect("valid ID"),
        parameter_id: ParameterId::ResistanceOhms,
        value: 3000.0,
    }) else {
        panic!("the update must succeed");
    };

    drive(
        &mut core,
        &sink,
        now,
        Duration::from_millis(100),
        Duration::from_millis(1),
        true,
    );

    let after: Vec<_> = sink
        .decoded()
        .into_iter()
        .filter(|frame| frame.metadata.document_revision == "rev-2")
        .collect();
    assert!(!after.is_empty(), "frames carry the new revision");
    assert!(after[0].metadata.frame_sequence >= snapshot.metadata.frame_sequence);
    assert_eq!(after[0].values.len(), snapshot.values.len());

    let expected: serde_json::Value = serde_json::from_str(EXPECTED).expect("expected fixture");
    let expected_r2 = expected["resistor1At3000Ohms"]["resistor2VoltageVolts"]
        .as_f64()
        .expect("number");
    assert!(
        after[0]
            .values
            .iter()
            .any(|value| (value - expected_r2).abs() < 1e-6),
        "the new frame holds the recalculated divider voltage"
    );

    // Frames of the old revision never follow frames of the new one.
    let revisions: Vec<_> = sink
        .decoded()
        .into_iter()
        .map(|frame| frame.metadata.document_revision)
        .collect();
    let first_new = revisions
        .iter()
        .position(|r| r == "rev-2")
        .expect("new frames");
    assert!(revisions[first_new..].iter().all(|r| r == "rev-2"));
}

#[test]
fn a_rejected_update_leaves_the_stream_and_its_values_unchanged() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id.clone(), Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    let result = core.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "wrong-revision".to_owned(),
        document_revision: "rev-2".to_owned(),
        component_id: StableId::parse(RESISTOR_1).expect("valid ID"),
        parameter_id: ParameterId::ResistanceOhms,
        value: 3000.0,
    });

    assert!(matches!(result, SetParameterResult::Failure { .. }));
    assert!(core.is_streaming());
    drive(
        &mut core,
        &sink,
        start,
        Duration::from_millis(50),
        Duration::from_millis(1),
        true,
    );
    assert!(
        sink.decoded()
            .iter()
            .all(|frame| frame.metadata.document_revision == "rev-1")
    );
}

#[test]
fn stopping_releases_the_sink() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), Instant::now())
        .expect("start");
    assert!(!sink.dropped.load(Ordering::SeqCst));

    core.stop(None);

    assert!(sink.dropped.load(Ordering::SeqCst));
    assert!(!core.is_streaming());
    assert!(
        core.next_deadline().is_none(),
        "an idle core needs no wake-up"
    );
}

#[test]
fn a_stop_for_another_run_does_not_end_the_stream() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    core.start(
        run_id.clone(),
        Box::new(SinkHandle(sink.clone())),
        Instant::now(),
    )
    .expect("start");
    let other = StableId::parse("00000000-0000-4000-8000-0000000000ee").expect("valid ID");

    core.stop(Some(&other));
    assert!(
        core.is_streaming(),
        "a late stop for an earlier run is ignored"
    );
    assert!(!sink.dropped.load(Ordering::SeqCst));

    core.stop(Some(&run_id));
    assert!(!core.is_streaming());
    assert!(sink.dropped.load(Ordering::SeqCst));
}

#[test]
fn stopping_with_no_stream_is_harmless() {
    let (mut core, run_id) = compiled_core(fake_clock());

    core.stop(Some(&run_id));
    core.stop(None);

    assert!(!core.is_streaming());
}

#[test]
fn a_failed_compile_keeps_the_old_run_and_stream_but_a_new_run_ends_it() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), Instant::now())
        .expect("start");

    // A geometry envelope with no components cannot describe the schematic's pins.
    let mut broken = request("rev-9");
    broken.geometry.components.clear();
    assert!(matches!(
        core.compile(broken),
        CompileResult::Failure { .. }
    ));
    assert!(
        core.is_streaming(),
        "a failed compile leaves the old run streaming"
    );
    assert!(!sink.dropped.load(Ordering::SeqCst));

    assert!(matches!(
        core.compile(request("rev-10")),
        CompileResult::Success { .. }
    ));
    assert!(
        sink.dropped.load(Ordering::SeqCst),
        "the old stream's sink is released"
    );
    assert!(!core.is_streaming());
}

#[test]
fn a_closed_receiver_ends_the_stream_instead_of_retrying() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");
    sink.closed.store(true, Ordering::SeqCst);

    core.tick(start + Duration::from_millis(20));

    assert!(!core.is_streaming());
    assert!(sink.dropped.load(Ordering::SeqCst));
    assert_eq!(sink.count(), 0);
}

#[test]
fn a_long_stall_is_not_simulated_in_one_burst() {
    let config = fake_clock();
    let cap = u64::from(config.max_steps_per_tick);
    let (mut core, run_id) = compiled_core(config);
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    // The machine sleeps for ten minutes between two ticks.
    core.tick(start + Duration::from_secs(600));

    let frame = sink.decoded().pop().expect("a frame was delivered");
    assert!(frame.metadata.simulation_step <= cap);
}

#[test]
fn a_revision_too_long_for_a_frame_is_refused_instead_of_silently_ending_the_stream() {
    let too_long = "r".repeat(open_circuit_app_lib::frame::MAX_REVISION_BYTES + 1);

    // At compile: the failure is reported and no run is created.
    let mut fresh = SimulationCore::new(fake_clock());
    let CompileResult::Failure { diagnostics } = fresh.compile(request(&too_long)) else {
        panic!("an over-long revision must not compile");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);

    // At update: the failure is reported and the run, stream, and values are unchanged.
    let (mut core, run_id) = compiled_core(fake_clock());
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id.clone(), Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    let SetParameterResult::Failure { diagnostics } = core.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "rev-1".to_owned(),
        document_revision: too_long,
        component_id: StableId::parse(RESISTOR_1).expect("valid ID"),
        parameter_id: ParameterId::ResistanceOhms,
        value: 3000.0,
    }) else {
        panic!("an over-long revision must be refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    assert!(core.is_streaming());
    drive(
        &mut core,
        &sink,
        start,
        Duration::from_millis(50),
        Duration::from_millis(1),
        true,
    );
    assert!(
        sink.decoded()
            .iter()
            .all(|frame| frame.metadata.document_revision == "rev-1")
    );

    // The longest allowed revision still works end to end.
    let longest = "r".repeat(open_circuit_app_lib::frame::MAX_REVISION_BYTES);
    let mut other = SimulationCore::new(fake_clock());
    assert!(matches!(
        other.compile(request(&longest)),
        CompileResult::Success { .. }
    ));
}

#[test]
fn trace_bridge_is_bounded_acknowledged_and_rejects_a_stale_run() {
    let (mut core, run_id) = compiled_core(fake_clock());
    let frame_sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id.clone(), Box::new(SinkHandle(frame_sink)), start)
        .expect("start");
    core.tick(start + Duration::from_millis(5));
    core.stop(Some(&run_id));
    assert!(!core.is_streaming());

    let request = TraceRequest {
        run_id: run_id.clone(),
        layout_version: 1,
        signal_keys: vec![format!("{RESISTOR_1}:voltage")],
        query: TraceQuery::History {
            start_step: 0,
            end_step: 5,
        },
        max_points: 4096,
    };
    let sink = TraceRecordingSink::default();
    core.read_traces(
        request.clone(),
        Box::new(TraceSinkHandle(sink.clone())),
        start + Duration::from_millis(5),
    )
    .expect("first bounded response");

    let response = sink.responses.lock().expect("lock")[0].clone();
    assert_eq!(&response[..4], b"OCT1");
    assert!(response.len() <= open_circuit_app_lib::trace::MAX_RESPONSE_BYTES);
    let header_length = u32::from_le_bytes(response[4..8].try_into().unwrap()) as usize;
    let header: serde_json::Value =
        serde_json::from_slice(&response[8..8 + header_length]).expect("valid trace header");
    assert_eq!(header["pointCount"], 6);
    assert_eq!(header["metadata"]["sampleCount"], 6);
    assert_eq!(header["metadata"]["firstSampleStep"], "0");
    assert_eq!(header["metadata"]["lastSampleStep"], "5");
    let sequence = header["traceSequence"]
        .as_str()
        .expect("trace sequence")
        .parse::<u64>()
        .expect("valid trace sequence");

    let busy = core.read_traces(
        request.clone(),
        Box::new(TraceSinkHandle(TraceRecordingSink::default())),
        start + Duration::from_millis(6),
    );
    assert_eq!(busy.unwrap_err()[0].code, DiagnosticCode::InvalidParameter);

    core.acknowledge_trace(&run_id, sequence);
    core.read_traces(
        request.clone(),
        Box::new(TraceSinkHandle(TraceRecordingSink::default())),
        start + Duration::from_millis(7),
    )
    .expect("acknowledgement releases the next response");
    core.read_traces(
        request,
        Box::new(TraceSinkHandle(TraceRecordingSink::default())),
        start + Duration::from_millis(1008),
    )
    .expect("an unacknowledged response times out after one second");
    core.acknowledge_trace(&run_id, sequence + 2);

    let stale = core.read_traces(
        TraceRequest {
            run_id: StableId::parse("00000000-0000-4000-8000-000000000099").unwrap(),
            layout_version: 1,
            signal_keys: vec![format!("{RESISTOR_1}:voltage")],
            query: TraceQuery::History {
                start_step: 0,
                end_step: 5,
            },
            max_points: 32,
        },
        Box::new(TraceSinkHandle(TraceRecordingSink::default())),
        start + Duration::from_millis(8),
    );
    assert_eq!(stale.unwrap_err()[0].code, DiagnosticCode::StaleRun);
}

#[test]
fn trace_acknowledgement_timeout_starts_after_the_response_is_sent() {
    let ack_timeout = Duration::from_millis(100);
    let (mut core, run_id) = compiled_core(SimulationConfig {
        ack_timeout,
        ..fake_clock()
    });
    let start = Instant::now();
    let request = TraceRequest {
        run_id,
        layout_version: 1,
        signal_keys: vec![format!("{RESISTOR_1}:voltage")],
        query: TraceQuery::History {
            start_step: 0,
            end_step: 0,
        },
        max_points: 8,
    };

    core.read_traces(
        request.clone(),
        Box::new(DelayedTraceSink(Duration::from_millis(130))),
        start,
    )
    .expect("first trace response is sent");

    let sent_recently = core.read_traces(
        request.clone(),
        Box::new(TraceSinkHandle(TraceRecordingSink::default())),
        start + Duration::from_millis(150),
    );
    assert_eq!(
        sent_recently.unwrap_err()[0].code,
        DiagnosticCode::InvalidParameter
    );

    core.read_traces(
        request,
        Box::new(TraceSinkHandle(TraceRecordingSink::default())),
        start + Duration::from_millis(1_000),
    )
    .expect("a sent response is released after its acknowledgement timeout");
}

#[test]
fn a_catch_up_batch_yields_after_its_budget_between_complete_steps_and_drops_the_debt() {
    // With no budget the batch stops after one complete step, however late the tick is, and the
    // wall-clock debt is dropped instead of being simulated at the next tick.
    let (mut core, run_id) = compiled_core(SimulationConfig {
        catch_up_budget: Duration::ZERO,
        frame_interval: Duration::from_millis(1),
        ..SimulationConfig::default()
    });
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id, Box::new(SinkHandle(sink.clone())), start)
        .expect("start");

    core.tick(start + Duration::from_millis(10));
    let first = sink
        .decoded()
        .last()
        .expect("a frame")
        .metadata
        .simulation_step;
    assert_eq!(first, 1, "one complete step, not the ten that were due");

    core.acknowledge(sink.decoded().last().unwrap().metadata.frame_sequence);
    core.tick(start + Duration::from_millis(11));
    let second = sink
        .decoded()
        .last()
        .expect("a frame")
        .metadata
        .simulation_step;
    assert_eq!(
        second, 2,
        "the debt was dropped: one more step is due, not ten"
    );
}

#[test]
fn playback_uses_fixed_steps_without_replaying_paused_time() {
    let config = SimulationConfig {
        frame_interval: PHYSICS_STEP,
        ..fake_clock()
    };
    let (mut core, run) = compiled_core(config);
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run.clone(), Box::new(SinkHandle(sink.clone())), start)
        .unwrap();
    let mut now = start + Duration::from_millis(10);
    core.tick(now);
    assert_eq!(sink.decoded()[0].metadata.simulation_step, 10);

    let refused = core
        .playback(&run, "rev-1", PlaybackCommand::Step, now)
        .unwrap_err();
    assert_eq!(refused[0].code, DiagnosticCode::InvalidParameter);
    let stale = core
        .playback(&run, "old", PlaybackCommand::Pause, now)
        .unwrap_err();
    assert_eq!(stale[0].code, DiagnosticCode::StaleDocumentRevision);
    let stale = core
        .playback(
            &StableId::parse(RESISTOR_1).unwrap(),
            "rev-1",
            PlaybackCommand::Pause,
            now,
        )
        .unwrap_err();
    assert_eq!(stale[0].code, DiagnosticCode::StaleRun);
    core.playback(&run, "rev-1", PlaybackCommand::Pause, now)
        .unwrap();
    assert!(core.next_deadline().is_none());
    now += Duration::from_secs(10);
    core.playback(&run, "rev-1", PlaybackCommand::Pause, now)
        .unwrap();
    core.tick(now);
    assert_eq!(sink.count(), 1);

    core.playback(&run, "rev-1", PlaybackCommand::Step, now)
        .unwrap();
    now += PHYSICS_STEP;
    core.tick(now);
    let stepped = sink.decoded().pop().unwrap();
    assert_eq!(stepped.metadata.simulation_step, 11);
    assert!((stepped.metadata.simulation_time_seconds - 0.011).abs() < 1e-12);
    core.acknowledge(stepped.metadata.frame_sequence);

    core.playback(&run, "rev-1", PlaybackCommand::QuarterSpeed, now)
        .unwrap();
    now += Duration::from_secs(10);
    core.playback(&run, "rev-1", PlaybackCommand::Play, now)
        .unwrap();
    now += Duration::from_millis(3);
    core.tick(now);
    assert_eq!(sink.count(), 2);
    core.playback(&run, "rev-1", PlaybackCommand::Play, now)
        .unwrap();
    now += Duration::from_millis(1);
    core.tick(now);
    assert_eq!(sink.decoded().last().unwrap().metadata.simulation_step, 12);
    core.acknowledge(sink.decoded().last().unwrap().metadata.frame_sequence);

    core.playback(&run, "rev-1", PlaybackCommand::DoubleSpeed, now)
        .unwrap();
    now += Duration::from_millis(2);
    core.tick(now);
    let last = sink.decoded().pop().unwrap();
    assert_eq!(last.metadata.simulation_step, 16);
    assert!((last.metadata.simulation_time_seconds - 0.016).abs() < 1e-12);
    assert!(last.metadata.frame_sequence > stepped.metadata.frame_sequence);
    core.acknowledge(last.metadata.frame_sequence);

    core.playback(&run, "rev-1", PlaybackCommand::HalfSpeed, now)
        .unwrap();
    now += Duration::from_millis(6);
    core.tick(now);
    let half_speed = sink.decoded().pop().unwrap();
    assert_eq!(half_speed.metadata.simulation_step, 19);
    core.acknowledge(half_speed.metadata.frame_sequence);

    core.playback(&run, "rev-1", PlaybackCommand::NormalSpeed, now)
        .unwrap();
    now += Duration::from_millis(3);
    core.tick(now);
    assert_eq!(sink.decoded().last().unwrap().metadata.simulation_step, 22);
}

#[test]
fn paused_operator_updates_replace_the_latest_trace_sample_before_stepping() {
    const SWITCH: &str = "00000000-0000-4000-8000-000000000002";
    const LOAD: &str = "00000000-0000-4000-8000-000000000003";
    let mut core = SimulationCore::new(fake_clock());
    let mut input: CompileRequest = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-6-drawn-switch.request.json"
    ))
    .unwrap();
    input.document_revision = "rev-1".into();
    let CompileResult::Success { run_id, .. } = core.compile(input) else {
        panic!("switch fixture must compile");
    };
    let sink = RecordingSink::default();
    let start = Instant::now();
    core.start(run_id.clone(), Box::new(SinkHandle(sink.clone())), start)
        .unwrap();
    let now = start + Duration::from_millis(20);
    core.tick(now);
    let old = sink.decoded().pop().unwrap();
    core.acknowledge(old.metadata.frame_sequence);
    core.playback(&run_id, "rev-1", PlaybackCommand::Pause, now)
        .unwrap();
    let SetOperatorStateResult::Success { snapshot, .. } =
        core.set_operator_state(SetOperatorStateRequest {
            run_id: run_id.clone(),
            component_id: StableId::parse(SWITCH).unwrap(),
            position: OperatorPosition::Closed,
            expected_document_revision: "rev-1".into(),
            document_revision: "rev-2".into(),
        })
    else {
        panic!("the switch must close while paused");
    };
    assert_eq!(
        snapshot.metadata.simulation_step,
        old.metadata.simulation_step
    );
    let now = now + Duration::from_secs(1);
    core.tick(now);
    let updated = sink.decoded().pop().unwrap();
    assert_eq!(updated.metadata, snapshot.metadata);
    assert!(updated.metadata.frame_sequence > old.metadata.frame_sequence);
    core.acknowledge(updated.metadata.frame_sequence);

    core.playback(&run_id, "rev-2", PlaybackCommand::Step, now)
        .unwrap();
    let traces = TraceRecordingSink::default();
    core.read_traces(
        TraceRequest {
            run_id,
            layout_version: 1,
            signal_keys: vec![format!("{LOAD}:voltage")],
            query: TraceQuery::History {
                start_step: 20,
                end_step: 21,
            },
            max_points: 8,
        },
        Box::new(TraceSinkHandle(traces.clone())),
        now,
    )
    .unwrap();
    let responses = traces.responses.lock().unwrap();
    let response = &responses[0];
    let length = u32::from_le_bytes(response[4..8].try_into().unwrap()) as usize;
    let header: serde_json::Value = serde_json::from_slice(&response[8..8 + length]).unwrap();
    assert_eq!(header["pointCount"], 2);
    assert_eq!(header["metadata"]["firstSampleStep"], "20");
    assert_eq!(header["metadata"]["lastSampleStep"], "21");
    let payload = (8 + length).next_multiple_of(8);
    let step20_voltage =
        f64::from_le_bytes(response[payload + 8..payload + 16].try_into().unwrap());
    let step21_voltage =
        f64::from_le_bytes(response[payload + 32..payload + 40].try_into().unwrap());
    assert!((step20_voltage - 10.0 * 1000.0 / 2000.01).abs() < 1e-8);
    assert_eq!(step20_voltage, step21_voltage);
    core.tick(now + Duration::from_secs(1));
    assert_eq!(sink.decoded().last().unwrap().metadata.simulation_step, 21);
}
