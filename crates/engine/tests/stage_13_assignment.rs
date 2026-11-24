//! Fault-finding assignments with the `leak` fault (docs/contracts/stage-13-protection-and-safety.md
//! sections 3.1 and 3.4): the file is checked when it loads, a diagnosis names a leak by its
//! component and pin, and the Stage 9, 10 and 13 devices are open to a fault-finding task only.

use open_circuit_engine::{
    AssignmentErrorCode, DiagnosisEntryVerdict, DiagnosisFaultVerdict, DiagnosisOutcome,
    DiagnosisResult, check_diagnosis, decode_assignment_json, encode_assignment_json,
    validate_assignment_value,
};
use serde_json::{Value, json};

use AssignmentErrorCode::{InvalidField, TaskMismatch, UnknownField, UnknownReference};

const FILE: &str = include_str!("../../../tests/golden/stage-13-earth-fault.ocassignment");
const LEGACY: &str = include_str!("../../../tests/golden/stage-5-fault-finding.ocassignment");

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const LOAD: &str = "00000000-0000-4000-8000-000000000002";
const FRAME: &str = "00000000-0000-4000-8000-000000000003";
const ELECTRODE: &str = "00000000-0000-4000-8000-000000000004";

fn file() -> Value {
    serde_json::from_str(FILE).unwrap()
}

fn errors(document: &Value) -> Vec<(AssignmentErrorCode, String)> {
    match validate_assignment_value(document) {
        Ok(_) => Vec::new(),
        Err(errors) => errors
            .into_iter()
            .map(|error| (error.code, error.path))
            .collect(),
    }
}

fn with_fault(edit: impl FnOnce(&mut Value)) -> Value {
    let mut document = file();
    edit(&mut document["faults"][0]);
    document
}

fn at(code: AssignmentErrorCode, path: &str) -> (AssignmentErrorCode, String) {
    (code, path.to_owned())
}

fn remove(fault: &mut Value, key: &str) {
    fault.as_object_mut().unwrap().remove(key);
}

#[test]
fn the_canonical_three_phase_file_loads_and_round_trips() {
    let assignment = decode_assignment_json(FILE.as_bytes()).expect("the file loads");
    let encoded = encode_assignment_json(&assignment).expect("the file encodes");
    let again = decode_assignment_json(&encoded).expect("the encoding loads");

    assert_eq!(assignment, again);
    let fault = &assignment.faults.as_ref().unwrap()[0];
    assert_eq!(fault.value, Some(500.0));
    assert!(fault.pin_id.is_some() && fault.frame_id.is_some());
}

#[test]
fn a_leak_is_checked_against_the_circuit_when_the_file_loads() {
    let cases: Vec<(&str, Value, (AssignmentErrorCode, &str))> = vec![
        (
            "a pin that the part does not have",
            with_fault(|fault| fault["pinId"] = json!("pe")),
            (UnknownReference, "/faults/0/pinId"),
        ),
        (
            "a pin that is not a pin id",
            with_fault(|fault| fault["pinId"] = json!("nowhere")),
            (UnknownReference, "/faults/0/pinId"),
        ),
        (
            "a frame that is not in the circuit",
            with_fault(|fault| {
                fault["frameId"] = json!("00000000-0000-4000-8000-0000000000ff");
            }),
            (UnknownReference, "/faults/0/frameId"),
        ),
        (
            "a frame that is not an exposed part",
            with_fault(|fault| fault["frameId"] = json!(SOURCE)),
            (UnknownReference, "/faults/0/frameId"),
        ),
        (
            "no pin",
            with_fault(|fault| remove(fault, "pinId")),
            (InvalidField, "/faults/0/pinId"),
        ),
        (
            "no frame",
            with_fault(|fault| remove(fault, "frameId")),
            (InvalidField, "/faults/0/frameId"),
        ),
        (
            "no resistance",
            with_fault(|fault| remove(fault, "value")),
            (InvalidField, "/faults/0/value"),
        ),
        (
            "a resistance below the bolted-fault floor",
            with_fault(|fault| fault["value"] = json!(0.0005)),
            (InvalidField, "/faults/0/value"),
        ),
        (
            "a resistance above the largest",
            with_fault(|fault| fault["value"] = json!(2.0e9)),
            (InvalidField, "/faults/0/value"),
        ),
        (
            "a parameter id on a leak",
            with_fault(|fault| fault["parameterId"] = json!("resistance_ohms")),
            (InvalidField, "/faults/0/parameterId"),
        ),
        (
            "a pin on an open fault",
            with_fault(|fault| fault["kind"] = json!("open")),
            (InvalidField, "/faults/0/pinId"),
        ),
        (
            "a leak on a part the contract does not list",
            with_fault(|fault| {
                fault["componentId"] = json!(ELECTRODE);
                fault["pinId"] = json!("pe");
            }),
            (InvalidField, "/faults/0"),
        ),
    ];

    for (what, document, expected) in cases {
        let found = errors(&document);
        assert!(
            found.contains(&at(expected.0, expected.1)),
            "{what}: expected {expected:?} among {found:?}"
        );
    }
}

#[test]
fn the_frame_of_a_leak_is_locked_as_the_faulted_part_is() {
    let mut document = file();
    document["restrictions"]["lockedComponents"] = json!([SOURCE, LOAD, ELECTRODE]);

    assert_eq!(
        errors(&document),
        vec![at(TaskMismatch, "/faults/0/frameId")]
    );
}

#[test]
fn a_fault_entry_still_refuses_unknown_fields() {
    let document = with_fault(|fault| fault["extra"] = json!(1));

    assert_eq!(errors(&document), vec![at(UnknownField, "/faults/0/extra")]);
}

#[test]
fn the_new_devices_are_open_to_a_fault_finding_task_only() {
    let mut build = file();
    build["task"] = json!({"kind": "build"});
    build.as_object_mut().unwrap().remove("faults");
    build["checks"] = json!([]);
    let found = errors(&build);

    // Every device of the starting circuit is refused in a build task, as before.
    for index in 0..4 {
        let path = format!("/startingCircuit/components/{index}");
        assert!(
            found.contains(&at(InvalidField, &path)),
            "component {index} in a build task: {found:?}"
        );
    }
    // And the restriction list of a build task cannot name them.
    let mut restricted = file();
    restricted["task"] = json!({"kind": "build"});
    restricted.as_object_mut().unwrap().remove("faults");
    restricted["restrictions"]["allowedComponentTypes"] = json!(["exposed_part"]);
    assert!(errors(&restricted).contains(&at(
        UnknownReference,
        "/restrictions/allowedComponentTypes/0"
    )));
}

#[test]
fn a_fault_finding_task_may_offer_the_new_devices_to_the_student() {
    let mut document = file();
    document["restrictions"]["allowedComponentTypes"] =
        json!(["ac_voltmeter", "ac_ammeter", "rcd_two_pole", "exposed_part"]);

    assert_eq!(errors(&document), Vec::new());
}

fn diagnosis(entries: Value) -> Result<DiagnosisResult, Vec<String>> {
    let assignment = decode_assignment_json(FILE.as_bytes()).unwrap();
    check_diagnosis(&assignment, &entries).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| format!("{:?} {}", error.code, error.path))
            .collect()
    })
}

#[test]
fn a_diagnosis_names_a_leak_by_its_part_and_pin() {
    let result = diagnosis(json!([{"componentId": LOAD, "kind": "leak", "pinId": "l1"}])).unwrap();

    assert_eq!(result.outcome, DiagnosisOutcome::Passed);
    assert_eq!(result.entries[0].verdict, DiagnosisEntryVerdict::Correct);
    assert_eq!(result.faults[0].verdict, DiagnosisFaultVerdict::Found);
}

#[test]
fn the_right_part_with_another_pin_or_kind_is_wrong_kind_not_incorrect() {
    for entry in [
        json!({"componentId": LOAD, "kind": "leak", "pinId": "l2"}),
        json!({"componentId": LOAD, "kind": "open"}),
    ] {
        let result = diagnosis(json!([entry])).unwrap();

        assert_eq!(result.entries[0].verdict, DiagnosisEntryVerdict::WrongKind);
        assert_eq!(result.faults[0].verdict, DiagnosisFaultVerdict::Missed);
        assert_eq!(result.outcome, DiagnosisOutcome::Partial);
    }
}

#[test]
fn a_leak_on_another_part_is_incorrect() {
    let result =
        diagnosis(json!([{"componentId": SOURCE, "kind": "leak", "pinId": "l1"}])).unwrap();

    assert_eq!(result.entries[0].verdict, DiagnosisEntryVerdict::Incorrect);
    assert_eq!(result.outcome, DiagnosisOutcome::Failed);
}

#[test]
fn a_diagnosis_entry_is_refused_with_a_pointer_when_the_pin_is_wrong() {
    let cases = [
        (
            "a leak without a pin",
            json!({"componentId": LOAD, "kind": "leak"}),
            "InvalidField /0/pinId",
        ),
        (
            "a pin the part does not have",
            json!({"componentId": LOAD, "kind": "leak", "pinId": "pe"}),
            "InvalidField /0/pinId",
        ),
        (
            "a pin on an open entry",
            json!({"componentId": LOAD, "kind": "open", "pinId": "l1"}),
            "InvalidField /0/pinId",
        ),
        (
            "a parameter id on a leak",
            json!({"componentId": LOAD, "kind": "leak", "pinId": "l1", "parameterId": "resistance_ohms"}),
            "InvalidField /0/parameterId",
        ),
        (
            "a frame the student may not name",
            json!({"componentId": LOAD, "kind": "leak", "pinId": "l1", "frameId": FRAME}),
            "UnknownField /0/frameId",
        ),
    ];

    for (what, entry, expected) in cases {
        let errors = diagnosis(json!([entry])).expect_err(what);
        assert!(errors.contains(&expected.to_owned()), "{what}: {errors:?}");
    }
}

#[test]
fn legacy_faults_and_diagnoses_keep_their_meaning() {
    let legacy: Value = serde_json::from_str(LEGACY).unwrap();
    let assignment = validate_assignment_value(&legacy).expect("the legacy file loads");
    let result = check_diagnosis(
        &assignment,
        &json!([{"componentId": "00000000-0000-4000-8000-000000000003", "kind": "open"}]),
    )
    .unwrap();

    assert_eq!(result.outcome, DiagnosisOutcome::Passed);
}
