use open_circuit_engine::{
    Assignment, CheckCriterion, CheckRequest, CheckVerdict, ComponentTypeId, GradedCheckRequest,
    StableId, decode_assignment_json, encode_assignment_json, evaluate_checks,
    evaluate_graded_checks,
};
use serde_json::{Value, json};

const LEGACY: &str = include_str!("../../../tests/golden/stage-5-divider-build.ocassignment");
const DRAWING: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");

fn graded_document() -> Value {
    let mut file: Value = serde_json::from_str(LEGACY).unwrap();
    let rows: Vec<_> = file["checks"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, criterion)| {
            json!({
                "criterionId": criterion["id"],
                "points": ([7, 3, 2][index]),
                "hintKey": "divider",
            })
        })
        .collect();
    file["schemaVersion"] = 2.into();
    file["grading"] =
        json!({ "criteria": rows, "hints": {"divider": "Check the resistor ratio."} });
    file
}

fn decode(file: &Value) -> Result<Assignment, Vec<open_circuit_engine::AssignmentError>> {
    decode_assignment_json(file.to_string().as_bytes())
}

#[test]
fn schema_two_round_trips_and_student_export_only_removes_reference() {
    let file = graded_document();
    let assignment = decode(&file).unwrap();
    let bytes = encode_assignment_json(&assignment).unwrap();
    let encoded: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(encoded["schemaVersion"], 2);
    assert_eq!(encoded["startingCircuit"]["schemaVersion"], 1);
    assert_eq!(encoded["grading"], file["grading"]);
    assert_eq!(decode_assignment_json(&bytes).unwrap(), assignment);
    let copy = assignment.student_copy();
    assert!(copy.reference.is_none());
    assert_eq!(copy.grading, assignment.grading);
    assert_eq!(
        decode_assignment_json(&encode_assignment_json(&copy).unwrap()).unwrap(),
        copy
    );

    let legacy = decode_assignment_json(LEGACY.as_bytes()).unwrap();
    assert_eq!(legacy.schema_version, 1);
    assert!(legacy.grading.is_none());
    let serialized: Value =
        serde_json::from_slice(&encode_assignment_json(&legacy).unwrap()).unwrap();
    assert!(serialized.get("grading").is_none());
}

#[test]
fn refuses_bad_versions_references_limits_and_extra_fields() {
    let file = graded_document();
    for (pointer, value) in [
        ("/schemaVersion", json!(1)),
        ("/schemaVersion", json!(3)),
        ("/grading", Value::Null),
        ("/grading/criteria/0/points", json!(1001)),
        ("/grading/criteria/0/points", json!(0.5)),
        ("/grading/criteria/0/points", json!(-1)),
        ("/grading/criteria/0/hintKey", json!(null)),
        ("/grading/criteria/0/hintKey", json!("undefined")),
        ("/grading/criteria/0/hintKey", json!("invalid/key")),
        (
            "/grading/criteria/0/criterionId",
            file["checks"][1]["id"].clone(),
        ),
        (
            "/grading/criteria/0/criterionId",
            json!("00000000-0000-4000-8000-0000000000ff"),
        ),
        ("/grading/hints/divider", json!(" ")),
        ("/grading/hints/divider", json!("€".repeat(501))),
        ("/task", json!({"kind":"findFault","reveal":"afterCheck"})),
    ] {
        let mut invalid = file.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(decode(&invalid).is_err(), "{pointer}");
    }
    let mut invalid = file.clone();
    invalid["grading"]["extra"] = true.into();
    assert!(decode(&invalid).is_err());
    let mut invalid = file.clone();
    invalid["grading"]["criteria"][0]["extra"] = true.into();
    assert!(decode(&invalid).is_err());
    let mut invalid = file.clone();
    invalid["grading"]["hints"] = Value::Object(
        (0..65)
            .map(|index| (format!("hint-{index}"), json!("x")))
            .collect(),
    );
    assert!(decode(&invalid).is_err());
    let mut invalid = file.clone();
    for row in invalid["grading"]["criteria"].as_array_mut().unwrap() {
        row["points"] = 0.into();
    }
    assert!(decode(&invalid).is_err());
    let mut invalid = file.clone();
    invalid["grading"]["criteria"].as_array_mut().unwrap().pop();
    assert!(decode(&invalid).is_err());
    let mut boundary = file;
    boundary["grading"]["hints"]["divider"] = json!("€".repeat(500));
    assert!(decode(&boundary).is_ok());
}

fn graded_request() -> GradedCheckRequest {
    let assignment = decode(&graded_document()).unwrap();
    let schematic = serde_json::from_str::<open_circuit_engine::Schematic>(DRAWING).unwrap();
    let mut grading = assignment.grading.unwrap();
    let mut checks = assignment.checks.unwrap();
    checks[0] = CheckCriterion::ComponentCount {
        id: grading.criteria[0].criterion_id.clone(),
        label: None,
        type_id: ComponentTypeId::Resistor,
        min: 2,
        max: Some(2),
    };
    checks[1] = CheckCriterion::ComponentCount {
        id: grading.criteria[1].criterion_id.clone(),
        label: None,
        type_id: ComponentTypeId::Resistor,
        min: 3,
        max: None,
    };
    grading.criteria.reverse();
    GradedCheckRequest {
        request: CheckRequest {
            checks,
            locked_components: vec![],
            starting_circuit: schematic.clone(),
            schematic,
            geometry: serde_json::from_str(GEOMETRY).unwrap(),
            document_revision: "test".into(),
        },
        grading,
    }
}

#[test]
fn engine_scores_in_authored_order_and_only_hints_failed_criteria() {
    let input = graded_request();
    let legacy = evaluate_checks(input.request.clone());
    let result = evaluate_graded_checks(input.clone()).unwrap();
    assert_eq!(result, evaluate_graded_checks(input).unwrap());
    assert_eq!(result.result, legacy);
    assert_eq!(result.schema_version, 2);
    assert_eq!((result.score.earned, result.score.possible), (9, 12));
    assert_eq!(
        result
            .score
            .criteria
            .iter()
            .map(|row| row.earned)
            .collect::<Vec<_>>(),
        [7, 0, 2]
    );
    assert!(result.score.criteria[0].hint.is_none());
    assert!(result.score.criteria[2].hint.is_none());
    assert_eq!(
        result.score.criteria[1].hint.as_ref().unwrap().text,
        "Check the resistor ratio."
    );
    let serialized = serde_json::to_value(result).unwrap();
    assert_eq!(serialized["score"]["criteria"][1]["hint"]["key"], "divider");
    assert!(serialized["score"]["criteria"][0].get("hint").is_none());
}

#[test]
fn compile_and_locked_failures_cannot_award_points_or_hints() {
    let mut input = graded_request();
    input
        .request
        .locked_components
        .push(StableId::parse("00000000-0000-4000-8000-0000000000ff").unwrap());
    let locked = evaluate_graded_checks(input).unwrap();
    assert_eq!(locked.result.locked.verdict, CheckVerdict::Fail);
    assert_eq!((locked.score.earned, locked.score.possible), (0, 12));
    assert!(locked.score.criteria.iter().all(|row| row.hint.is_none()));

    let mut input = graded_request();
    input.request.geometry.components.clear();
    let failed = evaluate_graded_checks(input).unwrap();
    assert!(!failed.result.diagnostics.is_empty());
    assert_eq!((failed.score.earned, failed.score.possible), (0, 12));
    assert!(failed.score.criteria.iter().all(|row| row.hint.is_none()));
}
