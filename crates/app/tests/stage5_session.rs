//! The Stage 5 acceptance session (issue #85): the canonical assignments, authored and drawn
//! the way a teacher and a student do it in the application, run through the real Engine and
//! the bridge code, and carried the way the bridge carries them.
//!
//! The assignment files, the check requests of the student's attempts, and the faulted compile
//! request are `tests/golden/stage-5-drawn-*`, produced by `src/store/stage5Drawing.test.ts`
//! from a blank document with the editor's own commands, the draft store, and the student
//! side's request builders. The expected values are the fixtures'
//! `tests/golden/stage-5-*.expected.json`. The test writes the session to
//! `<target>/tmp/stage5-session.json`, which `src/store/stage5EndToEnd.test.tsx` replays through
//! the TypeScript adapter, controller, and panels.

mod common;

use std::fs;
use std::path::PathBuf;

use common::{Run, assert_close, hex, value_of};
use open_circuit_app_lib::assignment::{
    AssignmentSession, read_assignment_file, write_assignment_file,
};
use open_circuit_app_lib::checks::CheckService;
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_engine::{
    Assignment, AssignmentDecoded, AssignmentSaveOutcome, CheckOutcome, CheckRequest, CheckResult,
    CheckVerdict, CompileRequest, CompileResult, CriterionResult, DiagnosisOutcome, DiagnosticCode,
    FaultSpec, ResultQuantity, SetParameterRequest, SetParameterResult, StableId, check_diagnosis,
    component_catalog, decode_assignment_json, encode_assignment_json,
};
use serde_json::{Value, json};

const DIVIDER: &str =
    include_str!("../../../tests/golden/stage-5-drawn-divider-build.ocassignment");
const DIVIDER_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-5-divider-build.expected.json");
const FAULT: &str = include_str!("../../../tests/golden/stage-5-drawn-fault-finding.ocassignment");
const FAULT_REQUEST: &str =
    include_str!("../../../tests/golden/stage-5-drawn-fault-finding.request.json");
const FAULT_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-5-fault-finding.expected.json");

const ATTEMPTS: [(&str, &str); 6] = [
    (
        "correct",
        include_str!("../../../tests/golden/stage-5-drawn-divider-correct.check.json"),
    ),
    (
        "three-resistors",
        include_str!("../../../tests/golden/stage-5-drawn-divider-three-resistors.check.json"),
    ),
    (
        "equal-resistors",
        include_str!("../../../tests/golden/stage-5-drawn-divider-equal-resistors.check.json"),
    ),
    (
        "no-resistors",
        include_str!("../../../tests/golden/stage-5-drawn-divider-no-resistors.check.json"),
    ),
    (
        "locked-part-changed",
        include_str!("../../../tests/golden/stage-5-drawn-divider-locked-part-changed.check.json"),
    ),
    (
        "source-shorted",
        include_str!("../../../tests/golden/stage-5-drawn-divider-source-shorted.check.json"),
    ),
];

/// The fault-finding circuit numbers its parts in the order they were drawn.
const R1: &str = "00000000-0000-4000-8000-000000000002";
const R2: &str = "00000000-0000-4000-8000-000000000003";
const METER: &str = "00000000-0000-4000-8000-000000000004";

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("a JSON fixture")
}

fn assignment(text: &str) -> Assignment {
    decode_assignment_json(text.as_bytes()).expect("the authored assignment is valid")
}

fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number"))
}

fn temporary() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary directory")
}

fn verdicts(result: &CheckResult) -> Vec<&'static str> {
    result
        .criteria
        .iter()
        .map(|criterion| match criterion {
            CriterionResult::Value { verdict, .. }
            | CriterionResult::Range { verdict, .. }
            | CriterionResult::DeviceState { verdict, .. }
            | CriterionResult::ComponentCount { verdict, .. } => verdict_name(*verdict),
        })
        .collect()
}

fn verdict_name(verdict: CheckVerdict) -> &'static str {
    match verdict {
        CheckVerdict::Pass => "pass",
        CheckVerdict::Fail => "fail",
        CheckVerdict::NotEvaluated => "notEvaluated",
    }
}

fn outcome_name(outcome: CheckOutcome) -> &'static str {
    match outcome {
        CheckOutcome::Passed => "passed",
        CheckOutcome::Partial => "partial",
        CheckOutcome::Failed => "failed",
        CheckOutcome::NotEvaluated => "notEvaluated",
    }
}

/// The student's copy as the application writes it and reads it back.
fn student_copy_text(teacher: &Assignment) -> (String, Assignment) {
    let directory = temporary();
    let path = directory.path().join("student.ocassignment");

    assert_eq!(
        write_assignment_file(&path, &teacher.student_copy()).unwrap(),
        AssignmentSaveOutcome::Saved
    );
    let text = fs::read_to_string(&path).unwrap();
    let AssignmentDecoded::Valid { assignment: copy } = read_assignment_file(&path).unwrap() else {
        panic!("the student's copy should open");
    };

    (text, *copy)
}

#[test]
fn the_teacher_saves_the_divider_assignment_and_the_student_opens_a_copy_without_the_reference() {
    let teacher = assignment(DIVIDER);
    let directory = temporary();
    let path = directory.path().join("divider.ocassignment");
    let session = AssignmentSession::default();

    // The teacher's file keeps the reference and round-trips exactly.
    assert_eq!(
        session.save_to(&path, &teacher).unwrap(),
        AssignmentSaveOutcome::Saved
    );
    let AssignmentDecoded::Valid {
        assignment: reopened,
    } = session.load(&path).unwrap()
    else {
        panic!("the saved file should open");
    };
    assert_eq!(*reopened, teacher);
    let written = fs::read_to_string(&path).unwrap();
    assert!(written.contains("\"reference\""));
    for computed in [
        "snapshot", "layout", "runId", "trace", "measured", "verdict",
    ] {
        assert!(!written.contains(computed), "the file stores {computed}");
    }

    // The student's copy is the same task without it, and complete.
    let (text, copy) = student_copy_text(&teacher);
    assert!(!text.contains("\"reference\""));
    assert!(copy.reference.is_none());
    assert_eq!(copy.checks, teacher.checks);
    assert_eq!(copy.starting_circuit, teacher.starting_circuit);
    assert_eq!(copy.restrictions, teacher.restrictions);
    assert_eq!(copy.title, "Divider to 2.5 V");

    // The authoring panel's draft conversion writes what the Engine accepts and what its own
    // writer writes back, up to formatting.
    let encoded = encode_assignment_json(&teacher).unwrap();
    assert_eq!(decode_assignment_json(&encoded).unwrap(), teacher);
}

#[test]
fn the_students_attempts_get_the_agreed_structured_results() {
    let expected = json_of(DIVIDER_EXPECTED);
    let service = CheckService::default();
    let mut ran = 0;

    for (name, text) in ATTEMPTS {
        let request: CheckRequest = serde_json::from_str(text).expect("a check request");
        assert_eq!(request.document_revision, "check-1");
        let result = service.run(request.clone()).expect("within the limits");
        let attempt = expected["attempts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|attempt| attempt["name"] == name)
            .unwrap_or_else(|| panic!("no fixture attempt {name}"));
        let wanted = &attempt["expected"];

        assert_eq!(result.document_revision, "check-1", "{name}");
        assert_eq!(
            outcome_name(result.outcome),
            wanted["outcome"],
            "{name}: outcome"
        );
        let by_name = verdicts(&result);
        for (index, criterion) in ["reading", "stable", "two-resistors"].iter().enumerate() {
            if let Some(verdict) = wanted["verdicts"].get(*criterion) {
                assert_eq!(
                    by_name[index],
                    verdict.as_str().unwrap(),
                    "{name}: {criterion}"
                );
            }
        }
        assert_eq!(
            verdict_name(result.locked.verdict),
            wanted["verdicts"]["locked"].as_str().unwrap(),
            "{name}: locked"
        );
        if let Some(reading) = attempt["voltmeterReadingVolts"].as_f64() {
            let CriterionResult::Value { measured, unit, .. } = &result.criteria[0] else {
                panic!("the first criterion reads the voltmeter");
            };
            assert_close(
                measured.expect("a measured reading"),
                reading,
                number(attempt, "readingToleranceVolts"),
                &format!("{name}: the reading"),
            );
            assert_eq!(serde_json::to_value(unit).unwrap(), "volt");
        }
        if let Some(codes) = wanted["diagnostics"].as_array() {
            let found = result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<DiagnosticCode>>();
            let expected_codes = codes
                .iter()
                .map(|code| serde_json::from_value(code.clone()).unwrap())
                .collect::<Vec<DiagnosticCode>>();
            assert_eq!(found, expected_codes, "{name}: diagnostics");
        }
        // The same request gives the same result, and no number is lost.
        assert_eq!(service.run(request).unwrap(), result, "{name}: determinism");
        assert!(
            !serde_json::to_string(&result).unwrap().contains("null"),
            "{name}"
        );
        ran += 1;
    }

    assert_eq!(ran, 6);
}

#[test]
fn the_reference_solution_passes_its_own_checks_and_a_failed_attempt_never_does() {
    let teacher = assignment(DIVIDER);
    let correct: CheckRequest = serde_json::from_str(ATTEMPTS[0].1).expect("the correct attempt");

    // The teacher's preview checks the reference; the student's correct attempt is the same
    // circuit as the drawn reference, with the same parts and values.
    assert_eq!(Some(&correct.schematic), teacher.reference.as_ref());
    let result = CheckService::default().run(correct).unwrap();
    assert_eq!(result.outcome, CheckOutcome::Passed);
    assert_eq!(result.counts.pass, 3);
    assert_eq!(result.counts.fail + result.counts.not_evaluated, 0);
}

fn fault_request(faults: Option<Vec<FaultSpec>>) -> CompileRequest {
    let mut request: CompileRequest = serde_json::from_str(FAULT_REQUEST).expect("a request");
    request.faults = faults;
    request
}

fn fault(kind: &str, component: &str) -> FaultSpec {
    serde_json::from_value(json!({
        "id": "00000000-0000-4000-8000-0000000000b1",
        "kind": kind,
        "componentId": component,
    }))
    .unwrap()
}

fn reading(run: &Run, snapshot: &open_circuit_engine::ResultSnapshot) -> f64 {
    value_of(&run.layout, snapshot, METER, ResultQuantity::Voltage)
}

#[test]
fn the_faulty_circuit_behaves_as_the_fixture_says_and_the_fault_is_in_no_saved_file() {
    let expected = json_of(FAULT_EXPECTED);
    let measurement = |name: &str| -> f64 {
        let entry = expected["measurements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"].as_str().unwrap().starts_with(name))
            .unwrap_or_else(|| panic!("no measurement {name}"));
        number(entry, "voltmeterReadingVolts")
    };
    let teacher = assignment(FAULT);
    let faults = teacher
        .faults
        .clone()
        .expect("a fault-finding task has faults");

    // What the student's session compiles: the drawn circuit with the assignment's faults.
    let mut run = Run::start_every_step(fault_request(Some(faults.clone())));
    let faulted = measurement("the fault of the assignment");
    let faulted_entry = expected["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["name"]
                .as_str()
                .unwrap()
                .starts_with("the fault of the assignment")
        })
        .unwrap();
    run.advance(5);

    for step in [0_u64, 5] {
        let snapshot = if step == 0 {
            run.initial_snapshot.clone()
        } else {
            run.frame_at(step).0.clone()
        };
        assert_close(
            reading(&run, &snapshot),
            faulted,
            number(faulted_entry, "toleranceVolts"),
            &format!("the faulted reading at step {step}"),
        );
        // R2 carries no current; R1 carries only the meter's.
        assert_eq!(
            value_of(&run.layout, &snapshot, R2, ResultQuantity::Current),
            0.0
        );
        assert_close(
            value_of(&run.layout, &snapshot, R1, ResultQuantity::Current),
            number(faulted_entry, "r1CurrentAmps"),
            number(faulted_entry, "currentToleranceAmps"),
            "the current of R1",
        );
        assert_close(
            value_of(&run.layout, &snapshot, R2, ResultQuantity::Voltage),
            number(faulted_entry, "r2VoltageVolts"),
            number(faulted_entry, "toleranceVolts"),
            "the voltage across the open R2",
        );
    }
    // Frames of the faulted run carry the faulted reading, in order.
    assert!(run.delivered.len() >= 5);

    // The same document without the faults is the healthy circuit, a new run; the faulted one is stale.
    let faulted_run = run.run_id.clone();
    let healthy = Run::start_every_step(fault_request(None));
    assert_close(
        reading(&healthy, &healthy.initial_snapshot),
        measurement("healthy circuit"),
        1e-9,
        "the healthy reading",
    );
    // Compiling the same document without the faults on the same core replaces the faulted
    // run: an update that names the faulted run is then refused as stale.
    let CompileResult::Success {
        run_id: replacement,
        ..
    } = run.core.compile(fault_request(None))
    else {
        panic!("the healthy circuit compiles");
    };
    assert_ne!(replacement, faulted_run);
    let stale = run.core.set_parameter(SetParameterRequest {
        run_id: faulted_run.clone(),
        expected_document_revision: "rev-1".to_owned(),
        document_revision: "rev-2".to_owned(),
        component_id: StableId::parse(R1).unwrap(),
        parameter_id: open_circuit_engine::ParameterId::ResistanceOhms,
        value: 2000.0,
    });
    let SetParameterResult::Failure { diagnostics } = stale else {
        panic!("an update for a replaced run must be refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);
    assert_ne!(healthy.run_id, faulted_run);

    // The wrong hypotheses read what the fixture lists for them.
    for (name, kind, component) in [
        ("hypothesis: R2 short", "short", R2),
        ("hypothesis: R1 open", "open", R1),
        ("hypothesis: R1 short", "short", R1),
    ] {
        let other = Run::start_every_step(fault_request(Some(vec![fault(kind, component)])));
        let entry = expected["measurements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"].as_str().unwrap().starts_with(name))
            .unwrap();
        assert_close(
            reading(&other, &other.initial_snapshot),
            number(entry, "voltmeterReadingVolts"),
            number(entry, "toleranceVolts"),
            name,
        );
    }

    // The student saves progress as a project: it is the circuit, and no fault is in it.
    let request = fault_request(Some(faults.clone()));
    let directory = temporary();
    let path = directory.path().join("progress.ocircuit");
    write_project_file(&path, &request.schematic).unwrap();
    let saved = fs::read_to_string(&path).unwrap();
    for answer in [
        "faults",
        "\"open\"",
        "kind",
        "0a1",
        "instructions",
        "checks",
    ] {
        assert!(!saved.contains(answer), "the project holds {answer}");
    }
    assert_eq!(read_project_file(&path).unwrap(), request.schematic);
    assert_eq!(Some(&request.schematic), teacher.starting_circuit.as_ref());

    // The student's copy of the assignment carries the faults (the contract accepts that
    // limit, section 2.4), and the teacher's file keeps them too.
    let (text, copy) = student_copy_text(&teacher);
    assert!(text.contains("\"faults\""));
    assert_eq!(copy.faults, teacher.faults);
}

#[test]
fn the_diagnoses_of_the_fixture_are_checked_by_the_engine() {
    let expected = json_of(FAULT_EXPECTED);
    let (_, copy) = student_copy_text(&assignment(FAULT));
    let component = |name: &str| match name {
        "R1" => R1,
        "R2" => R2,
        other => panic!("no component {other}"),
    };

    for diagnosis in expected["diagnoses"].as_array().unwrap() {
        let name = diagnosis["name"].as_str().unwrap();
        let submitted = Value::Array(
            diagnosis["submitted"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    json!({
                        "componentId": component(entry["component"].as_str().unwrap()),
                        "kind": entry["kind"],
                    })
                })
                .collect(),
        );
        let result = check_diagnosis(&copy, &submitted).unwrap_or_else(|errors| {
            panic!("{name}: {errors:?}");
        });
        let wanted = &diagnosis["expected"];

        assert_eq!(
            serde_json::to_value(result.outcome).unwrap(),
            wanted["outcome"],
            "{name}"
        );
        let entries = result
            .entries
            .iter()
            .map(|entry| serde_json::to_value(entry.verdict).unwrap())
            .collect::<Vec<_>>();
        let wanted_entries = wanted["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["verdict"].clone())
            .collect::<Vec<_>>();
        assert_eq!(entries, wanted_entries, "{name}: entries");
        assert_eq!(
            serde_json::to_value(result.faults[0].verdict).unwrap(),
            wanted["faults"][0]["verdict"],
            "{name}: the fault"
        );
    }

    // A wrong diagnosis is never a pass, and the right one is.
    let right = json!([{ "componentId": R2, "kind": "open" }]);
    let wrong = json!([{ "componentId": R1, "kind": "open" }]);
    assert_eq!(
        check_diagnosis(&copy, &right).unwrap().outcome,
        DiagnosisOutcome::Passed
    );
    assert_eq!(
        check_diagnosis(&copy, &wrong).unwrap().outcome,
        DiagnosisOutcome::Failed
    );
}

/// Writes what the interface replays: the requests and replies of the whole session.
#[test]
fn the_session_is_recorded_for_the_interface() {
    let teacher = assignment(DIVIDER);
    let (student_text, _) = student_copy_text(&teacher);
    let service = CheckService::default();
    let attempts = ATTEMPTS
        .iter()
        .map(|(name, text)| {
            let request: CheckRequest = serde_json::from_str(text).unwrap();
            let result = service.run(request.clone()).unwrap();
            json!({ "name": name, "request": request, "result": result })
        })
        .collect::<Vec<_>>();

    let fault_teacher = assignment(FAULT);
    let (fault_text, fault_copy) = student_copy_text(&fault_teacher);
    let faults = fault_teacher.faults.clone().unwrap();
    let mut faulted = Run::start_every_step(fault_request(Some(faults.clone())));
    faulted.advance(3);
    // Step zero is the compile reply's own snapshot; the frames are the steps after it.
    let frames = (1..=3)
        .map(|step| json!({ "step": step, "hex": hex(&faulted.frame_at(step).1) }))
        .collect::<Vec<_>>();
    let healthy = Run::start_every_step(fault_request(None));
    let diagnoses = json_of(FAULT_EXPECTED)["diagnoses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|diagnosis| {
            let component = |name: &str| if name == "R1" { R1 } else { R2 };
            let submitted = Value::Array(
                diagnosis["submitted"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| {
                        json!({
                            "componentId": component(entry["component"].as_str().unwrap()),
                            "kind": entry["kind"],
                        })
                    })
                    .collect(),
            );
            let result = check_diagnosis(&fault_copy, &submitted).unwrap();
            json!({
                "name": diagnosis["name"],
                "submitted": submitted,
                "result": result,
            })
        })
        .collect::<Vec<_>>();

    let recorded = json!({
        "format": 1,
        "description": "The Stage 5 acceptance session, written by crates/app/tests/stage5_session.rs on every cargo test. src/store/stage5EndToEnd.test.tsx replays it through the TypeScript adapter, controller, and panels.",
        "catalog": component_catalog(),
        "divider": {
            "teacherFile": json_of(DIVIDER),
            "studentCopyText": student_text,
            "attempts": attempts,
        },
        "fault": {
            "teacherFile": json_of(FAULT),
            "studentCopyText": fault_text,
            "request": fault_request(Some(faults)),
            "faultedReply": faulted.compile_result,
            "faultedFrames": frames,
            "healthyReply": healthy.compile_result,
            "diagnoses": diagnoses,
        },
    });
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("stage5-session.json");
    fs::create_dir_all(path.parent().expect("a parent directory")).expect("create the directory");
    fs::write(
        &path,
        serde_json::to_string_pretty(&recorded).expect("the session serializes"),
    )
    .expect("write the session");

    // Nothing about a result is in the files the application saves.
    assert!(matches!(
        recorded["divider"]["attempts"][0]["result"]["outcome"].as_str(),
        Some("passed")
    ));
    let CompileResult::Success { .. } = &healthy.compile_result else {
        panic!("the healthy circuit compiles");
    };
}
