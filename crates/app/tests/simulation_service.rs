//! The simulation thread, with real time. The timing rules themselves are covered with a
//! fake clock in `simulation_core.rs`; these tests check the threading and lifecycle.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::simulation::{
    FrameSink, ServiceError, SimulationConfig, SimulationService, SinkClosed, TraceSink,
};
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, ParameterId, ResultSnapshot,
    SetParameterRequest, SetParameterResult, StableId, TraceQuery, TraceRequest,
};

const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const RESISTOR_1: &str = "00000000-0000-4000-8000-000000000002";
const WAIT: Duration = Duration::from_secs(3);

struct ChannelSink(Sender<Vec<u8>>);

impl FrameSink for ChannelSink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.send(frame).map_err(|_| SinkClosed)
    }
}

struct TraceChannelSink(Sender<Vec<u8>>);

impl TraceSink for TraceChannelSink {
    fn send(&mut self, response: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.send(response).map_err(|_| SinkClosed)
    }
}

fn fast_config() -> SimulationConfig {
    SimulationConfig {
        frame_interval: Duration::from_millis(5),
        max_in_flight: 2,
        ack_timeout: Duration::from_secs(30),
        max_steps_per_tick: 1_000,
    }
}

fn request(revision: &str) -> CompileRequest {
    CompileRequest {
        schematic: serde_json::from_str(SCHEMATIC).expect("schematic fixture"),
        geometry: serde_json::from_str(GEOMETRY).expect("geometry fixture"),
        document_revision: revision.to_owned(),
    }
}

fn compile(service: &SimulationService, revision: &str) -> StableId {
    let CompileResult::Success { run_id, .. } = service
        .compile(request(revision))
        .expect("the thread answers")
    else {
        panic!("the canonical divider must compile");
    };
    run_id
}

fn start(service: &SimulationService, run_id: StableId) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    service
        .start(run_id, Box::new(ChannelSink(sender)))
        .expect("the thread answers")
        .expect("the run is active");
    receiver
}

fn next_frame(receiver: &Receiver<Vec<u8>>) -> ResultSnapshot {
    decode_frame(&receiver.recv_timeout(WAIT).expect("a frame arrives")).expect("decodes")
}

#[test]
fn trace_requests_flow_through_the_simulation_thread_and_require_acknowledgement() {
    let service = SimulationService::spawn(fast_config());
    let run_id = compile(&service, "rev-1");
    let request = TraceRequest {
        run_id: run_id.clone(),
        layout_version: 1,
        signal_keys: vec![format!("{RESISTOR_1}:voltage")],
        query: TraceQuery::History {
            start_step: 0,
            end_step: 0,
        },
        max_points: 8,
    };
    let (sender, _receiver) = mpsc::channel();
    let stale_run = StableId::parse("00000000-0000-4000-8000-000000000099").unwrap();
    let stale = service
        .read_traces(
            TraceRequest {
                run_id: stale_run,
                ..request.clone()
            },
            Box::new(TraceChannelSink(sender)),
        )
        .expect("the thread is not needed to reject a stale run");
    assert_eq!(stale.unwrap_err()[0].code, DiagnosticCode::StaleRun);

    let (sender, receiver) = mpsc::channel();
    let sequence_from_service = service
        .read_traces(request.clone(), Box::new(TraceChannelSink(sender)))
        .expect("the thread answers")
        .expect("the initial trace is available");
    let response = receiver.recv_timeout(WAIT).expect("trace response arrives");
    assert_eq!(&response[..4], b"OCT1");
    let header_length = u32::from_le_bytes(response[4..8].try_into().unwrap()) as usize;
    let header: serde_json::Value =
        serde_json::from_slice(&response[8..8 + header_length]).expect("trace header is JSON");
    let sequence = header["traceSequence"]
        .as_str()
        .expect("trace sequence is a decimal string")
        .parse::<u64>()
        .expect("trace sequence is valid");
    assert_eq!(sequence_from_service, sequence);

    service
        .acknowledge_trace(run_id.clone(), sequence + 1)
        .expect("a stale acknowledgement is harmless");

    let (sender, _receiver) = mpsc::channel();
    let pending = service
        .read_traces(request.clone(), Box::new(TraceChannelSink(sender)))
        .expect("the thread answers");
    assert_eq!(
        pending.unwrap_err()[0].code,
        DiagnosticCode::InvalidParameter
    );

    service
        .acknowledge_trace(run_id, sequence)
        .expect("trace acknowledgement is queued");
    let (sender, receiver) = mpsc::channel();
    service
        .read_traces(request, Box::new(TraceChannelSink(sender)))
        .expect("the thread answers")
        .expect("acknowledgement permits another trace");
    assert_eq!(
        &receiver
            .recv_timeout(WAIT)
            .expect("second response arrives")[..4],
        b"OCT1"
    );
    service.shutdown();
}

#[test]
fn frames_flow_from_the_simulation_thread_without_the_caller_doing_anything() {
    let service = SimulationService::spawn(fast_config());
    let receiver = start(&service, compile(&service, "rev-1"));

    let mut previous: Option<ResultSnapshot> = None;

    for _ in 0..4 {
        let frame = next_frame(&receiver);
        service
            .acknowledge(frame.metadata.frame_sequence)
            .expect("acknowledge");

        if let Some(previous) = &previous {
            assert!(frame.metadata.frame_sequence > previous.metadata.frame_sequence);
            assert!(frame.metadata.simulation_step > previous.metadata.simulation_step);
        }
        previous = Some(frame);
    }

    service.shutdown();
}

#[test]
fn requests_are_answered_while_frames_are_streaming() {
    let service = SimulationService::spawn(fast_config());
    let run_id = compile(&service, "rev-1");
    let receiver = start(&service, run_id.clone());
    let _ = next_frame(&receiver);

    let result = service
        .set_parameter(SetParameterRequest {
            run_id,
            expected_document_revision: "rev-1".to_owned(),
            document_revision: "rev-2".to_owned(),
            component_id: StableId::parse(RESISTOR_1).expect("valid ID"),
            parameter_id: ParameterId::ResistanceOhms,
            value: 3000.0,
        })
        .expect("the thread answers");
    assert!(matches!(result, SetParameterResult::Success { .. }));

    // Acknowledge as we go; a frame carrying the new revision must arrive.
    let mut saw_new_revision = false;
    for _ in 0..40 {
        let frame = next_frame(&receiver);
        service
            .acknowledge(frame.metadata.frame_sequence)
            .expect("acknowledge");

        if frame.metadata.document_revision == "rev-2" {
            saw_new_revision = true;
            break;
        }
    }
    assert!(saw_new_revision);

    service.shutdown();
}

#[test]
fn an_unacknowledged_consumer_receives_only_the_allowed_number_of_frames() {
    let config = fast_config();
    let allowed = config.max_in_flight;
    let service = SimulationService::spawn(config);
    let receiver = start(&service, compile(&service, "rev-1"));

    let mut received = 0;
    while receiver.recv_timeout(Duration::from_millis(400)).is_ok() {
        received += 1;
    }

    assert_eq!(received, allowed);
    service.shutdown();
}

#[test]
fn stopping_releases_the_receiver() {
    let service = SimulationService::spawn(fast_config());
    let receiver = start(&service, compile(&service, "rev-1"));
    let _ = next_frame(&receiver);

    service.stop(None).expect("stop");

    // Frames already queued may still be there; after them the channel is closed.
    loop {
        match receiver.recv_timeout(WAIT) {
            Ok(_) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("the sink was not released after stop"),
        }
    }

    service.shutdown();
}

#[test]
fn a_new_compile_releases_the_previous_stream() {
    let service = SimulationService::spawn(fast_config());
    let receiver = start(&service, compile(&service, "rev-1"));
    let _ = next_frame(&receiver);

    let _ = compile(&service, "rev-2");

    loop {
        match receiver.recv_timeout(WAIT) {
            Ok(_) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("the old stream was not released"),
        }
    }

    service.shutdown();
}

#[test]
fn starting_an_unknown_run_is_refused_with_a_diagnostic() {
    let service = SimulationService::spawn(fast_config());
    let (sender, _receiver) = mpsc::channel();
    let unknown = StableId::parse("00000000-0000-4000-8000-0000000000ff").expect("valid ID");

    let error = service
        .start(unknown, Box::new(ChannelSink(sender)))
        .expect("the thread answers")
        .expect_err("an unknown run cannot start");

    assert_eq!(error[0].code, DiagnosticCode::StaleRun);
    service.shutdown();
}

#[test]
fn shutdown_joins_the_thread_and_later_calls_fail_cleanly() {
    let service = SimulationService::spawn(fast_config());
    let receiver = start(&service, compile(&service, "rev-1"));
    let _ = next_frame(&receiver);

    service.shutdown();
    service.shutdown();

    assert_eq!(service.stop(None), Err(ServiceError::Stopped));
    assert_eq!(service.acknowledge(1), Err(ServiceError::Stopped));
    assert!(matches!(
        service.compile(request("rev-3")),
        Err(ServiceError::Stopped)
    ));

    // The worker dropped its sink, so the receiver is closed.
    loop {
        match receiver.recv_timeout(WAIT) {
            Ok(_) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("shutdown left the sink open"),
        }
    }
}

#[test]
fn dropping_the_service_stops_the_thread() {
    let receiver;
    {
        let service = SimulationService::spawn(fast_config());
        receiver = start(&service, compile(&service, "rev-1"));
        let _ = next_frame(&receiver);
    }

    loop {
        match receiver.recv_timeout(WAIT) {
            Ok(_) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => panic!("dropping the service left the sink open"),
        }
    }
}
