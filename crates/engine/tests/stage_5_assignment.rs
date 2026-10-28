//! Assignment files (docs/contracts/stage-5-assignments.md sections 2, 3, 5.7 and 7.2): the
//! canonical files in `tests/golden/stage-5-*.ocassignment`, the validation of every rule with
//! its code and JSON pointer, the student's copy, and the comparison of a diagnosis.

use open_circuit_engine::{
    Assignment, AssignmentError, AssignmentErrorCode, AssignmentTask, CheckCriterion,
    DiagnosisEntryVerdict, DiagnosisFaultVerdict, DiagnosisOutcome, MAX_ASSIGNMENT_BYTES,
    RevealPolicy, check_diagnosis, decode_assignment_json, encode_assignment_json,
    validate_assignment, validate_assignment_value,
};
use serde_json::{Value, json};

const DIVIDER: &str = include_str!("../../../tests/golden/stage-5-divider-build.ocassignment");
const FUSE: &str = include_str!("../../../tests/golden/stage-5-fuse-state.ocassignment");
const FAULT: &str = include_str!("../../../tests/golden/stage-5-fault-finding.ocassignment");
const DIAGNOSES: &str = include_str!("../../../tests/golden/stage-5-fault-finding.expected.json");

use AssignmentErrorCode::{
    DuplicateId, InvalidField, Malformed, TaskMismatch, TooLarge, UnknownField, UnknownReference,
    UnsupportedVersion,
};

const V1: &str = "00000000-0000-4000-8000-000000000001";
const VM1: &str = "00000000-0000-4000-8000-000000000002";
const R1: &str = "00000000-0000-4000-8000-000000000002";
const R2: &str = "00000000-0000-4000-8000-000000000003";
const METER: &str = "00000000-0000-4000-8000-000000000004";

type Found = Vec<(AssignmentErrorCode, String)>;
type Case = (&'static str, Value, Found);
type PointerCase = (&'static str, &'static str, Value, Found);

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn divider() -> Value {
    parse(DIVIDER)
}

fn fault_finding() -> Value {
    parse(FAULT)
}

/// The errors of a document as `(code, pointer)` pairs.
fn errors(document: &Value) -> Vec<(AssignmentErrorCode, String)> {
    match validate_assignment_value(document) {
        Ok(_) => Vec::new(),
        Err(errors) => errors
            .into_iter()
            .map(|error| (error.code, error.path))
            .collect(),
    }
}

fn at(code: AssignmentErrorCode, path: &str) -> (AssignmentErrorCode, String) {
    (code, path.to_owned())
}

fn edited(mut document: Value, edit: impl FnOnce(&mut Value)) -> Value {
    edit(&mut document);
    document
}

fn set(document: &mut Value, pointer: &str, value: Value) {
    *document
        .pointer_mut(pointer)
        .unwrap_or_else(|| panic!("no {pointer}")) = value;
}

fn remove(document: &mut Value, pointer: &str) {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let parent = document.pointer_mut(parent).unwrap();
    match parent {
        Value::Object(object) => {
            object.remove(key).unwrap_or_else(|| panic!("no {pointer}"));
        }
        Value::Array(list) => {
            list.remove(key.parse().unwrap());
        }
        other => panic!("cannot remove from {other}"),
    }
}

#[test]
fn the_canonical_files_are_valid_and_round_trip() {
    for (name, text) in [("divider", DIVIDER), ("fuse", FUSE), ("fault", FAULT)] {
        let assignment = decode_assignment_json(text.as_bytes())
            .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
        let encoded = encode_assignment_json(&assignment).unwrap();

        assert!(encoded.ends_with(b"\n"), "{name}");
        assert_eq!(
            decode_assignment_json(&encoded).unwrap(),
            assignment,
            "{name}"
        );
        // Encoding is deterministic.
        assert_eq!(encode_assignment_json(&assignment).unwrap(), encoded);
        let written = String::from_utf8(encoded).unwrap();
        for computed in [
            "snapshot", "layout", "runId", "trace", "history", "measured",
        ] {
            assert!(!written.contains(computed), "{name} stores {computed}");
        }
    }

    let divider = decode_assignment_json(DIVIDER.as_bytes()).unwrap();
    assert_eq!(divider.task, AssignmentTask::Build);
    assert_eq!(divider.checks.as_ref().unwrap().len(), 3);
    assert!(matches!(
        divider.checks.as_ref().unwrap()[1],
        CheckCriterion::Range { .. }
    ));
    let fault = decode_assignment_json(FAULT.as_bytes()).unwrap();
    assert_eq!(
        fault.task,
        AssignmentTask::FindFault {
            reveal: RevealPolicy::AfterCheck
        }
    );
    assert_eq!(fault.faults.as_ref().unwrap().len(), 1);
}

#[test]
fn the_students_copy_is_the_same_file_without_the_reference() {
    let assignment = decode_assignment_json(DIVIDER.as_bytes()).unwrap();
    let copy = assignment.student_copy();

    assert!(assignment.reference.is_some());
    assert!(copy.reference.is_none());
    assert_eq!(
        Assignment {
            reference: assignment.reference.clone(),
            ..copy.clone()
        },
        assignment
    );
    validate_assignment(&copy).unwrap();
    let written = String::from_utf8(encode_assignment_json(&copy).unwrap()).unwrap();
    assert!(!written.contains("\"reference\""));
    // A file without a reference is complete: the starting circuit and the checks stay.
    assert!(copy.starting_circuit.is_some() && copy.checks.is_some());
}

#[test]
fn unreadable_unsupported_and_oversized_files_are_refused_with_one_error() {
    let code_of = |bytes: &[u8]| {
        let errors = decode_assignment_json(bytes).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        (
            errors[0].code,
            errors[0].path.clone(),
            errors[0].message_key.clone(),
        )
    };

    for bytes in [&b"{not json"[..], b"", b"[]", b"3", b"null", b"\"text\""] {
        assert_eq!(
            code_of(bytes),
            (
                Malformed,
                String::new(),
                "assignment.error.malformed".to_owned()
            )
        );
    }
    for version in [
        json!(2),
        json!(0),
        json!("1"),
        json!(1.5),
        json!(null),
        json!(-1),
    ] {
        let document = edited(divider(), |document| {
            set(document, "/schemaVersion", version)
        });
        assert_eq!(
            code_of(document.to_string().as_bytes()),
            (
                UnsupportedVersion,
                "/schemaVersion".to_owned(),
                "assignment.error.unsupportedVersion".to_owned()
            )
        );
    }
    let missing = edited(divider(), |document| remove(document, "/schemaVersion"));
    assert_eq!(
        code_of(missing.to_string().as_bytes()).0,
        UnsupportedVersion
    );

    // The size limit is on the bytes of the file, valid or not.
    assert_eq!(
        code_of(&vec![b' '; MAX_ASSIGNMENT_BYTES + 1]),
        (
            TooLarge,
            String::new(),
            "assignment.error.tooLarge".to_owned()
        )
    );
    let padded = format!("{}{}", DIVIDER, " ".repeat(MAX_ASSIGNMENT_BYTES));
    assert_eq!(code_of(padded.as_bytes()).0, TooLarge);
    let at_the_limit = format!(
        "{}{}",
        DIVIDER,
        " ".repeat(MAX_ASSIGNMENT_BYTES - DIVIDER.len())
    );
    assert_eq!(at_the_limit.len(), MAX_ASSIGNMENT_BYTES);
    decode_assignment_json(at_the_limit.as_bytes()).unwrap();
}

#[test]
fn unknown_fields_are_refused_wherever_they_are() {
    let cases: Vec<(&str, Value, &str)> = vec![
        ("/grade", divider(), "/grade"),
        ("/task/weight", divider(), "/task/weight"),
        ("/restrictions/palette", divider(), "/restrictions/palette"),
        ("/checks/0/weight", divider(), "/checks/0/weight"),
        (
            "/checks/0/signal/channel",
            divider(),
            "/checks/0/signal/channel",
        ),
        ("/faults/0/note", fault_finding(), "/faults/0/note"),
        (
            "/startingCircuit/runtimeState",
            divider(),
            "/startingCircuit/runtimeState",
        ),
    ];
    for (pointer, document, expected) in cases {
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        let document = edited(document, |document| {
            document.pointer_mut(parent).unwrap()[key] = json!(1);
        });
        let found = errors(&document);
        assert_eq!(found, vec![at(UnknownField, expected)], "{pointer}");
    }
}

#[test]
fn text_and_id_fields_have_limits() {
    let long = |length: usize| json!("x".repeat(length));
    let cases = vec![
        ("/id", json!("not-a-uuid"), vec![at(InvalidField, "/id")]),
        ("/id", json!(7), vec![at(InvalidField, "/id")]),
        ("/title", json!(""), vec![at(InvalidField, "/title")]),
        ("/title", long(121), vec![at(InvalidField, "/title")]),
        ("/title", json!("a\u{0}b"), vec![at(InvalidField, "/title")]),
        ("/title", long(120), vec![]),
        ("/title", json!("é".repeat(120)), vec![]),
        (
            "/instructions",
            long(8001),
            vec![at(InvalidField, "/instructions")],
        ),
        ("/instructions", long(8000), vec![]),
        ("/instructions", json!(""), vec![]),
        ("/instructions", json!("one\ntwo\r\n\tthree"), vec![]),
        (
            "/instructions",
            json!(["not text"]),
            vec![at(InvalidField, "/instructions")],
        ),
        (
            "/checks/0/label",
            long(121),
            vec![at(InvalidField, "/checks/0/label")],
        ),
        ("/checks/0/label", long(120), vec![]),
    ];
    for (pointer, value, expected) in cases {
        let document = edited(divider(), |document| set(document, pointer, value.clone()));
        assert_eq!(errors(&document), expected, "{pointer} = {value}");
    }
    for pointer in ["/id", "/title", "/instructions", "/task", "/checks"] {
        let document = edited(divider(), |document| remove(document, pointer));
        assert_eq!(
            errors(&document).len(),
            1,
            "{pointer} is required: {:?}",
            errors(&document)
        );
    }
}

#[test]
fn the_task_kind_decides_which_fields_exist() {
    let cases: Vec<Case> = vec![
        (
            "a build task with faults",
            edited(divider(), |document| {
                document["faults"] = fault_finding()["faults"].clone();
            }),
            vec![at(TaskMismatch, "/faults")],
        ),
        (
            "a build task with a reveal policy",
            edited(divider(), |document| {
                document["task"]["reveal"] = json!("never");
            }),
            vec![at(TaskMismatch, "/task/reveal")],
        ),
        (
            "a build task without checks",
            edited(divider(), |document| remove(document, "/checks")),
            vec![at(TaskMismatch, "/checks")],
        ),
        (
            "a fault-finding task with checks",
            edited(fault_finding(), |document| {
                document["checks"] = divider()["checks"].clone();
            }),
            vec![at(TaskMismatch, "/checks")],
        ),
        (
            "a fault-finding task without a reveal policy",
            edited(fault_finding(), |document| {
                remove(document, "/task/reveal");
            }),
            vec![at(TaskMismatch, "/task/reveal")],
        ),
        (
            "a fault-finding task with an unknown reveal policy",
            edited(fault_finding(), |document| {
                set(document, "/task/reveal", json!("always"));
            }),
            vec![at(InvalidField, "/task/reveal")],
        ),
        (
            "a fault-finding task without a starting circuit",
            edited(fault_finding(), |document| {
                remove(document, "/startingCircuit");
            }),
            // Its faults and locked parts then name nothing.
            vec![
                at(TaskMismatch, "/startingCircuit"),
                at(UnknownReference, "/restrictions/lockedComponents/0"),
                at(UnknownReference, "/restrictions/lockedComponents/1"),
                at(UnknownReference, "/restrictions/lockedComponents/2"),
                at(UnknownReference, "/restrictions/lockedComponents/3"),
                at(UnknownReference, "/faults/0/componentId"),
            ],
        ),
        (
            "a fault-finding task without faults",
            edited(fault_finding(), |document| remove(document, "/faults")),
            vec![at(TaskMismatch, "/faults")],
        ),
        (
            "a fault-finding task with an empty fault list",
            edited(fault_finding(), |document| {
                set(document, "/faults", json!([]))
            }),
            vec![at(InvalidField, "/faults")],
        ),
        (
            "an unknown task kind",
            edited(divider(), |document| {
                set(document, "/task/kind", json!("quiz"))
            }),
            vec![at(InvalidField, "/task/kind")],
        ),
        (
            "a build task without a starting circuit is allowed",
            edited(divider(), |document| {
                remove(document, "/startingCircuit");
                remove(document, "/restrictions");
                set(document, "/checks", json!([]));
                remove(document, "/reference");
            }),
            vec![],
        ),
        (
            "a faulted part that is not locked",
            edited(fault_finding(), |document| {
                set(
                    document,
                    "/restrictions/lockedComponents",
                    json!([V1, R1, VM1]),
                );
            }),
            vec![at(TaskMismatch, "/faults/0/componentId")],
        ),
        (
            "a faulted part and no restrictions at all",
            edited(fault_finding(), |document| {
                remove(document, "/restrictions")
            }),
            vec![at(TaskMismatch, "/faults/0/componentId")],
        ),
    ];
    for (name, document, expected) in cases {
        assert_eq!(errors(&document), expected, "{name}");
    }
}

#[test]
fn stable_ids_are_unique_within_their_space() {
    // A schematic's own IDs, across its components, wires, junctions, and markers.
    let marker_reuses_a_component = edited(divider(), |document| {
        let first = document["startingCircuit"]["components"][0]["id"].clone();
        set(document, "/startingCircuit/referenceMarkers/0/id", first);
    });
    assert_eq!(
        errors(&marker_reuses_a_component),
        vec![at(DuplicateId, "/startingCircuit/referenceMarkers/0/id")]
    );
    let reference_marker_reuses_a_component = edited(divider(), |document| {
        let first = document["reference"]["components"][0]["id"].clone();
        set(document, "/reference/referenceMarkers/0/id", first);
    });
    assert_eq!(
        errors(&reference_marker_reuses_a_component),
        vec![at(DuplicateId, "/reference/referenceMarkers/0/id")]
    );
    // The reference keeps the starting circuit's IDs: that is not a duplicate.
    assert_eq!(errors(&divider()), vec![]);

    // Criteria are unique among themselves and differ from every ID of both schematics.
    let two_criteria = edited(divider(), |document| {
        let first = document["checks"][0]["id"].clone();
        set(document, "/checks/1/id", first);
    });
    assert_eq!(errors(&two_criteria), vec![at(DuplicateId, "/checks/1/id")]);
    let reuses_a_component = edited(divider(), |document| {
        set(document, "/checks/0/id", json!(V1));
    });
    assert_eq!(
        errors(&reuses_a_component),
        vec![at(DuplicateId, "/checks/0/id")]
    );
    let reuses_a_reference_part = edited(divider(), |document| {
        set(
            document,
            "/checks/2/id",
            json!("00000000-0000-4000-8000-000000000011"),
        );
    });
    assert_eq!(
        errors(&reuses_a_reference_part),
        vec![at(DuplicateId, "/checks/2/id")]
    );

    // Faults, the same.
    let fault_reuses_a_component = edited(fault_finding(), |document| {
        set(document, "/faults/0/id", json!(R2));
    });
    assert_eq!(
        errors(&fault_reuses_a_component),
        vec![at(DuplicateId, "/faults/0/id")]
    );
    let two_faults_one_id = edited(fault_finding(), |document| {
        document["faults"] = json!([
            { "id": "00000000-0000-4000-8000-0000000000a1", "kind": "open", "componentId": R2 },
            { "id": "00000000-0000-4000-8000-0000000000a1", "kind": "open", "componentId": R1 },
        ]);
    });
    assert_eq!(
        errors(&two_faults_one_id),
        vec![at(DuplicateId, "/faults/1/id")]
    );
}

#[test]
fn references_must_resolve_in_the_starting_circuit_and_the_catalog() {
    let missing = "00000000-0000-4000-8000-0000000000ee";
    let cases: Vec<Case> = vec![
        (
            "a locked component that is not in the starting circuit",
            edited(divider(), |document| {
                set(
                    document,
                    "/restrictions/lockedComponents",
                    json!([V1, missing]),
                );
            }),
            vec![at(UnknownReference, "/restrictions/lockedComponents/1")],
        ),
        (
            "an unknown component type in the palette",
            edited(divider(), |document| {
                set(
                    document,
                    "/restrictions/allowedComponentTypes",
                    json!(["resistor", "transistor"]),
                );
            }),
            vec![at(
                UnknownReference,
                "/restrictions/allowedComponentTypes/1",
            )],
        ),
        (
            "an unknown component type in a count",
            edited(divider(), |document| {
                set(document, "/checks/2/typeId", json!("transistor"));
            }),
            vec![at(UnknownReference, "/checks/2/typeId")],
        ),
        (
            "a signal of a component that is not in the starting circuit",
            edited(divider(), |document| {
                set(
                    document,
                    "/checks/0/signal/key",
                    json!(format!("{missing}:voltage")),
                );
            }),
            vec![at(UnknownReference, "/checks/0/signal/key")],
        ),
        (
            "a signal the component's type does not have",
            edited(divider(), |document| {
                set(
                    document,
                    "/checks/0/signal/key",
                    json!(format!("{VM1}:accumulated_i2t")),
                );
            }),
            vec![at(UnknownReference, "/checks/0/signal/key")],
        ),
        (
            "a signal with the wrong separator",
            edited(divider(), |document| {
                set(
                    document,
                    "/checks/0/signal/key",
                    json!(format!("{VM1}/voltage")),
                );
            }),
            vec![at(UnknownReference, "/checks/0/signal/key")],
        ),
        (
            "a signal that is not a key at all",
            edited(divider(), |document| {
                set(document, "/checks/0/signal/key", json!("voltage"));
            }),
            vec![at(UnknownReference, "/checks/0/signal/key")],
        ),
        (
            "a signal with the unit of another slot",
            edited(divider(), |document| {
                set(document, "/checks/0/signal/unit", json!("amp"));
            }),
            vec![at(InvalidField, "/checks/0/signal/unit")],
        ),
        (
            "a signal with an unknown unit",
            edited(divider(), |document| {
                set(document, "/checks/0/signal/unit", json!("furlong"));
            }),
            vec![at(InvalidField, "/checks/0/signal/unit")],
        ),
        (
            "a fault on a component that is not in the starting circuit",
            edited(fault_finding(), |document| {
                set(document, "/faults/0/componentId", json!(missing));
            }),
            vec![at(UnknownReference, "/faults/0/componentId")],
        ),
    ];
    for (name, document, expected) in cases {
        assert_eq!(errors(&document), expected, "{name}");
    }

    // Every slot of every type is a valid signal, including a relay's six ports, a fuse's
    // accumulated I2t, and a motor's angular velocity.
    for (type_id, parameters, keys) in [
        (
            "relay_spdt",
            json!({
                "coilResistanceOhms": 100.0, "coilInductanceHenries": 0.01,
                "pickupCurrentAmps": 0.04, "dropoutCurrentAmps": 0.02,
                "closedContactResistanceOhms": 0.1
            }),
            vec![
                ("/coil/voltage", "volt"),
                ("/coil/current", "amp"),
                ("/normally_open/voltage", "volt"),
                ("/normally_open/current", "amp"),
                ("/normally_closed/voltage", "volt"),
                ("/normally_closed/current", "amp"),
            ],
        ),
        (
            "fuse",
            json!({ "intactResistanceOhms": 0.01, "tripI2tAmpereSquaredSeconds": 0.004 }),
            vec![
                (":voltage", "volt"),
                (":current", "amp"),
                ("/accumulated_i2t", "ampere_squared_second"),
            ],
        ),
        (
            "pmdc_motor",
            json!({
                "armatureResistanceOhms": 1.0, "armatureInductanceHenries": 0.001,
                "motorConstantNewtonMetersPerAmp": 0.05,
                "rotorInertiaKilogramMetersSquared": 0.0001,
                "viscousFrictionNewtonMeterSecondsPerRadian": 0.0001,
                "loadTorqueNewtonMeters": 0.0, "initialAngularVelocityRadiansPerSecond": 0.0
            }),
            vec![
                (":voltage", "volt"),
                (":current", "amp"),
                ("/angular_velocity", "radian_per_second"),
            ],
        ),
    ] {
        for (suffix, unit) in keys {
            let document = edited(divider(), |document| {
                document["startingCircuit"]["components"][1] = json!({
                    "id": VM1, "typeId": type_id, "position": { "x": 0, "y": 0 },
                    "rotationDeg": 0, "parameters": parameters
                });
                document["reference"]["components"][1] =
                    document["startingCircuit"]["components"][1].clone();
                document["checks"] = json!([{
                    "kind": "value", "id": "00000000-0000-4000-8000-0000000000c1",
                    "signal": { "key": format!("{VM1}{suffix}"), "unit": unit },
                    "atSeconds": 0, "expected": 1.0, "toleranceAbs": 0.1
                }]);
            });
            assert_eq!(errors(&document), vec![], "{type_id}{suffix}");
        }
    }
}

#[test]
fn criteria_fields_follow_the_contracts_limits() {
    let cases: Vec<PointerCase> = vec![
        // Times are non-negative multiples of the public step, at most 60 s.
        (
            "time",
            "/checks/0/atSeconds",
            json!(0.0005),
            vec![at(InvalidField, "/checks/0/atSeconds")],
        ),
        (
            "time",
            "/checks/0/atSeconds",
            json!(-0.001),
            vec![at(InvalidField, "/checks/0/atSeconds")],
        ),
        (
            "time",
            "/checks/0/atSeconds",
            json!(60.001),
            vec![at(InvalidField, "/checks/0/atSeconds")],
        ),
        (
            "time",
            "/checks/0/atSeconds",
            json!("0"),
            vec![at(InvalidField, "/checks/0/atSeconds")],
        ),
        ("time", "/checks/0/atSeconds", json!(60), vec![]),
        ("time", "/checks/0/atSeconds", json!(0.3), vec![]),
        ("time", "/checks/0/atSeconds", json!(0.002), vec![]),
        // Tolerances.
        (
            "tolerance",
            "/checks/0/toleranceAbs",
            json!(-1e-9),
            vec![at(InvalidField, "/checks/0/toleranceAbs")],
        ),
        ("tolerance", "/checks/0/toleranceAbs", json!(0), vec![]),
        (
            "tolerance",
            "/checks/0/toleranceRel",
            json!(-0.1),
            vec![at(InvalidField, "/checks/0/toleranceRel")],
        ),
        (
            "tolerance",
            "/checks/0/expected",
            json!("2.5"),
            vec![at(InvalidField, "/checks/0/expected")],
        ),
        // A range: from <= to, min <= max.
        (
            "range",
            "/checks/1/fromSeconds",
            json!(0.02),
            vec![at(InvalidField, "/checks/1/toSeconds")],
        ),
        (
            "range",
            "/checks/1/min",
            json!(2.7),
            vec![at(InvalidField, "/checks/1/max")],
        ),
        ("range", "/checks/1/min", json!(2.6), vec![]),
        ("range", "/checks/1/fromSeconds", json!(0.01), vec![]),
        // A count: integers from 0 to 1000, max >= min.
        (
            "count",
            "/checks/2/min",
            json!(3),
            vec![at(InvalidField, "/checks/2/max")],
        ),
        (
            "count",
            "/checks/2/min",
            json!(1001),
            vec![at(InvalidField, "/checks/2/min")],
        ),
        (
            "count",
            "/checks/2/max",
            json!(1001),
            vec![at(InvalidField, "/checks/2/max")],
        ),
        ("count", "/checks/2/max", json!(1000), vec![]),
        (
            "count",
            "/checks/2/min",
            json!(-1),
            vec![at(InvalidField, "/checks/2/min")],
        ),
        (
            "count",
            "/checks/2/min",
            json!(1.5),
            vec![at(InvalidField, "/checks/2/min")],
        ),
        ("count", "/checks/2/min", json!(0), vec![]),
        // The kind.
        (
            "kind",
            "/checks/0/kind",
            json!("average"),
            vec![at(InvalidField, "/checks/0/kind")],
        ),
    ];
    for (group, pointer, value, expected) in cases {
        let document = edited(divider(), |document| set(document, pointer, value.clone()));
        assert_eq!(errors(&document), expected, "{group}: {pointer} = {value}");
    }

    // A count may leave `max` out, and a value its `toleranceRel`.
    let relaxed = edited(divider(), |document| {
        remove(document, "/checks/2/max");
        remove(document, "/checks/0/toleranceRel");
    });
    assert_eq!(errors(&relaxed), vec![]);
}

#[test]
fn a_device_state_names_a_relay_or_a_fuse_and_one_of_its_states() {
    let fuse = |edit: &dyn Fn(&mut Value)| edited(parse(FUSE), |document| edit(document));
    let f1 = "00000000-0000-4000-8000-000000000003";

    assert_eq!(errors(&parse(FUSE)), vec![]);
    for state in ["intact", "tripped"] {
        let document = fuse(&|document| set(document, "/checks/0/state", json!(state)));
        assert_eq!(errors(&document), vec![], "{state}");
    }
    for state in ["released", "picked", "blown", ""] {
        let document = fuse(&|document| set(document, "/checks/0/state", json!(state)));
        assert_eq!(
            errors(&document),
            vec![at(InvalidField, "/checks/0/state")],
            "{state}"
        );
    }
    // A component of another type.
    let on_the_source = fuse(&|document| {
        set(document, "/checks/0/componentId", json!(V1));
    });
    assert_eq!(
        errors(&on_the_source),
        vec![at(InvalidField, "/checks/0/componentId")]
    );
    let missing = fuse(&|document| {
        set(
            document,
            "/checks/0/componentId",
            json!("00000000-0000-4000-8000-0000000000ee"),
        );
    });
    assert_eq!(
        errors(&missing),
        vec![at(UnknownReference, "/checks/0/componentId"),]
    );

    // A relay accepts its own states.
    let relay = |state: &str| {
        fuse(&|document| {
            document["startingCircuit"]["components"][1] = json!({
                "id": f1, "typeId": "relay_spdt", "position": { "x": 2, "y": 0 }, "rotationDeg": 0,
                "parameters": {
                    "coilResistanceOhms": 100.0, "coilInductanceHenries": 0.01,
                    "pickupCurrentAmps": 0.04, "dropoutCurrentAmps": 0.02,
                    "closedContactResistanceOhms": 0.1
                }
            });
            document["reference"]["components"][1] =
                document["startingCircuit"]["components"][1].clone();
            set(document, "/checks/0/state", json!(state));
        })
    };
    assert_eq!(errors(&relay("picked")), vec![]);
    assert_eq!(errors(&relay("released")), vec![]);
    assert_eq!(
        errors(&relay("tripped")),
        vec![at(InvalidField, "/checks/0/state")]
    );
}

#[test]
fn limits_on_lists_are_too_large_and_never_a_partial_read() {
    let check = |index: usize| {
        json!({
            "kind": "componentCount",
            "id": format!("00000000-0000-4000-8000-{:012x}", 0x1000 + index),
            "typeId": "resistor", "min": 0
        })
    };
    let with_checks = |count: usize| {
        edited(divider(), |document| {
            document["checks"] = Value::Array((0..count).map(check).collect());
        })
    };
    assert_eq!(errors(&with_checks(64)), vec![]);
    assert_eq!(errors(&with_checks(65)), vec![at(TooLarge, "/checks")]);

    let fault = |index: usize, component: &str| {
        json!({
            "id": format!("00000000-0000-4000-8000-{:012x}", 0x2000 + index),
            "kind": "open", "componentId": component
        })
    };
    let with_faults = |count: usize| {
        edited(fault_finding(), |document| {
            // Eight faults on the four parts is more than one fault per component, so the
            // list limit is judged first.
            document["faults"] = Value::Array((0..count).map(|index| fault(index, R2)).collect());
        })
    };
    assert_eq!(errors(&with_faults(9)), vec![at(TooLarge, "/faults")]);
    // Eight faults pass the limit; the second fault on a component is then refused.
    assert_eq!(errors(&with_faults(8)).len(), 7);

    let types = |count: usize| {
        edited(divider(), |document| {
            set(
                document,
                "/restrictions/allowedComponentTypes",
                Value::Array(vec![json!("resistor"); count]),
            );
        })
    };
    assert_eq!(errors(&types(9)), vec![]);
    assert_eq!(
        errors(&types(10)),
        vec![at(TooLarge, "/restrictions/allowedComponentTypes")]
    );
    // `null` is every type.
    let null_types = edited(divider(), |document| {
        set(document, "/restrictions/allowedComponentTypes", Value::Null);
    });
    assert_eq!(errors(&null_types), vec![]);
}

#[test]
fn schematics_are_valid_version_1_documents() {
    let cases: Vec<Case> = vec![
        (
            "a parameter outside its domain",
            edited(divider(), |document| {
                set(
                    document,
                    "/startingCircuit/components/1/parameters/inputResistanceOhms",
                    json!(0),
                );
            }),
            vec![at(InvalidField, "/startingCircuit/components/1")],
        ),
        (
            "an unsupported schematic version",
            edited(divider(), |document| {
                set(document, "/reference/schemaVersion", json!(2));
            }),
            vec![at(UnsupportedVersion, "/reference/schemaVersion")],
        ),
        (
            "an invalid ID",
            edited(divider(), |document| {
                set(
                    document,
                    "/startingCircuit/referenceMarkers/0/id",
                    json!("nope"),
                );
            }),
            vec![at(InvalidField, "/startingCircuit/referenceMarkers/0/id")],
        ),
        (
            "a schematic that is not an object",
            edited(divider(), |document| set(document, "/reference", json!([]))),
            vec![at(InvalidField, "/reference")],
        ),
        (
            "an unknown component type",
            edited(divider(), |document| {
                set(
                    document,
                    "/reference/components/0/typeId",
                    json!("transistor"),
                );
            }),
            vec![at(InvalidField, "/reference")],
        ),
        (
            "an invalid rotation",
            edited(divider(), |document| {
                set(document, "/reference/components/0/rotationDeg", json!(45));
            }),
            vec![at(InvalidField, "/reference/components/0")],
        ),
    ];
    for (name, document, expected) in cases {
        assert_eq!(errors(&document), expected, "{name}");
    }

    // An electrically incomplete circuit is still a valid file: nothing is solved.
    let open_circuit = edited(divider(), |document| {
        set(document, "/startingCircuit/components", json!([]));
        set(document, "/restrictions/lockedComponents", json!([]));
        set(document, "/checks", json!([]));
        remove(document, "/reference");
    });
    assert_eq!(errors(&open_circuit), vec![]);
}

#[test]
fn faults_are_judged_on_the_starting_circuit_without_solving_it() {
    let fault = |kind: &str, component: &str, extra: Value| {
        let mut fault = json!({
            "id": "00000000-0000-4000-8000-0000000000a1",
            "kind": kind, "componentId": component
        });
        for (key, value) in extra.as_object().cloned().unwrap_or_default() {
            fault[key] = value;
        }
        edited(fault_finding(), |document| {
            document["faults"] = json!([fault]);
        })
    };

    assert_eq!(errors(&fault("open", R2, json!({}))), vec![]);
    assert_eq!(errors(&fault("short", R2, json!({}))), vec![]);
    assert_eq!(
        errors(&fault(
            "parameter",
            R2,
            json!({ "parameterId": "resistanceOhms", "value": 3000.0 })
        )),
        vec![]
    );
    // An open source is a valid fault that makes the circuit unsolvable: a file does not solve.
    assert_eq!(errors(&fault("open", V1, json!({}))), vec![]);

    let invalid: Vec<(&str, Value)> = vec![
        ("a short on a source", fault("short", V1, json!({}))),
        ("an open voltmeter", fault("open", METER, json!({}))),
        (
            "a value outside the domain",
            fault(
                "parameter",
                R2,
                json!({ "parameterId": "resistanceOhms", "value": 0.0 }),
            ),
        ),
        (
            "a parameter the type does not have",
            fault(
                "parameter",
                R2,
                json!({ "parameterId": "voltageVolts", "value": 5.0 }),
            ),
        ),
        (
            "a parameter fault without a parameter",
            fault("parameter", R2, json!({ "value": 1.0 })),
        ),
        (
            "a parameter fault without a value",
            fault("parameter", R2, json!({ "parameterId": "resistanceOhms" })),
        ),
        (
            "an open fault with a value",
            fault("open", R2, json!({ "value": 1.0 })),
        ),
        (
            "an open fault with a parameter",
            fault("open", R2, json!({ "parameterId": "resistanceOhms" })),
        ),
        ("an unknown kind", fault("melt", R2, json!({}))),
    ];
    for (name, document) in invalid {
        let found = errors(&document);
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(found[0].0, InvalidField, "{name}");
        assert!(found[0].1.starts_with("/faults/0"), "{name}: {found:?}");
    }

    // Two faults on one component.
    let two_on_one = edited(fault_finding(), |document| {
        document["faults"] = json!([
            { "id": "00000000-0000-4000-8000-0000000000a1", "kind": "open", "componentId": R2 },
            { "id": "00000000-0000-4000-8000-0000000000a2", "kind": "short", "componentId": R2 },
        ]);
    });
    assert_eq!(errors(&two_on_one), vec![at(InvalidField, "/faults/0")]);
}

#[test]
fn errors_come_in_the_order_of_the_contract_and_are_capped_at_32() {
    let document = edited(divider(), |document| {
        set(document, "/title", json!(""));
        set(document, "/restrictions/lockedComponents", json!(["x"]));
        set(document, "/checks/1/min", json!("low"));
        set(document, "/checks/0/atSeconds", json!(0.0005));
        document["grade"] = json!(1);
        set(document, "/id", json!("nope"));
    });
    assert_eq!(
        errors(&document),
        vec![
            at(UnknownField, "/grade"),
            at(InvalidField, "/id"),
            at(InvalidField, "/title"),
            at(InvalidField, "/restrictions/lockedComponents/0"),
            at(InvalidField, "/checks/0/atSeconds"),
            at(InvalidField, "/checks/1/min"),
        ]
    );

    let many = edited(divider(), |document| {
        document["checks"] = Value::Array(
            (0..40)
                .map(|_| json!({ "kind": "componentCount", "id": "bad", "typeId": "resistor", "min": 0 }))
                .collect(),
        );
    });
    let found = errors(&many);
    assert_eq!(found.len(), 32);
    assert_eq!(found[0], at(InvalidField, "/checks/0/id"));
    assert_eq!(found[31], at(InvalidField, "/checks/31/id"));
}

#[test]
fn the_message_key_of_an_error_spells_its_code() {
    for (code, key) in [
        (Malformed, "assignment.error.malformed"),
        (UnsupportedVersion, "assignment.error.unsupportedVersion"),
        (UnknownField, "assignment.error.unknownField"),
        (InvalidField, "assignment.error.invalidField"),
        (DuplicateId, "assignment.error.duplicateId"),
        (UnknownReference, "assignment.error.unknownReference"),
        (TaskMismatch, "assignment.error.taskMismatch"),
        (TooLarge, "assignment.error.tooLarge"),
    ] {
        assert_eq!(AssignmentError::new(code, "/x").message_key, key);
    }
    let error = serde_json::to_value(AssignmentError::new(UnknownField, "/grade")).unwrap();
    assert_eq!(
        error,
        json!({ "code": "UnknownField", "path": "/grade", "messageKey": "assignment.error.unknownField" })
    );
}

#[test]
fn saving_validates_first_and_refuses_an_invalid_or_oversized_assignment() {
    let valid = decode_assignment_json(DIVIDER.as_bytes()).unwrap();

    let mut untitled = valid.clone();
    untitled.title.clear();
    assert_eq!(
        encode_assignment_json(&untitled).unwrap_err(),
        vec![AssignmentError::new(InvalidField, "/title")]
    );
    let mut faulty = valid.clone();
    faulty.faults = fault_finding_assignment().faults;
    assert_eq!(
        encode_assignment_json(&faulty).unwrap_err(),
        vec![AssignmentError::new(TaskMismatch, "/faults")]
    );
    // Many long labels cannot push a file past the limit: the instructions bound it.
    let mut large = valid;
    large.instructions = "é".repeat(8000);
    assert!(encode_assignment_json(&large).is_ok());
}

fn fault_finding_assignment() -> Assignment {
    decode_assignment_json(FAULT.as_bytes()).unwrap()
}

fn entries(submitted: &Value) -> Value {
    // The golden file names components by their short names.
    let id = |name: &str| match name {
        "R1" => R1,
        "R2" => R2,
        other => panic!("no component {other}"),
    };
    Value::Array(
        submitted
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                json!({ "componentId": id(entry["component"].as_str().unwrap()), "kind": entry["kind"] })
            })
            .collect(),
    )
}

#[test]
fn the_diagnoses_of_the_golden_fixture_get_their_verdicts() {
    let golden = parse(DIAGNOSES);
    let assignment = fault_finding_assignment();
    let fault_id = assignment.faults.as_ref().unwrap()[0].id.clone();

    for diagnosis in golden["diagnoses"].as_array().unwrap() {
        let name = diagnosis["name"].as_str().unwrap();
        let expected = &diagnosis["expected"];
        let result = check_diagnosis(&assignment, &entries(&diagnosis["submitted"]))
            .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));

        let outcome = match result.outcome {
            DiagnosisOutcome::Passed => "passed",
            DiagnosisOutcome::Partial => "partial",
            DiagnosisOutcome::Failed => "failed",
        };
        assert_eq!(outcome, expected["outcome"], "{name}");
        assert_eq!(result.outcome_key, format!("diagnosis.outcome.{outcome}"));
        let wanted = expected["entries"].as_array().unwrap();
        assert_eq!(result.entries.len(), wanted.len(), "{name}");
        for (found, wanted) in result.entries.iter().zip(wanted) {
            let verdict = match found.verdict {
                DiagnosisEntryVerdict::Correct => "correct",
                DiagnosisEntryVerdict::WrongKind => "wrongKind",
                DiagnosisEntryVerdict::Incorrect => "incorrect",
            };
            assert_eq!(verdict, wanted["verdict"], "{name}");
            assert_eq!(found.message_key, format!("diagnosis.entry.{verdict}"));
        }
        assert_eq!(result.faults.len(), 1);
        assert_eq!(result.faults[0].fault_id, fault_id);
        let found = match result.faults[0].verdict {
            DiagnosisFaultVerdict::Found => "found",
            DiagnosisFaultVerdict::Missed => "missed",
        };
        assert_eq!(found, expected["faults"][0]["verdict"], "{name}");
        assert_eq!(
            result.faults[0].message_key,
            format!("diagnosis.fault.{found}")
        );
    }
}

#[test]
fn a_parameter_diagnosis_needs_the_same_parameter_and_duplicates_count_once() {
    let mut document = fault_finding();
    document["faults"] = json!([
        { "id": "00000000-0000-4000-8000-0000000000a1", "kind": "parameter",
          "componentId": V1, "parameterId": "voltageVolts", "value": 12.0 },
        { "id": "00000000-0000-4000-8000-0000000000a2", "kind": "open", "componentId": R1 },
    ]);
    let assignment = validate_assignment_value(&document).unwrap();
    let outcome = |diagnosis: Value| check_diagnosis(&assignment, &diagnosis).unwrap();

    let both = outcome(json!([
        { "componentId": V1, "kind": "parameter", "parameterId": "voltageVolts" },
        { "componentId": R1, "kind": "open" },
    ]));
    assert_eq!(both.outcome, DiagnosisOutcome::Passed);

    // The same entry twice changes nothing: every fault is found once.
    let twice = outcome(json!([
        { "componentId": R1, "kind": "open" },
        { "componentId": R1, "kind": "open" },
        { "componentId": V1, "kind": "parameter", "parameterId": "voltageVolts" },
    ]));
    assert_eq!(twice.outcome, DiagnosisOutcome::Passed);
    assert_eq!(twice.entries.len(), 3);
    assert_eq!(twice.faults.len(), 2);
    assert!(
        twice
            .faults
            .iter()
            .all(|fault| fault.verdict == DiagnosisFaultVerdict::Found)
    );

    // Another parameter of the same part is the wrong kind of fault, and the fault is missed.
    let other_parameter = outcome(json!([
        { "componentId": V1, "kind": "parameter", "parameterId": "internalResistanceOhms" }
    ]));
    assert_eq!(
        other_parameter.entries[0].verdict,
        DiagnosisEntryVerdict::WrongKind
    );
    assert_eq!(
        other_parameter.faults[0].verdict,
        DiagnosisFaultVerdict::Missed
    );
    assert_eq!(other_parameter.outcome, DiagnosisOutcome::Partial);
}

#[test]
fn a_diagnosis_is_validated_with_pointers_into_the_diagnosis() {
    let assignment = fault_finding_assignment();
    let missing = "00000000-0000-4000-8000-0000000000ee";
    let errors_of = |diagnosis: Value| {
        check_diagnosis(&assignment, &diagnosis)
            .unwrap_err()
            .into_iter()
            .map(|error| (error.code, error.path))
            .collect::<Vec<_>>()
    };

    assert_eq!(errors_of(json!({})), vec![at(InvalidField, "")]);
    assert_eq!(
        errors_of(Value::Array(vec![
            json!({ "componentId": R2, "kind": "open" });
            9
        ])),
        vec![at(TooLarge, "")]
    );
    assert!(
        check_diagnosis(
            &assignment,
            &Value::Array(vec![json!({ "componentId": R2, "kind": "open" }); 8])
        )
        .is_ok()
    );
    assert_eq!(errors_of(json!([7])), vec![at(InvalidField, "/0")]);
    assert_eq!(
        errors_of(json!([{ "componentId": missing, "kind": "open" }])),
        vec![at(UnknownReference, "/0/componentId")]
    );
    assert_eq!(
        errors_of(json!([{ "componentId": "nope", "kind": "open" }])),
        vec![at(InvalidField, "/0/componentId")]
    );
    assert_eq!(
        errors_of(json!([{ "kind": "open" }])),
        vec![at(InvalidField, "/0/componentId")]
    );
    assert_eq!(
        errors_of(json!([{ "componentId": R2, "kind": "melt" }])),
        vec![at(InvalidField, "/0/kind")]
    );
    assert_eq!(
        errors_of(json!([{ "componentId": R2, "kind": "open", "value": 1 }])),
        vec![at(UnknownField, "/0/value")]
    );
    // A parameter entry needs a parameter of the component's type, and an open one carries none.
    assert_eq!(
        errors_of(json!([{ "componentId": R2, "kind": "parameter" }])),
        vec![at(InvalidField, "/0/parameterId")]
    );
    assert_eq!(
        errors_of(
            json!([{ "componentId": R2, "kind": "parameter", "parameterId": "voltageVolts" }])
        ),
        vec![at(InvalidField, "/0/parameterId")]
    );
    assert_eq!(
        errors_of(json!([{ "componentId": R2, "kind": "open", "parameterId": "resistanceOhms" }])),
        vec![at(InvalidField, "/0/parameterId")]
    );
    // Several problems are reported, each at its entry.
    assert_eq!(
        errors_of(json!([
            { "componentId": missing, "kind": "open" },
            { "componentId": R2, "kind": "melt" },
        ])),
        vec![
            at(UnknownReference, "/0/componentId"),
            at(InvalidField, "/1/kind"),
        ]
    );

    // A build task has no faults to compare with.
    let build = decode_assignment_json(DIVIDER.as_bytes()).unwrap();
    assert_eq!(
        check_diagnosis(&build, &json!([])).unwrap_err(),
        vec![AssignmentError::new(TaskMismatch, "/task/kind")]
    );
}

#[test]
fn errors_of_the_reference_come_last_although_its_ids_are_read_first() {
    let document = edited(divider(), |document| {
        // A reference that is not a valid schematic, a criterion with a bad ID, and a bad title.
        set(document, "/reference/components/0/rotationDeg", json!(45));
        set(document, "/checks/0/id", json!("bad"));
        set(document, "/title", json!(""));
    });

    assert_eq!(
        errors(&document),
        vec![
            at(InvalidField, "/title"),
            at(InvalidField, "/checks/0/id"),
            at(InvalidField, "/reference/components/0"),
        ]
    );

    // The reference's IDs still count: a criterion may not reuse one.
    let reuse = edited(divider(), |document| {
        set(
            document,
            "/checks/0/id",
            json!("00000000-0000-4000-8000-000000000011"),
        );
        set(document, "/reference/components/0/rotationDeg", json!(45));
    });
    assert_eq!(
        errors(&reuse),
        vec![
            at(DuplicateId, "/checks/0/id"),
            at(InvalidField, "/reference/components/0"),
        ]
    );
}

#[test]
fn a_label_may_hold_a_tab_or_a_line_break() {
    let document = edited(divider(), |document| {
        set(
            document,
            "/checks/0/label",
            json!("Reading\tat the output\nnode"),
        );
    });

    assert_eq!(errors(&document), vec![]);
}

#[test]
fn a_diagnosis_is_refused_for_an_assignment_that_is_no_longer_valid() {
    let mut assignment = fault_finding_assignment();
    assignment.restrictions = None;
    let refused = check_diagnosis(&assignment, &json!([])).unwrap_err();
    assert_eq!(
        refused,
        vec![AssignmentError::new(TaskMismatch, "/faults/0/componentId")]
    );

    let mut other_version = fault_finding_assignment();
    other_version.schema_version = 2;
    assert_eq!(
        check_diagnosis(&other_version, &json!([])).unwrap_err(),
        vec![AssignmentError::new(UnsupportedVersion, "/schemaVersion")]
    );
}
