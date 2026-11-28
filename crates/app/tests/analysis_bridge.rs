//! The analysis bridge with the real Engine (docs/contracts/stage-17-extra-analyses.md section 5):
//! a DC sweep, a frequency response and a harmonic spectrum run as jobs through `AnalysisService`,
//! as JSON requests the way the Tauri commands receive them, while a simulation keeps running.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use open_circuit_app_lib::analysis::AnalysisService;
use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::simulation::{
    FrameSink, SimulationConfig, SimulationService, SinkClosed,
};
use open_circuit_engine::engine::analyses::{
    AnalysisFailureReason, AnalysisJobReport, AnalysisOutcome, AnalysisRequest, AnalysisResult,
    AnalysisStart, MAX_ANALYSIS_PAYLOAD_BYTES,
};
use open_circuit_engine::{CompileRequest, CompileResult, ResultLayout, StableId};
use serde_json::{Value, json};

const DIVIDER: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const DIVIDER_GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const RC: &str = include_str!("../../../tests/golden/stage-3-rc-charge.schematic.json");
const RC_GEOMETRY: &str = include_str!("../../../tests/golden/stage-3-rc-charge.geometry.json");
const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const R2: &str = "00000000-0000-4000-8000-000000000003";
const WINDOW: &str = "main";
const WAIT: Duration = Duration::from_secs(10);

struct ChannelSink(mpsc::Sender<Vec<u8>>);

impl FrameSink for ChannelSink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.send(frame).map_err(|_| SinkClosed)
    }
}

fn config() -> SimulationConfig {
    SimulationConfig {
        frame_interval: Duration::from_millis(5),
        max_in_flight: 4,
        ack_timeout: Duration::from_secs(30),
        ..SimulationConfig::default()
    }
}

fn setup() -> (Arc<SimulationService>, AnalysisService) {
    let simulation = Arc::new(SimulationService::spawn(config()));
    let analysis = AnalysisService::new(Arc::clone(&simulation));
    (simulation, analysis)
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// A request as the interface would send it: JSON, decoded as the Tauri command decodes it.
fn request(value: Value) -> AnalysisRequest {
    serde_json::from_value(value).expect("the request decodes")
}

fn dc_sweep(revision: &str, start: f64, stop: f64, points: u32) -> Value {
    json!({
        "kind": "dcSweep",
        "schematic": json_of(DIVIDER),
        "geometry": json_of(DIVIDER_GEOMETRY),
        "documentRevision": revision,
        "sourceId": SOURCE,
        "signalKey": format!("{R2}:voltage"),
        "startVolts": start,
        "stopVolts": stop,
        "pointCount": points,
    })
}

fn started(start: AnalysisStart) -> String {
    match start {
        AnalysisStart::Started { job_id, .. } => job_id,
        other => panic!("expected a started job, got {other:?}"),
    }
}

fn finished(analysis: &AnalysisService, id: &str) -> AnalysisJobReport {
    let deadline = Instant::now() + WAIT;
    loop {
        let report = analysis.read(WINDOW, id).expect("the job is there");
        if !matches!(
            report.outcome,
            AnalysisOutcome::Pending | AnalysisOutcome::Running
        ) {
            return report;
        }
        assert!(Instant::now() < deadline, "the job did not finish");
        thread::sleep(Duration::from_millis(2));
    }
}

fn compile_divider(simulation: &SimulationService, revision: &str) -> (StableId, ResultLayout) {
    let CompileResult::Success { run_id, layout, .. } = simulation
        .compile(CompileRequest {
            schematic: serde_json::from_str(DIVIDER).unwrap(),
            geometry: serde_json::from_str(DIVIDER_GEOMETRY).unwrap(),
            document_revision: revision.to_owned(),
            faults: None,
        })
        .expect("the thread answers")
    else {
        panic!("the divider compiles");
    };
    (run_id, layout)
}

fn run_stream(simulation: &SimulationService, run_id: StableId) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    simulation
        .start(run_id, Box::new(ChannelSink(sender)))
        .expect("the thread answers")
        .expect("the run is active");
    receiver
}

/// Reads frames, acknowledging each, until the simulation has passed `step`, and returns the step
/// reached.
fn run_past(simulation: &SimulationService, frames: &Receiver<Vec<u8>>, step: u64) -> u64 {
    let deadline = Instant::now() + WAIT;
    loop {
        let frame = decode_frame(&frames.recv_timeout(WAIT).expect("a frame arrives")).unwrap();
        simulation
            .acknowledge(frame.metadata.frame_sequence)
            .expect("the thread answers");
        if frame.metadata.simulation_step > step {
            return frame.metadata.simulation_step;
        }
        assert!(Instant::now() < deadline, "the run did not advance");
    }
}

#[test]
fn a_dc_sweep_job_returns_ordered_points_that_match_the_closed_form() {
    let (_simulation, analysis) = setup();
    let id = started(analysis.start(WINDOW, request(dc_sweep("divider-1", -5.0, 5.0, 11))));

    let report = finished(&analysis, &id);

    assert_eq!(report.document_revision, "divider-1");
    let AnalysisOutcome::Succeeded {
        result:
            AnalysisResult::DcSweep {
                document_revision,
                points,
                ..
            },
    } = report.outcome
    else {
        panic!("the sweep succeeds: {:?}", report.outcome);
    };
    assert_eq!(document_revision, "divider-1");
    assert_eq!(points.len(), 11);
    // The source has 0.1 ohm of internal resistance and the two resistors are 1 kilohm each.
    for point in &points {
        let expected = point.source_volts * 1000.0 / (2000.0 + 0.1);
        assert!(
            (point.reading - expected).abs() <= 1e-8 + 1e-4 * expected.abs(),
            "{} V gives {} V, expected {expected} V",
            point.source_volts,
            point.reading
        );
    }
    // The supplied endpoints are exact and the order is the order asked for.
    assert_eq!(points[0].source_volts, -5.0);
    assert_eq!(points[10].source_volts, 5.0);
    assert!(
        points
            .windows(2)
            .all(|pair| pair[0].source_volts < pair[1].source_volts)
    );
}

#[test]
fn the_result_crosses_as_json_with_a_job_id_string_and_a_named_kind() {
    let (_simulation, analysis) = setup();
    let start = analysis.start(WINDOW, request(dc_sweep("divider-1", 0.0, 1.0, 2)));
    let started_json = serde_json::to_value(&start).unwrap();
    assert_eq!(started_json["status"], "started");
    assert_eq!(started_json["analysis"], "dcSweep");
    let id = started_json["jobId"].as_str().expect("a decimal string");
    assert!(id.parse::<u64>().is_ok());

    let report = serde_json::to_value(finished(&analysis, id)).unwrap();

    assert_eq!(report["jobId"], id);
    assert_eq!(report["analysis"], "dcSweep");
    assert_eq!(report["outcome"]["status"], "succeeded");
    assert_eq!(report["outcome"]["result"]["kind"], "dcSweep");
    assert_eq!(report["outcome"]["result"]["documentRevision"], "divider-1");
    assert_eq!(report["outcome"]["result"]["points"][1]["sourceVolts"], 1.0);
}

#[test]
fn a_frequency_response_job_matches_the_first_order_transfer_function() {
    let (_simulation, analysis) = setup();
    let capacitor = "00000000-0000-4000-8000-000000000003";
    let id = started(analysis.start(
        WINDOW,
        request(json!({
            "kind": "frequencyResponse",
            "schematic": json_of(RC),
            "geometry": json_of(RC_GEOMETRY),
            "documentRevision": "rc-1",
            "sourceId": SOURCE,
            "output": [
                {"componentId": capacitor, "pinId": "pin1"},
                {"componentId": capacitor, "pinId": "pin2"},
            ],
            "startHz": 1.0,
            "stopHz": 10000.0,
            "pointCount": 9,
            "spacing": "logarithmic",
        })),
    ));

    let AnalysisOutcome::Succeeded {
        result:
            AnalysisResult::FrequencyResponse {
                points,
                document_revision,
                ..
            },
    } = finished(&analysis, &id).outcome
    else {
        panic!("the response succeeds");
    };

    assert_eq!(document_revision, "rc-1");
    assert_eq!(points.len(), 9);
    assert_eq!(points[0].frequency_hz, 1.0);
    assert_eq!(points[8].frequency_hz, 10000.0);
    // R = 1 kilohm, C = 10 microfarads, an ideal source: H = 1 / (1 + j w R C).
    for point in &points {
        let w_rc = std::f64::consts::TAU * point.frequency_hz * 1000.0 * 10e-6;
        let magnitude = 1.0 / (1.0 + w_rc * w_rc).sqrt();
        let phase = -w_rc.atan().to_degrees();
        assert!(
            (point.magnitude - magnitude).abs() <= 1e-8 + 1e-5 * magnitude,
            "{} Hz",
            point.frequency_hz
        );
        assert!(
            (point.phase_degrees.unwrap() - phase).abs() <= 0.01,
            "{} Hz",
            point.frequency_hz
        );
    }
}

#[test]
fn a_spectrum_job_reads_the_history_of_a_running_simulation_and_leaves_it_running() {
    let (simulation, analysis) = setup();
    let (run_id, layout) = compile_divider(&simulation, "rev-1");
    let frames = run_stream(&simulation, run_id.clone());
    let reached = run_past(&simulation, &frames, 200);
    let key = format!("{R2}:voltage");
    let end_step = 127_u64;
    assert!(reached > end_step);

    let id = started(analysis.start(
        WINDOW,
        request(json!({
            "kind": "spectrum",
            "runId": run_id,
            "layoutVersion": layout.layout_version,
            "documentRevision": "rev-1",
            "signalKey": key,
            "endStep": end_step.to_string(),
            "sampleCount": 64,
        })),
    ));
    let before = run_past(&simulation, &frames, reached);
    let report = finished(&analysis, &id);
    let after = run_past(&simulation, &frames, before);

    let AnalysisOutcome::Succeeded {
        result:
            AnalysisResult::Spectrum {
                bins,
                first_step,
                end_step: reported_end,
                bin_spacing_hz,
                run_id: reported_run,
                ..
            },
    } = report.outcome
    else {
        panic!("the spectrum succeeds: {:?}", report.outcome);
    };
    assert_eq!(reported_run, run_id);
    assert_eq!(first_step, "64");
    assert_eq!(reported_end, "127");
    assert_eq!(bins.len(), 33);
    assert_eq!(bin_spacing_hz, 1000.0 / 64.0);
    // A steady 2.4999 V: the DC bin holds it, and the Hann window spreads it into bin 1 only.
    let steady = 5.0 * 1000.0 / 2000.1;
    assert!((bins[0].peak_amplitude - steady).abs() <= 1e-6);
    assert!(bins[2..].iter().all(|bin| bin.peak_amplitude < 1e-6));
    // The live simulation was not stopped or slowed by the job: it went on past it.
    assert!(after > before);
}

#[test]
fn a_dc_sweep_does_not_disturb_a_running_simulation() {
    let (simulation, analysis) = setup();
    let (run_id, _layout) = compile_divider(&simulation, "rev-1");
    let frames = run_stream(&simulation, run_id);
    let before = run_past(&simulation, &frames, 20);

    let id = started(analysis.start(WINDOW, request(dc_sweep("rev-1", -1000.0, 1000.0, 257))));
    let during = run_past(&simulation, &frames, before);
    finished(&analysis, &id);
    let after = run_past(&simulation, &frames, during);

    assert!(before < during && during < after);
}

#[test]
fn a_stale_revision_run_or_layout_is_rejected_and_a_missing_history_is_unavailable() {
    let (simulation, analysis) = setup();
    let (run_id, layout) = compile_divider(&simulation, "rev-1");
    let frames = run_stream(&simulation, run_id.clone());
    run_past(&simulation, &frames, 150);
    let key = format!("{R2}:voltage");
    let spectrum = |revision: &str, layout_version: u32, run: &StableId, end: &str| {
        request(json!({
            "kind": "spectrum", "runId": run, "layoutVersion": layout_version,
            "documentRevision": revision, "signalKey": key,
            "endStep": end, "sampleCount": 64,
        }))
    };
    let failure_of = |analysis_request: AnalysisRequest| {
        let id = started(analysis.start(WINDOW, analysis_request));
        match finished(&analysis, &id).outcome {
            AnalysisOutcome::Failed { failure } => failure,
            other => panic!("expected a failure, got {other:?}"),
        }
    };

    // A drawing that is no longer the one the run was compiled from.
    assert_eq!(
        failure_of(spectrum("rev-2", layout.layout_version, &run_id, "127")).reason,
        AnalysisFailureReason::StaleInput
    );
    // A run that is not the active one.
    let other_run = StableId::parse("00000000-0000-4000-8000-0000000000aa").unwrap();
    assert_eq!(
        failure_of(spectrum("rev-1", layout.layout_version, &other_run, "127")).reason,
        AnalysisFailureReason::StaleInput
    );
    // A layout version the run does not have.
    assert_eq!(
        failure_of(spectrum("rev-1", layout.layout_version + 1, &run_id, "127")).reason,
        AnalysisFailureReason::StaleInput
    );
    // Steps the history does not hold: not yet reached, and before the first step.
    assert_eq!(
        failure_of(spectrum("rev-1", layout.layout_version, &run_id, "9999999")).reason,
        AnalysisFailureReason::UnavailableHistory
    );
    assert_eq!(
        failure_of(spectrum("rev-1", layout.layout_version, &run_id, "10")).reason,
        AnalysisFailureReason::UnavailableHistory
    );
}

#[test]
fn a_bad_request_fails_with_a_reason_and_nothing_runs_on_the_simulation_thread() {
    let (_simulation, analysis) = setup();
    let failure_of = |value: Value| {
        let id = started(analysis.start(WINDOW, request(value)));
        match finished(&analysis, &id).outcome {
            AnalysisOutcome::Failed { failure } => failure,
            other => panic!("expected a failure, got {other:?}"),
        }
    };

    // One point is not a sweep.
    assert_eq!(
        failure_of(dc_sweep("rev", 0.0, 1.0, 1)).reason,
        AnalysisFailureReason::InvalidRequest
    );
    // A step that is not plain decimal digits.
    let mut spectrum = json!({
        "kind": "spectrum", "runId": "00000000-0000-4000-8000-0000000000aa",
        "layoutVersion": 1, "documentRevision": "rev", "signalKey": "x:voltage",
        "endStep": "-1", "sampleCount": 64,
    });
    assert_eq!(
        failure_of(spectrum.clone()).reason,
        AnalysisFailureReason::InvalidRequest
    );
    spectrum["endStep"] = json!("1e3");
    assert_eq!(
        failure_of(spectrum).reason,
        AnalysisFailureReason::InvalidRequest
    );
    // A signal that is not an instantaneous voltage or current of the run.
    let (simulation, analysis) = setup();
    let (run_id, layout) = compile_divider(&simulation, "rev-1");
    let frames = run_stream(&simulation, run_id.clone());
    run_past(&simulation, &frames, 150);
    let id = started(analysis.start(
        WINDOW,
        request(json!({
            "kind": "spectrum", "runId": run_id, "layoutVersion": layout.layout_version,
            "documentRevision": "rev-1", "signalKey": "not-a-signal",
            "endStep": "127", "sampleCount": 64,
        })),
    ));
    let AnalysisOutcome::Failed { failure } = finished(&analysis, &id).outcome else {
        panic!("an unknown signal is refused");
    };
    assert_eq!(failure.reason, AnalysisFailureReason::UnsupportedSignal);
}

#[test]
fn an_unsupported_circuit_names_the_components() {
    let (_simulation, analysis) = setup();
    let mut schematic = json_of(DIVIDER);
    // A lamp is not one of the devices a DC sweep supports.
    schematic["components"][1]["typeId"] = json!("lamp");
    schematic["components"][1]["parameters"] = json!({"resistanceOhms": 100});
    let mut geometry = json_of(DIVIDER_GEOMETRY);
    geometry["components"][1]["pins"][0]["pinId"] = json!("p");
    geometry["components"][1]["pins"][1]["pinId"] = json!("n");
    let mut value = dc_sweep("rev", 0.0, 1.0, 3);
    value["schematic"] = schematic;
    value["geometry"] = geometry;
    let id = started(analysis.start(WINDOW, request(value)));

    let AnalysisOutcome::Failed { failure } = finished(&analysis, &id).outcome else {
        panic!("a lamp is refused");
    };

    assert_eq!(failure.reason, AnalysisFailureReason::UnsupportedCircuit);
    assert_eq!(
        failure.component_ids,
        [StableId::parse("00000000-0000-4000-8000-000000000002").unwrap()]
    );
}

#[test]
fn a_request_over_the_bound_is_refused_before_anything_runs() {
    let (_simulation, analysis) = setup();
    let mut value = dc_sweep("rev", 0.0, 1.0, 3);
    value["documentRevision"] = json!("x".repeat(MAX_ANALYSIS_PAYLOAD_BYTES + 1));

    let start = analysis.start(WINDOW, request(value));

    let AnalysisStart::Rejected { failure } = start else {
        panic!("the request is too large");
    };
    assert_eq!(failure.reason, AnalysisFailureReason::PayloadTooLarge);
    // Nothing was started: the window has no job, so another start is not busy.
    started(analysis.start(WINDOW, request(dc_sweep("rev", 0.0, 1.0, 2))));
}

#[test]
fn requests_decode_strictly() {
    let mut value = dc_sweep("rev", 0.0, 1.0, 3);
    value["extra"] = json!(true);
    assert!(serde_json::from_value::<AnalysisRequest>(value).is_err());

    let mut value = dc_sweep("rev", 0.0, 1.0, 3);
    value["kind"] = json!("noise");
    assert!(serde_json::from_value::<AnalysisRequest>(value).is_err());

    // A step as a bare number is not the decimal string the transport uses.
    assert!(
        serde_json::from_value::<AnalysisRequest>(json!({
            "kind": "spectrum", "runId": "00000000-0000-4000-8000-0000000000aa",
            "layoutVersion": 1, "documentRevision": "rev", "signalKey": "k",
            "endStep": 127, "sampleCount": 64,
        }))
        .is_err()
    );
}

#[test]
fn invalidating_ends_the_jobs_of_a_replaced_run_and_forgets_their_results() {
    let (_simulation, analysis) = setup();
    let id = started(analysis.start(WINDOW, request(dc_sweep("rev-1", 0.0, 1.0, 2))));
    finished(&analysis, &id);

    analysis.invalidate();

    assert!(analysis.read(WINDOW, &id).is_none());
}
