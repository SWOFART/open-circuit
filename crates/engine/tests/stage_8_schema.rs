//! Stage 8 schema/validation/migration boundary (#130), independent of rail net assembly.

use open_circuit_engine::{
    CheckCriterion, CheckOutcome, CheckRequest, CheckVerdict, CompileRequest, CompileResult,
    DiagnosticCode, DiagnosticReference, Engine, GeometryEnvelope, MAX_RAIL_COUNT, Schematic,
    decode_assignment_json, decode_project_json, encode_assignment_json, evaluate_checks,
    validate_assignment, validate_assignment_draft_value, validate_assignment_value,
};
use serde_json::{Value, json};

fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn rail(id: usize, name: &str) -> Value {
    json!({"id":format!("00000000-0000-4000-8000-{id:012x}"),"name":name,
        "start":{"x":0,"y":0},"end":{"x":0,"y":10}})
}

fn empty() -> Value {
    json!({"schemaVersion":2,"components":[],"wires":[],"junctions":[],"referenceMarkers":[],"rails":[]})
}

fn decode(v: &Value) -> Result<Schematic, open_circuit_engine::ProjectDocumentError> {
    decode_project_json(&serde_json::to_vec(v).unwrap())
}

fn with_rail() -> Value {
    let mut v = empty();
    v["rails"] = json!([rail(0x8000, "L+")]);
    v
}

// JSON integer literals and serialized f64 parameters can spell the same exact
// model input differently (5 versus 5.0). Compare every key/element and numeric
// value exactly, without a tolerance or any omitted fields.
fn same_json(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64()),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                same_json(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for (key, a) in a {
                same_json(a, &b[key]);
            }
        }
        _ => assert_eq!(a, b),
    }
}

#[test]
fn v1_migrates_losslessly_and_v2_roundtrips_only_drawn_inputs() {
    let legacy = include_bytes!("../../../tests/golden/stage-8-legacy-divider.ocircuit");
    let normalized = decode_project_json(legacy).unwrap();
    assert_eq!(normalized.schema_version, 2);
    assert!(normalized.rails.is_empty());
    same_json(
        &serde_json::to_value(&normalized).unwrap(),
        &fixture("stage-1-divider.schematic.json"),
    );
    for pair in fixture("stage-8-circuits.json")["circuits"]
        .as_array()
        .unwrap()
    {
        let schematic = decode(&pair["railSchematic"]).unwrap();
        let bytes = serde_json::to_vec(&schematic).unwrap();
        assert_eq!(decode_project_json(&bytes).unwrap(), schematic);
        same_json(
            &serde_json::to_value(schematic).unwrap(),
            &pair["railSchematic"],
        );
    }
}

#[test]
fn strict_version_shapes_reject_unknown_null_missing_and_duplicate_fields() {
    for key in ["schemaVersion", "rails"] {
        let mut v = with_rail();
        v.as_object_mut().unwrap().remove(key);
        assert!(decode(&v).is_err());
        let mut v = with_rail();
        v[key] = Value::Null;
        assert!(decode(&v).is_err());
    }
    for version in [0, 3, 999] {
        let mut v = with_rail();
        v["schemaVersion"] = version.into();
        assert!(decode(&v).is_err());
    }
    let mut v = with_rail();
    v["schemaVersion"] = 1.into();
    assert!(decode(&v).is_err(), "v1 must reject even empty rails");
    for key in ["voltageVolts", "runtimeState", "nodeId"] {
        let mut v = with_rail();
        v["rails"][0][key] = json!(1);
        assert!(decode(&v).is_err());
    }
    for key in ["name", "id", "start", "end"] {
        let mut v = with_rail();
        v["rails"][0].as_object_mut().unwrap().remove(key);
        assert!(decode(&v).is_err());
        let mut v = with_rail();
        v["rails"][0][key] = Value::Null;
        assert!(decode(&v).is_err());
    }
    for coordinate in [json!(0.5), json!(2147483648i64), json!(-2147483649i64)] {
        let mut v = with_rail();
        v["rails"][0]["start"]["x"] = coordinate;
        assert!(decode(&v).is_err());
    }
    let raw = serde_json::to_string(&with_rail()).unwrap();
    let duplicate = raw.replacen(
        "\"schemaVersion\":2",
        "\"schemaVersion\":2,\"schemaVersion\":2",
        1,
    );
    assert!(decode_project_json(duplicate.as_bytes()).is_err());
    let duplicate = raw.replacen("\"name\":\"L+\"", "\"name\":\"L+\",\"name\":\"R\"", 1);
    assert!(decode_project_json(duplicate.as_bytes()).is_err());
}

#[test]
fn legacy_dynamic_and_nonlinear_inputs_preserve_every_model_parameter_and_initial_condition() {
    for name in [
        "stage-3-rc-charge.schematic.json",
        "stage-3-rc-discharge.schematic.json",
        "stage-3-rl-rise.schematic.json",
        "stage-3-rl-decay.schematic.json",
        "stage-3-lossless-lc.schematic.json",
        "stage-6-drawn-latching.request.json",
        "stage-7-drawn-diode-forward.request.json",
        "stage-7-drawn-npn-active.request.json",
    ] {
        let document = fixture(name);
        let current = if document.get("schematic").is_some() {
            document["schematic"].clone()
        } else {
            document
        };
        let mut legacy = current.clone();
        legacy["schemaVersion"] = 1.into();
        legacy.as_object_mut().unwrap().remove("rails");
        assert_eq!(
            decode(&legacy).unwrap(),
            decode(&current).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn names_extents_and_cross_kind_ids_follow_contract_oracles() {
    for name in ["0V", "L+", "N", "PE", "L1", "l1", &"A".repeat(32)] {
        let mut v = with_rail();
        v["rails"][0]["name"] = name.into();
        assert!(decode(&v).is_ok());
    }
    for name in ["", " L+", "L+ ", "+L", "L\n", "Lé", &"A".repeat(33)] {
        let mut v = with_rail();
        v["rails"][0]["name"] = name.into();
        assert!(decode(&v).is_err());
    }
    for case in fixture("stage-8-connectivity.json")["cases"]
        .as_array()
        .unwrap()
    {
        let errors = case["errorCodes"].as_array().unwrap();
        let structural = errors.iter().any(|e| {
            ["InvalidGeometry", "InvalidRailName", "DuplicateRailName"]
                .contains(&e.as_str().unwrap())
        });
        let result = decode(&case["railSchematic"]);
        assert_eq!(result.is_err(), structural, "{}", case["id"]);
        if structural {
            let typed: Schematic = serde_json::from_value(case["railSchematic"].clone()).unwrap();
            let diagnostics = typed.validate();
            for code in errors {
                assert!(
                    diagnostics
                        .iter()
                        .any(|d| format!("{:?}", d.code) == code.as_str().unwrap())
                );
            }
            for reference in case["errorReferences"].as_array().unwrap() {
                assert!(diagnostics.iter().any(|d| {
                    d.references
                        .iter()
                        .any(|r| serde_json::to_value(r).unwrap() == *reference)
                }));
            }
        }
    }
    let mut v = fixture("stage-1-divider.schematic.json");
    v["rails"] = json!([rail(1, "L+")]);
    let typed: Schematic = serde_json::from_value(v).unwrap();
    let error = typed
        .validate()
        .into_iter()
        .find(|d| d.code == DiagnosticCode::DuplicateId)
        .unwrap();
    assert!(
        error
            .references
            .iter()
            .any(|r| matches!(r, DiagnosticReference::Component { .. }))
    );
    assert!(
        error
            .references
            .iter()
            .any(|r| matches!(r, DiagnosticReference::Rail { .. }))
    );
    let mut v = with_rail();
    v["rails"] = json!([
        rail(0x8002, "SAME"),
        rail(0x8000, "SAME"),
        rail(0x8001, "SAME")
    ]);
    let a: Schematic = serde_json::from_value(v.clone()).unwrap();
    v["rails"].as_array_mut().unwrap().reverse();
    let b: Schematic = serde_json::from_value(v).unwrap();
    assert_eq!(
        a.validate(),
        b.validate(),
        "three-way duplicate names are deterministic"
    );
}

#[test]
fn count_extent_and_full_pin_contact_budget_are_checked_without_grid_expansion() {
    let mut v = empty();
    v["rails"] = Value::Array(
        (0..MAX_RAIL_COUNT)
            .map(|n| rail(0x8000 + n, &format!("R{n}")))
            .collect(),
    );
    assert!(decode(&v).is_ok());
    v["rails"]
        .as_array_mut()
        .unwrap()
        .push(rail(0x9000, "EXTRA"));
    let typed: Schematic = serde_json::from_value(v).unwrap();
    let diagnostics = typed.validate();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, DiagnosticCode::RailLimitExceeded);
    assert!(diagnostics[0].references.is_empty());
    let mut v = with_rail();
    v["rails"][0]["start"] = json!({"x":2147483647,"y":-2147483648i64});
    v["rails"][0]["end"] = json!({"x":2147483647,"y":-2147483646i64});
    assert!(decode(&v).is_ok());
    v["rails"][0]["end"]["y"] = 2147483647.into();
    assert!(decode(&v).is_err());

    let fixture = fixture("stage-1-divider.schematic.json");
    let mut v = empty();
    v["components"] = json!([fixture["components"][1]]);
    v["rails"] = Value::Array(
        (0..MAX_RAIL_COUNT)
            .map(|n| rail(0x8000 + n, &format!("R{n}")))
            .collect(),
    );
    let allowed = (1_000_000 - MAX_RAIL_COUNT * (MAX_RAIL_COUNT - 1) / 2) / MAX_RAIL_COUNT;
    let points = (0..=allowed - 2)
        .map(|y| json!({"x":100,"y":y}))
        .collect::<Vec<_>>();
    v["wires"] = json!([{"id":"00000000-0000-4000-8000-00000000f000","points":points}]);
    let schematic = decode(&v).unwrap();
    let geometry: GeometryEnvelope = serde_json::from_value(
        json!({"components":[{"componentId":fixture["components"][1]["id"],"pins":[
        {"pinId":"pin1","position":{"x":3,"y":0}},{"pinId":"pin2","position":{"x":5,"y":0}}]}]}),
    )
    .unwrap();
    assert!(geometry.validate_for(&schematic).is_empty());
    v["wires"][0]["points"]
        .as_array_mut()
        .unwrap()
        .push(json!({"x":100,"y":allowed-1}));
    let schematic = decode(&v).unwrap(); // Structural budget excludes unresolved pins.
    assert_eq!(
        geometry.validate_for(&schematic)[0].code,
        DiagnosticCode::RailLimitExceeded
    );
}

#[test]
fn assignment_files_stay_v1_but_live_dtos_are_v2_and_rails_are_refused() {
    for name in ["divider-build", "fault-finding"] {
        let file = fixture(&format!("stage-5-drawn-{name}.ocassignment"));
        let assignment = decode_assignment_json(&serde_json::to_vec(&file).unwrap()).unwrap();
        let dto = serde_json::to_value(&assignment).unwrap();
        same_json(
            &dto,
            &fixture(&format!("stage-5-drawn-{name}.assignment.json")),
        );
        assert_eq!(dto["startingCircuit"]["schemaVersion"], 2);
        same_json(
            &serde_json::from_slice::<Value>(&encode_assignment_json(&assignment).unwrap())
                .unwrap(),
            &file,
        );
        assert!(validate_assignment(&assignment).is_ok());
        assert!(validate_assignment_draft_value(&dto).is_ok());
        let error = validate_assignment_value(&dto).unwrap_err();
        assert!(
            error
                .iter()
                .any(|e| e.path == "/startingCircuit/schemaVersion")
        );
        let mut bad = assignment.clone();
        bad.starting_circuit.as_mut().unwrap().rails = decode(&with_rail()).unwrap().rails;
        let error = validate_assignment(&bad).unwrap_err();
        assert!(error.iter().any(|e| e.path == "/startingCircuit/rails"));
        assert!(encode_assignment_json(&bad).is_err());
    }
}

#[test]
fn direct_checks_reject_rails_in_either_input_before_solving_and_bound_references() {
    let assignment = decode_assignment_json(
        &serde_json::to_vec(&fixture("stage-5-drawn-divider-build.ocassignment")).unwrap(),
    )
    .unwrap();
    let base = assignment.starting_circuit.unwrap();
    for start in [false, true] {
        let mut starting = base.clone();
        let mut student = base.clone();
        let target = if start { &mut starting } else { &mut student };
        target.rails = decode(&with_rail()).unwrap().rails;
        let mut request = CheckRequest {
            checks: vec![CheckCriterion::Value {
                id: open_circuit_engine::StableId::parse("00000000-0000-4000-8000-000000006000")
                    .unwrap(),
                label: None,
                signal: open_circuit_engine::CheckSignal {
                    key: format!("{}:voltage", base.components[0].id()),
                    unit: open_circuit_engine::Unit::Volt,
                },
                at_seconds: 0.0,
                expected: 5.0,
                tolerance_abs: 1e-6,
                tolerance_rel: 0.0,
            }],
            locked_components: vec![],
            starting_circuit: starting,
            schematic: student,
            geometry: GeometryEnvelope { components: vec![] },
            document_revision: "guard".into(),
        };
        request.checks.push(CheckCriterion::ComponentCount {
            id: open_circuit_engine::StableId::parse("00000000-0000-4000-8000-000000006001")
                .unwrap(),
            label: None,
            type_id: base.components[0].type_id(),
            min: 1,
            max: Some(1),
        });
        let result = evaluate_checks(request.clone());
        assert_eq!(result.outcome, CheckOutcome::Failed);
        assert!(matches!(
            result.criteria[0],
            open_circuit_engine::CriterionResult::Value {
                verdict: CheckVerdict::NotEvaluated,
                measured: None,
                ..
            }
        ));
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::UnsupportedAssignmentRails
        );
        assert_eq!(result.document_revision, "guard");
        assert!(matches!(
            result.criteria[1],
            open_circuit_engine::CriterionResult::ComponentCount {
                verdict: CheckVerdict::Pass,
                measured: Some(1),
                ..
            }
        ));
        let mut over = request;
        over.schematic.rails = (0..=MAX_RAIL_COUNT)
            .map(|_| decode(&with_rail()).unwrap().rails.remove(0))
            .collect();
        let result = evaluate_checks(over);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::RailLimitExceeded
        );
        assert!(result.diagnostics[0].references.is_empty());
    }
}

#[test]
fn pinless_rail_is_saveable_but_not_a_solvable_circuit() {
    let mut engine = Engine::default();
    let result = engine.compile(CompileRequest {
        schematic: decode(&with_rail()).unwrap(),
        geometry: GeometryEnvelope { components: vec![] },
        document_revision: "pinless-rail".into(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = result else {
        panic!("a pinless conductor must not produce a usable result");
    };
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::FloatingNode)
    );
}
