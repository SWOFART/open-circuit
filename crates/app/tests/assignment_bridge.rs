//! The assignment bridge (docs/contracts/stage-5-assignments.md sections 2.5 and 6): file
//! round trips and refusals on disk, the check and faulted-compile commands against the real
//! Engine with the canonical fixtures, and the guarantee that a check or a fault never
//! disturbs the live run.

use std::fs;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use open_circuit_app_lib::assignment::{
    AssignmentSession, normalize_save_path, read_assignment_file, write_assignment_file,
};
use open_circuit_app_lib::checks::CheckService;
use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::project::write_project_file;
use open_circuit_app_lib::simulation::{
    FrameSink, SimulationConfig, SimulationService, SinkClosed,
};
use open_circuit_engine::{
    Assignment, AssignmentDecoded, AssignmentErrorCode, AssignmentSaveOutcome, CheckOutcome,
    CheckRequest, CheckResult, CheckVerdict, CompileRequest, CompileResult, CriterionResult,
    DiagnosisChecked, DiagnosticCode, GeometryEnvelope, ParameterId, ResultSnapshot,
    SetParameterRequest, SetParameterResult, StableId, check_diagnosis, decode_assignment_json,
};
use serde_json::{Value, json};

const DIVIDER: &str = include_str!("../../../tests/golden/stage-5-divider-build.ocassignment");
const DIVIDER_GEOMETRY: &str =
    include_str!("../../../tests/golden/stage-5-divider-build.reference.geometry.json");
const FUSE: &str = include_str!("../../../tests/golden/stage-5-fuse-state.ocassignment");
const FUSE_GEOMETRY: &str =
    include_str!("../../../tests/golden/stage-5-fuse-state.reference.geometry.json");
const FAULT: &str = include_str!("../../../tests/golden/stage-5-fault-finding.ocassignment");
const FAULT_GEOMETRY: &str =
    include_str!("../../../tests/golden/stage-5-fault-finding.geometry.json");
const FAULT_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-5-fault-finding.expected.json");

const V1: &str = "00000000-0000-4000-8000-000000000001";
const R_TOP: &str = "00000000-0000-4000-8000-000000000011";
const R_BOTTOM: &str = "00000000-0000-4000-8000-000000000012";
const R_MIDDLE: &str = "00000000-0000-4000-8000-000000000013";
const FAULT_METER: &str = "00000000-0000-4000-8000-000000000004";
const WAIT: Duration = Duration::from_secs(3);

fn assignment(text: &str) -> Assignment {
    decode_assignment_json(text.as_bytes()).expect("the canonical assignment is valid")
}

fn temporary() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary directory should be created")
}

// --- files ---------------------------------------------------------------------------

#[test]
fn an_assignment_saves_and_opens_without_any_computed_state() {
    let directory = temporary();
    for (name, text) in [("divider", DIVIDER), ("fuse", FUSE), ("fault", FAULT)] {
        let path = directory.path().join(format!("{name}.ocassignment"));
        let expected = assignment(text);
        let session = AssignmentSession::default();

        assert_eq!(
            session.save_to(&path, &expected).unwrap(),
            AssignmentSaveOutcome::Saved
        );
        let written = fs::read_to_string(&path).unwrap();
        assert!(written.ends_with('\n'));
        for computed in [
            "snapshot", "layout", "runId", "trace", "history", "measured",
        ] {
            assert!(!written.contains(computed), "{name} stores {computed}");
        }
        assert_eq!(session.current_path().unwrap(), Some(path.clone()));

        let AssignmentDecoded::Valid { assignment: opened } = read_assignment_file(&path).unwrap()
        else {
            panic!("{name} should open");
        };
        assert_eq!(*opened, expected, "{name}");
    }
}

#[test]
fn a_save_without_an_extension_adds_the_native_one_and_no_other_extension_is_accepted() {
    let directory = temporary();
    let bare = directory.path().join("task");
    let saved = normalize_save_path(&bare).unwrap();
    assert_eq!(saved, directory.path().join("task.ocassignment"));

    let session = AssignmentSession::default();
    session.save_to(&bare, &assignment(DIVIDER)).unwrap();
    assert!(saved.is_file());
    assert_eq!(session.current_path().unwrap(), Some(saved));

    // A save may not overwrite a project or any other kind of file.
    for name in ["task.ocircuit", "task.json", "task.ocassignment.json"] {
        let other = directory.path().join(name);
        assert!(write_assignment_file(&other, &assignment(DIVIDER)).is_err());
        assert!(!other.exists(), "{name}");
        assert!(read_assignment_file(&other).is_err());
    }
}

#[test]
fn malformed_unsupported_unknown_and_inconsistent_files_are_refused_with_their_errors() {
    let directory = temporary();
    let path = directory.path().join("refused.ocassignment");
    let valid: Value = serde_json::from_str(DIVIDER).unwrap();
    let edited = |edit: &dyn Fn(&mut Value)| {
        let mut document = valid.clone();
        edit(&mut document);
        document.to_string()
    };
    let cases: Vec<(&str, String, AssignmentErrorCode, &str)> = vec![
        (
            "not JSON",
            "{not json".to_owned(),
            AssignmentErrorCode::Malformed,
            "",
        ),
        ("empty", String::new(), AssignmentErrorCode::Malformed, ""),
        (
            "version 2",
            edited(&|document| document["schemaVersion"] = json!(2)),
            AssignmentErrorCode::UnsupportedVersion,
            "/schemaVersion",
        ),
        (
            "an unknown field",
            edited(&|document| document["grade"] = json!(10)),
            AssignmentErrorCode::UnknownField,
            "/grade",
        ),
        (
            "a duplicate criterion ID",
            edited(&|document| {
                let first = document["checks"][0]["id"].clone();
                document["checks"][1]["id"] = first;
            }),
            AssignmentErrorCode::DuplicateId,
            "/checks/1/id",
        ),
        (
            "a criterion that names a missing part",
            edited(&|document| {
                document["checks"][0]["signal"]["key"] =
                    json!("00000000-0000-4000-8000-0000000000ee:voltage");
            }),
            AssignmentErrorCode::UnknownReference,
            "/checks/0/signal/key",
        ),
        (
            "faults in a build task",
            edited(&|document| document["faults"] = json!([])),
            AssignmentErrorCode::TaskMismatch,
            "/faults",
        ),
    ];
    for (name, contents, code, pointer) in cases {
        fs::write(&path, contents).unwrap();
        let AssignmentDecoded::Invalid { errors } = read_assignment_file(&path).unwrap() else {
            panic!("{name} should be refused");
        };
        assert_eq!(errors.len(), 1, "{name}: {errors:?}");
        assert_eq!(
            (errors[0].code, errors[0].path.as_str()),
            (code, pointer),
            "{name}"
        );
    }
}

#[test]
fn an_oversized_file_is_refused_without_being_decoded() {
    let directory = temporary();
    let path = directory.path().join("huge.ocassignment");
    let limit = open_circuit_engine::MAX_ASSIGNMENT_BYTES;

    for length in [limit + 1, limit + 4096] {
        fs::write(&path, vec![b' '; length]).unwrap();
        let AssignmentDecoded::Invalid { errors } = read_assignment_file(&path).unwrap() else {
            panic!("a file of {length} bytes should be refused");
        };
        assert_eq!(
            (errors[0].code, errors[0].path.as_str()),
            (AssignmentErrorCode::TooLarge, "")
        );
    }
    // Exactly at the limit is read (and is refused only for what it holds).
    let padded = format!("{DIVIDER}{}", " ".repeat(limit - DIVIDER.len()));
    fs::write(&path, padded).unwrap();
    assert!(matches!(
        read_assignment_file(&path).unwrap(),
        AssignmentDecoded::Valid { .. }
    ));
}

#[test]
fn a_refused_open_or_a_failed_save_leaves_the_previous_file_and_path_intact() {
    let directory = temporary();
    let existing = directory.path().join("existing.ocassignment");
    let refused = directory.path().join("refused.ocassignment");
    let session = AssignmentSession::default();
    session.save_to(&existing, &assignment(DIVIDER)).unwrap();
    let before = fs::read(&existing).unwrap();
    fs::write(&refused, "{}").unwrap();

    // A refused open does not move the active path.
    assert!(matches!(
        session.load(&refused).unwrap(),
        AssignmentDecoded::Invalid { .. }
    ));
    assert_eq!(session.current_path().unwrap(), Some(existing.clone()));

    // An invalid assignment is not written, and the file it would have replaced is intact.
    let mut untitled = assignment(DIVIDER);
    untitled.title.clear();
    let AssignmentSaveOutcome::Invalid { errors } = session.save_to(&existing, &untitled).unwrap()
    else {
        panic!("an untitled assignment should be refused");
    };
    assert_eq!(errors[0].path, "/title");
    assert_eq!(fs::read(&existing).unwrap(), before);
    assert_eq!(session.current_path().unwrap(), Some(existing.clone()));

    // A save that cannot be completed (the folder is not there) is an error, changes nothing,
    // and leaves no temporary file next to the existing one.
    let impossible = directory
        .path()
        .join("missing-folder")
        .join("a.ocassignment");
    assert!(session.save_to(&impossible, &assignment(DIVIDER)).is_err());
    assert_eq!(session.current_path().unwrap(), Some(existing.clone()));
    assert_eq!(fs::read(&existing).unwrap(), before);
    let leftovers = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(leftovers.len(), 2, "{leftovers:?}");

    session.clear_path().unwrap();
    assert_eq!(session.current_path().unwrap(), None);
}

#[test]
fn exporting_the_students_copy_drops_the_reference_and_keeps_everything_else() {
    let directory = temporary();
    let path = directory.path().join("student.ocassignment");
    let teacher = assignment(DIVIDER);

    write_assignment_file(&path, &teacher.student_copy()).unwrap();
    let AssignmentDecoded::Valid { assignment: copy } = read_assignment_file(&path).unwrap() else {
        panic!("the copy should open");
    };

    assert!(teacher.reference.is_some());
    assert!(copy.reference.is_none());
    assert!(!fs::read_to_string(&path).unwrap().contains("\"reference\""));
    assert_eq!(copy.checks, teacher.checks);
    assert_eq!(copy.starting_circuit, teacher.starting_circuit);
    assert_eq!(copy.restrictions, teacher.restrictions);
    assert_eq!(copy.title, teacher.title);
}

#[test]
fn the_results_the_bridge_returns_have_the_wire_format_the_interface_reads() {
    let valid = serde_json::to_value(AssignmentDecoded::from_result(decode_assignment_json(
        DIVIDER.as_bytes(),
    )))
    .unwrap();
    assert_eq!(valid["status"], "valid");
    assert_eq!(valid["assignment"]["task"], json!({ "kind": "build" }));

    let invalid = serde_json::to_value(AssignmentDecoded::from_result(decode_assignment_json(
        b"{}",
    )))
    .unwrap();
    assert_eq!(
        invalid,
        json!({
            "status": "invalid",
            "errors": [{
                "code": "UnsupportedVersion", "path": "/schemaVersion",
                "messageKey": "assignment.error.unsupportedVersion"
            }]
        })
    );
    assert_eq!(
        serde_json::to_value(AssignmentSaveOutcome::Saved).unwrap(),
        json!({ "status": "saved" })
    );
    assert_eq!(
        serde_json::to_value(AssignmentSaveOutcome::Cancelled).unwrap(),
        json!({ "status": "cancelled" })
    );

    let fault = assignment(FAULT);
    let checked = serde_json::to_value(DiagnosisChecked::from_result(check_diagnosis(
        &fault,
        &json!([{ "componentId": "00000000-0000-4000-8000-000000000003", "kind": "open" }]),
    )))
    .unwrap();
    assert_eq!(checked["status"], "checked");
    assert_eq!(checked["result"]["outcome"], "passed");
    assert_eq!(checked["result"]["outcomeKey"], "diagnosis.outcome.passed");
}

// --- checks --------------------------------------------------------------------------

/// A check request for `student`, a schematic JSON value, with the geometry of its parts.
fn check_request(assignment: &Assignment, student: Value, geometry: Value) -> CheckRequest {
    serde_json::from_value(json!({
        "checks": assignment.checks,
        "lockedComponents": assignment.restrictions.as_ref().unwrap().locked_components,
        "startingCircuit": assignment.starting_circuit,
        "schematic": student,
        "geometry": geometry,
        "documentRevision": "rev-9",
    }))
    .unwrap()
}

fn reference_attempt(assignment: &Assignment, geometry: &str) -> (Value, Value) {
    (
        serde_json::to_value(assignment.reference.as_ref().unwrap()).unwrap(),
        serde_json::from_str(geometry).unwrap(),
    )
}

fn remove_component(attempt: &mut (Value, Value), id: &str) {
    attempt.0["components"]
        .as_array_mut()
        .unwrap()
        .retain(|component| component["id"] != id);
    attempt.1["components"]
        .as_array_mut()
        .unwrap()
        .retain(|component| component["componentId"] != id);
}

fn set_parameter(attempt: &mut (Value, Value), id: &str, key: &str, value: f64) {
    let component = attempt.0["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["id"] == id)
        .unwrap();
    component["parameters"][key] = json!(value);
}

fn verdicts(result: &CheckResult) -> Vec<CheckVerdict> {
    result
        .criteria
        .iter()
        .map(|criterion| match criterion {
            CriterionResult::Value { verdict, .. }
            | CriterionResult::Range { verdict, .. }
            | CriterionResult::DeviceState { verdict, .. }
            | CriterionResult::ComponentCount { verdict, .. } => *verdict,
        })
        .collect()
}

fn run_check(assignment: &Assignment, attempt: (Value, Value)) -> CheckResult {
    CheckService::default()
        .run(check_request(assignment, attempt.0, attempt.1))
        .expect("the request is within the limits")
}

#[test]
fn the_check_command_gives_the_canonical_divider_attempts_their_results() {
    use CheckVerdict::{Fail, Pass};
    let divider = assignment(DIVIDER);
    let reference = || reference_attempt(&divider, DIVIDER_GEOMETRY);

    // The teacher's reference passes its own checks (section 2.3), and the reading is the
    // divider equation: 10 * (10k || 10M) / (30k + (10k || 10M)).
    let correct = run_check(&divider, reference());
    assert_eq!(correct.outcome, CheckOutcome::Passed);
    assert_eq!(verdicts(&correct), vec![Pass, Pass, Pass]);
    assert_eq!(correct.document_revision, "rev-9");
    let CriterionResult::Value { measured, .. } = &correct.criteria[0] else {
        panic!("a value result");
    };
    assert!((measured.unwrap() - 2.498126405196103).abs() < 1e-9);

    // Two equal resistors: the right count and the wrong ratio.
    let mut equal = reference();
    set_parameter(&mut equal, R_TOP, "resistanceOhms", 10_000.0);
    let equal = run_check(&divider, equal);
    assert_eq!(equal.outcome, CheckOutcome::Partial);
    assert_eq!(verdicts(&equal), vec![Fail, Fail, Pass]);
    let CriterionResult::Value { measured, .. } = &equal.criteria[0] else {
        panic!("a value result");
    };
    assert!((measured.unwrap() - 4.997501249375313).abs() < 1e-9);

    // Nothing added: the voltmeter reads 0 V.
    let mut nothing = reference();
    remove_component(&mut nothing, R_TOP);
    remove_component(&mut nothing, R_BOTTOM);
    let nothing = run_check(&divider, nothing);
    assert_eq!(nothing.outcome, CheckOutcome::Failed);
    assert_eq!(verdicts(&nothing), vec![Fail, Fail, Fail]);

    // A locked source changed: failed, whatever the circuit does.
    let mut changed = reference();
    set_parameter(&mut changed, V1, "voltageVolts", 20.0);
    let changed = run_check(&divider, changed);
    assert_eq!(changed.outcome, CheckOutcome::Failed);
    assert_eq!(changed.locked.verdict, CheckVerdict::Fail);

    // Three resistors with the same reading: 20 kOhm and 10 kOhm in series on top, between
    // (0, 0), a new node at (2, 0), and (4, 0). The count criterion fails.
    let mut three = reference();
    set_parameter(&mut three, R_TOP, "resistanceOhms", 20_000.0);
    three.1["components"][2]["pins"][1]["position"] = json!({ "x": 2, "y": 0 });
    three.0["components"].as_array_mut().unwrap().push(json!({
        "id": R_MIDDLE, "typeId": "resistor", "position": { "x": 2, "y": 4 },
        "rotationDeg": 0, "parameters": { "resistanceOhms": 10_000.0 }
    }));
    three.1["components"].as_array_mut().unwrap().push(json!({
        "componentId": R_MIDDLE,
        "pins": [
            { "pinId": "pin1", "position": { "x": 2, "y": 0 } },
            { "pinId": "pin2", "position": { "x": 4, "y": 0 } }
        ]
    }));
    let three = run_check(&divider, three);
    assert_eq!(three.outcome, CheckOutcome::Partial);
    assert_eq!(verdicts(&three), vec![Pass, Pass, Fail]);
    let CriterionResult::Value { measured, .. } = &three.criteria[0] else {
        panic!("a value result");
    };
    assert!((measured.unwrap() - 2.498126405196103).abs() < 1e-9);
}

#[test]
fn the_check_command_reads_a_device_state_of_the_canonical_fuse_assignment() {
    let fuse = assignment(FUSE);
    let reference = || reference_attempt(&fuse, FUSE_GEOMETRY);

    let one_ohm = run_check(&fuse, reference());
    assert_eq!(one_ohm.outcome, CheckOutcome::Passed);

    let mut ten_ohm = reference();
    set_parameter(&mut ten_ohm, R_TOP, "resistanceOhms", 10.0);
    let ten_ohm = run_check(&fuse, ten_ohm);
    assert_eq!(ten_ohm.outcome, CheckOutcome::Failed);
    let CriterionResult::DeviceState { measured, .. } = &ten_ohm.criteria[0] else {
        panic!("a device state result");
    };
    assert_eq!(
        serde_json::to_value(measured.unwrap()).unwrap(),
        json!("intact")
    );
}

#[test]
fn a_request_over_the_criteria_limit_is_refused_not_evaluated() {
    let divider = assignment(DIVIDER);
    let (schematic, geometry) = reference_attempt(&divider, DIVIDER_GEOMETRY);
    let mut request = check_request(&divider, schematic, geometry);
    let criterion = request.checks[2].clone();
    request.checks = vec![criterion; 65];

    let service = CheckService::default();
    assert!(service.run(request.clone()).is_err());
    request.checks.truncate(64);
    assert!(service.run(request).is_ok());
}

#[test]
fn concurrent_checks_each_get_their_own_result() {
    let divider = assignment(DIVIDER);
    let (schematic, geometry) = reference_attempt(&divider, DIVIDER_GEOMETRY);
    let service = std::sync::Arc::new(CheckService::default());

    let handles = (0..4)
        .map(|index| {
            let service = std::sync::Arc::clone(&service);
            let mut request = check_request(&divider, schematic.clone(), geometry.clone());
            request.document_revision = format!("rev-{index}");
            std::thread::spawn(move || service.run(request).unwrap())
        })
        .collect::<Vec<_>>();
    for (index, handle) in handles.into_iter().enumerate() {
        let result = handle.join().unwrap();
        assert_eq!(result.document_revision, format!("rev-{index}"));
        assert_eq!(result.outcome, CheckOutcome::Passed);
    }
}

// --- faults and the live run ----------------------------------------------------------

struct ChannelSink(Sender<Vec<u8>>);

impl FrameSink for ChannelSink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.send(frame).map_err(|_| SinkClosed)
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

fn fault_request(revision: &str, with_faults: bool) -> CompileRequest {
    let fault = assignment(FAULT);
    CompileRequest {
        schematic: fault.starting_circuit.clone().unwrap(),
        geometry: serde_json::from_str::<GeometryEnvelope>(FAULT_GEOMETRY).unwrap(),
        document_revision: revision.to_owned(),
        faults: with_faults.then(|| fault.faults.clone().unwrap()),
    }
}

/// Compiles on the simulation thread. Returns the run and the meter's voltage at step 0.
fn compile(service: &SimulationService, request: CompileRequest) -> (StableId, f64) {
    let CompileResult::Success {
        run_id,
        initial_snapshot,
        layout,
        ..
    } = service.compile(request).expect("the thread answers")
    else {
        panic!("the canonical circuit must compile");
    };
    // The meter's voltage slot, whatever the layout order is.
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.key == format!("{FAULT_METER}:voltage"))
        .unwrap();
    (run_id, initial_snapshot.values[slot.offset as usize])
}

fn next_frame(receiver: &Receiver<Vec<u8>>) -> ResultSnapshot {
    decode_frame(&receiver.recv_timeout(WAIT).expect("a frame arrives")).expect("decodes")
}

#[test]
fn a_faulted_compile_gives_the_fixtures_readings_and_a_recompile_replaces_the_run() {
    let expected: Value = serde_json::from_str(FAULT_EXPECTED).unwrap();
    let reading = |name: &str| {
        expected["measurements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"].as_str().unwrap().starts_with(name))
            .unwrap()["voltmeterReadingVolts"]
            .as_f64()
            .unwrap()
    };
    let service = SimulationService::spawn(fast_config());

    let (faulted_run, faulted) = compile(&service, fault_request("faulted", true));
    assert!((faulted - reading("the fault of the assignment")).abs() < 1e-9);

    // Live frames of the faulted run carry its run, its revision, and the faulted reading.
    let (sender, receiver) = mpsc::channel();
    service
        .start(faulted_run.clone(), Box::new(ChannelSink(sender)))
        .unwrap()
        .expect("the run is active");
    let frame = next_frame(&receiver);
    assert_eq!(frame.metadata.run_id, faulted_run);
    assert_eq!(frame.metadata.document_revision, "faulted");
    service.acknowledge(frame.metadata.frame_sequence).unwrap();

    // A recompile of the same document without faults is a new, healthy run; the faulted run
    // is stale for every later request.
    let (healthy_run, healthy) = compile(&service, fault_request("healthy", false));
    assert_ne!(healthy_run, faulted_run);
    assert!((healthy - reading("healthy circuit")).abs() < 1e-9);
    let stale = service
        .set_parameter(SetParameterRequest {
            run_id: faulted_run.clone(),
            expected_document_revision: "faulted".to_owned(),
            document_revision: "faulted-2".to_owned(),
            component_id: StableId::parse("00000000-0000-4000-8000-000000000002").unwrap(),
            parameter_id: ParameterId::ResistanceOhms,
            value: 2000.0,
        })
        .unwrap();
    let SetParameterResult::Failure { diagnostics } = stale else {
        panic!("a stale run must not be updated");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);
    assert!(
        service
            .start(faulted_run, Box::new(ChannelSink(mpsc::channel().0)))
            .unwrap()
            .is_err()
    );
    service.shutdown();
}

#[test]
fn faults_are_never_written_into_a_saved_project() {
    let directory = temporary();
    let path = directory.path().join("student.ocircuit");
    let fault = assignment(FAULT);
    let request = fault_request("faulted", true);

    // The faulted compile does not alter the schematic it was given.
    let service = SimulationService::spawn(fast_config());
    compile(&service, request.clone());
    assert_eq!(request.schematic, fault.starting_circuit.clone().unwrap());

    write_project_file(&path, &request.schematic).unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    for answer in [
        "faults",
        "\"open\"",
        "kind",
        "checks",
        "reveal",
        "instructions",
    ] {
        assert!(!saved.contains(answer), "the project contains {answer}");
    }
    service.shutdown();
}

#[test]
fn a_check_never_disturbs_the_run_the_user_is_watching() {
    let divider = assignment(DIVIDER);
    let (schematic, geometry) = reference_attempt(&divider, DIVIDER_GEOMETRY);
    let service = SimulationService::spawn(fast_config());
    let live = CompileRequest {
        schematic: serde_json::from_value(schematic.clone()).unwrap(),
        geometry: serde_json::from_value(geometry.clone()).unwrap(),
        document_revision: "live".to_owned(),
        faults: None,
    };
    let CompileResult::Success { run_id, .. } = service.compile(live).unwrap() else {
        panic!("the reference compiles");
    };
    let (sender, receiver) = mpsc::channel();
    service
        .start(run_id.clone(), Box::new(ChannelSink(sender)))
        .unwrap()
        .expect("the run is active");

    let first = next_frame(&receiver);
    service.acknowledge(first.metadata.frame_sequence).unwrap();

    // Checks (an unrelated, even failing, circuit among them) run while the stream is open.
    let checks = CheckService::default();
    let mut other = reference_attempt(&divider, DIVIDER_GEOMETRY);
    set_parameter(&mut other, V1, "voltageVolts", 20.0);
    for attempt in [(schematic.clone(), geometry.clone()), other] {
        let result = checks
            .run(check_request(&divider, attempt.0, attempt.1))
            .unwrap();
        assert_eq!(result.document_revision, "rev-9");
    }

    // The live run goes on, in order, with its own run and revision.
    let mut last = first.metadata.frame_sequence;
    for _ in 0..3 {
        let frame = next_frame(&receiver);
        assert_eq!(frame.metadata.run_id, run_id);
        assert_eq!(frame.metadata.document_revision, "live");
        assert!(frame.metadata.frame_sequence > last);
        last = frame.metadata.frame_sequence;
        service.acknowledge(last).unwrap();
    }
    service.shutdown();
}

#[test]
fn a_diagnosis_of_a_compiled_fault_task_is_compared_without_solving_anything() {
    // The canonical fault is R2 open; the unfaulted meter component is not a fault.
    let fault = assignment(FAULT);
    let outcome = |component: &str, kind: &str| {
        let DiagnosisChecked::Checked { result } = DiagnosisChecked::from_result(check_diagnosis(
            &fault,
            &json!([{ "componentId": component, "kind": kind }]),
        )) else {
            panic!("the diagnosis is well formed");
        };
        serde_json::to_value(result.outcome).unwrap()
    };

    assert_eq!(
        outcome("00000000-0000-4000-8000-000000000003", "open"),
        "passed"
    );
    assert_eq!(
        outcome("00000000-0000-4000-8000-000000000003", "short"),
        "partial"
    );
    assert_eq!(
        outcome("00000000-0000-4000-8000-000000000002", "open"),
        "failed"
    );
}
