//! Stage 12 project documentation format (#189): strict shapes, limits, the v1/v2 migration,
//! the v3 round trip, and the catalog's designator letters.

use open_circuit_engine::{
    DESIGNATOR_FIRST_LETTERS, DiagnosticCode, MAX_NOTES, MAX_SHEETS, ProjectDocumentError,
    Schematic, component_catalog, decode_assignment_json, decode_project_json, is_valid_designator,
};
use serde_json::{Value, json};

const R1: &str = "00000000-0000-4000-8000-000000000002";
const R2: &str = "00000000-0000-4000-8000-000000000003";
const WIRE: &str = "00000000-0000-4000-8000-000000000004";

fn golden(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(name);
    std::fs::read(path).unwrap()
}

fn divider() -> Value {
    serde_json::from_slice(&golden("stage-1-divider.schematic.json")).unwrap()
}

fn id(n: usize) -> String {
    format!("00000000-0000-4000-8000-{:012x}", 0x1200 + n)
}

fn with_documentation(documentation: Value) -> Value {
    let mut value = divider();
    let object = value["documentation"].as_object_mut().unwrap();
    for (key, entry) in documentation.as_object().unwrap() {
        object.insert(key.clone(), entry.clone());
    }
    value
}

fn decode(value: &Value) -> Result<Schematic, ProjectDocumentError> {
    decode_project_json(&serde_json::to_vec(value).unwrap())
}

fn codes(value: &Value) -> Vec<DiagnosticCode> {
    match decode(value) {
        Err(ProjectDocumentError::Invalid(codes)) => codes,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

fn refused(documentation: Value, code: DiagnosticCode) {
    let found = codes(&with_documentation(documentation.clone()));
    assert!(
        found.contains(&code),
        "{documentation} gave {found:?}, expected {code:?}"
    );
}

#[test]
fn a_full_documentation_round_trips_through_v3() {
    let value = with_documentation(json!({
        "designators": [{"componentId": R1, "designator": "R1"}, {"componentId": R2, "designator": "R2"}],
        "wireNumbers": [{"wireId": WIRE, "number": "12"}],
        "terminals": [{"id": id(1), "strip": "X1", "label": "3", "position": {"x": 4, "y": 2}}],
        "notes": [{"id": id(2), "position": {"x": 0, "y": 0}, "text": "Main supply"}],
        "titleBlock": {"title": "Divider", "drawingNumber": "D-1", "revision": "A",
            "author": "Student", "company": "School", "date": "2026-11-20"},
        "sheets": [{"id": id(3), "name": "Power", "size": "a4", "origin": {"x": 0, "y": 0}},
                   {"id": id(4), "name": "Control", "size": "a3", "origin": {"x": 100, "y": 0}}],
    }));
    let schematic = decode(&value).expect("a documented v3 project is valid");
    assert_eq!(schematic.documentation.designators.len(), 2);

    let saved = serde_json::to_vec(&schematic).unwrap();
    let again = decode_project_json(&saved).unwrap();
    assert_eq!(schematic, again);
    let saved_value: Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(saved_value["schemaVersion"], 3);
    assert_eq!(saved_value["documentation"], value["documentation"]);
}

#[test]
fn legacy_v1_and_v2_projects_open_with_empty_documentation() {
    for name in [
        "stage-12-legacy-v2-divider.ocircuit",
        "stage-12-legacy-v2-start-stop.ocircuit",
    ] {
        let bytes = golden(name);
        let legacy: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(legacy["schemaVersion"], 2, "{name} must stay a v2 sample");
        assert!(legacy.get("documentation").is_none());

        let schematic = decode_project_json(&bytes).expect(name);
        assert_eq!(schematic.schema_version, 3);
        assert!(schematic.documentation.is_empty());
        assert_eq!(
            schematic.components.len(),
            legacy["components"].as_array().unwrap().len()
        );
    }

    let v1 = json!({"schemaVersion": 1, "components": [], "wires": [], "junctions": [],
        "referenceMarkers": []});
    let schematic = decode(&v1).unwrap();
    assert_eq!(schematic.schema_version, 3);
    assert!(schematic.documentation.is_empty());
}

#[test]
fn a_v2_project_with_documentation_or_an_unknown_field_is_refused() {
    let mut v2 = json!({"schemaVersion": 2, "components": [], "wires": [], "junctions": [],
        "referenceMarkers": [], "rails": []});
    assert!(decode(&v2).is_ok());
    v2["documentation"] = divider()["documentation"].clone();
    assert!(matches!(
        decode(&v2),
        Err(ProjectDocumentError::Malformed(_))
    ));
}

#[test]
fn unknown_fields_and_wrong_shapes_are_malformed() {
    let mut value = divider();
    value["documentation"]["extra"] = json!(1);
    assert!(matches!(
        decode(&value),
        Err(ProjectDocumentError::Malformed(_))
    ));

    let mut value = divider();
    value["documentation"]["titleBlock"]["extra"] = json!("");
    assert!(matches!(
        decode(&value),
        Err(ProjectDocumentError::Malformed(_))
    ));

    let mut value = divider();
    value["documentation"] = json!({});
    assert!(matches!(
        decode(&value),
        Err(ProjectDocumentError::Malformed(_))
    ));

    let mut value = divider();
    value["documentation"]["designators"] = json!({R1: "R1"});
    assert!(matches!(
        decode(&value),
        Err(ProjectDocumentError::Malformed(_))
    ));

    let mut value = divider();
    value["documentation"]["sheets"] =
        json!([{"id": id(1), "name": "A", "size": "a5", "origin": {"x": 0, "y": 0}}]);
    assert!(matches!(
        decode(&value),
        Err(ProjectDocumentError::Malformed(_))
    ));

    let mut value = divider();
    value.as_object_mut().unwrap().remove("documentation");
    assert!(matches!(
        decode(&value),
        Err(ProjectDocumentError::Malformed(_))
    ));
}

#[test]
fn entries_for_unknown_objects_are_refused() {
    let unknown = id(99);
    refused(
        json!({"designators": [{"componentId": unknown, "designator": "R1"}]}),
        DiagnosticCode::UnknownDocumentationTarget,
    );
    refused(
        json!({"wireNumbers": [{"wireId": unknown, "number": "1"}]}),
        DiagnosticCode::UnknownDocumentationTarget,
    );
    // A wire ID is not a component ID.
    refused(
        json!({"designators": [{"componentId": WIRE, "designator": "R1"}]}),
        DiagnosticCode::UnknownDocumentationTarget,
    );
}

#[test]
fn designators_follow_the_grammar_and_stay_unique() {
    for bad in [
        "", "r1", "R", "R0", "R01", "R10000", "I1", "O2", "D1", "Z1", "RI1", "RRRR1", "R 1", "R1a",
    ] {
        refused(
            json!({"designators": [{"componentId": R1, "designator": bad}]}),
            DiagnosticCode::InvalidDesignator,
        );
    }
    for good in ["R1", "K12", "QF1", "FR9999", "XAB7"] {
        assert!(is_valid_designator(good), "{good}");
        decode(&with_documentation(
            json!({"designators": [{"componentId": R1, "designator": good}]}),
        ))
        .unwrap_or_else(|error| panic!("{good}: {error}"));
    }
    refused(
        json!({"designators": [{"componentId": R1, "designator": "R1"},
                               {"componentId": R2, "designator": "R1"}]}),
        DiagnosticCode::DuplicateDesignator,
    );
    refused(
        json!({"designators": [{"componentId": R1, "designator": "R1"},
                               {"componentId": R1, "designator": "R2"}]}),
        DiagnosticCode::DuplicateId,
    );
    assert!(
        DESIGNATOR_FIRST_LETTERS
            .chars()
            .all(|c| c != 'I' && c != 'O')
    );
}

#[test]
fn wire_numbers_terminals_notes_and_title_block_are_validated() {
    refused(
        json!({"wireNumbers": [{"wireId": WIRE, "number": ""}]}),
        DiagnosticCode::InvalidWireNumber,
    );
    refused(
        json!({"wireNumbers": [{"wireId": WIRE, "number": "a b"}]}),
        DiagnosticCode::InvalidWireNumber,
    );
    refused(
        json!({"wireNumbers": [{"wireId": WIRE, "number": "12345678901234567"}]}),
        DiagnosticCode::InvalidWireNumber,
    );

    let terminal = |n: usize, strip: &str, label: &str| json!({"id": id(n), "strip": strip, "label": label, "position": {"x": 0, "y": 0}});
    refused(
        json!({"terminals": [terminal(1, "x1", "1")]}),
        DiagnosticCode::InvalidTerminal,
    );
    refused(
        json!({"terminals": [terminal(1, "X1", "")]}),
        DiagnosticCode::InvalidTerminal,
    );
    refused(
        json!({"terminals": [terminal(1, "X1", "123456789")]}),
        DiagnosticCode::InvalidTerminal,
    );
    refused(
        json!({"terminals": [terminal(1, "X1", "1"), terminal(2, "X1", "1")]}),
        DiagnosticCode::DuplicateTerminal,
    );
    refused(
        json!({"terminals": [terminal(1, "X1", "1"), terminal(1, "X1", "2")]}),
        DiagnosticCode::DuplicateId,
    );

    let note = |text: &str| json!({"id": id(1), "position": {"x": 0, "y": 0}, "text": text});
    refused(json!({"notes": [note("")]}), DiagnosticCode::InvalidNote);
    refused(
        json!({"notes": [note(&"x".repeat(501))]}),
        DiagnosticCode::InvalidNote,
    );
    refused(
        json!({"notes": [note(&"x\n".repeat(11))]}),
        DiagnosticCode::InvalidNote,
    );
    refused(
        json!({"notes": [note("a\u{7}b")]}),
        DiagnosticCode::InvalidNote,
    );
    decode(&with_documentation(json!({"notes": [note("two\nlines")]}))).unwrap();

    let title = |field: &str, text: String| {
        let mut block = json!({"title": "", "drawingNumber": "", "revision": "", "author": "",
            "company": "", "date": ""});
        block[field] = Value::String(text);
        json!({ "titleBlock": block })
    };
    for (field, max) in [
        ("title", 100),
        ("drawingNumber", 32),
        ("revision", 16),
        ("author", 64),
        ("company", 64),
        ("date", 32),
    ] {
        decode(&with_documentation(title(field, "x".repeat(max)))).unwrap();
        refused(
            title(field, "x".repeat(max + 1)),
            DiagnosticCode::InvalidTitleBlock,
        );
    }
    refused(
        title("title", "a\nb".to_owned()),
        DiagnosticCode::InvalidTitleBlock,
    );
}

#[test]
fn sheets_are_named_sized_and_do_not_overlap() {
    let sheet = |n: usize, name: &str, size: &str, x: i64, y: i64| json!({"id": id(n), "name": name, "size": size, "origin": {"x": x, "y": y}});
    refused(
        json!({"sheets": [sheet(1, "", "a4", 0, 0)]}),
        DiagnosticCode::InvalidSheet,
    );
    refused(
        json!({"sheets": [sheet(1, &"n".repeat(33), "a4", 0, 0)]}),
        DiagnosticCode::InvalidSheet,
    );
    refused(
        json!({"sheets": [sheet(1, "A", "a4", 0, 0), sheet(2, "B", "a4", 58, 41)]}),
        DiagnosticCode::OverlappingSheets,
    );
    // A4 is 59 by 42 grid units, so touching edges do not overlap.
    decode(&with_documentation(
        json!({"sheets": [sheet(1, "A", "a4", 0, 0), sheet(2, "B", "a4", 59, 0),
                          sheet(3, "C", "a3", 0, 42)]}),
    ))
    .unwrap();
    // A3 is 84 by 59.
    refused(
        json!({"sheets": [sheet(1, "A", "a3", 0, 0), sheet(2, "B", "a4", 83, 0)]}),
        DiagnosticCode::OverlappingSheets,
    );
}

#[test]
fn documentation_ids_share_the_one_id_space() {
    refused(
        json!({"notes": [{"id": R1, "position": {"x": 0, "y": 0}, "text": "x"}]}),
        DiagnosticCode::DuplicateId,
    );
    refused(
        json!({"sheets": [{"id": WIRE, "name": "A", "size": "a4", "origin": {"x": 0, "y": 0}}]}),
        DiagnosticCode::DuplicateId,
    );
}

#[test]
fn limits_reject_the_whole_document() {
    let notes: Vec<Value> = (0..=MAX_NOTES)
        .map(|n| json!({"id": id(n + 100), "position": {"x": 0, "y": 0}, "text": "x"}))
        .collect();
    let found = codes(&with_documentation(json!({ "notes": notes })));
    assert_eq!(found, vec![DiagnosticCode::DocumentationLimitExceeded]);

    let sheets: Vec<Value> = (0..=MAX_SHEETS)
        .map(|n| {
            json!({"id": id(n + 100), "name": "S", "size": "a4",
                "origin": {"x": n as i64 * 59, "y": 0}})
        })
        .collect();
    let found = codes(&with_documentation(json!({ "sheets": sheets })));
    assert_eq!(found, vec![DiagnosticCode::DocumentationLimitExceeded]);

    let at_limit: Vec<Value> = (0..MAX_NOTES)
        .map(|n| json!({"id": id(n + 100), "position": {"x": 0, "y": 0}, "text": "x"}))
        .collect();
    decode(&with_documentation(json!({ "notes": at_limit }))).unwrap();
}

#[test]
fn assignment_files_refuse_documentation() {
    let bytes = golden("stage-5-drawn-divider-build.ocassignment");
    decode_assignment_json(&bytes).expect("the golden assignment is valid");

    let mut assignment: Value = serde_json::from_slice(&bytes).unwrap();
    assignment["startingCircuit"]["documentation"]["notes"] =
        json!([{"id": id(1), "position": {"x": 0, "y": 0}, "text": "x"}]);
    let result = decode_assignment_json(&serde_json::to_vec(&assignment).unwrap());
    assert!(
        result.is_err(),
        "an assignment must not carry documentation"
    );
}

#[test]
fn every_catalog_part_has_a_valid_default_designator_prefix() {
    let expected = [
        ("dc_voltage_source", "G"),
        ("ac_voltage_source", "G"),
        ("three_phase_source_star", "G"),
        ("three_phase_source_delta", "G"),
        ("resistor", "R"),
        ("inductor", "R"),
        ("diode", "R"),
        ("three_phase_load_star", "R"),
        ("three_phase_load_delta", "R"),
        ("capacitor", "C"),
        ("fuse", "F"),
        ("thermal_overload_three_pole", "F"),
        ("relay_spdt", "K"),
        ("contactor", "Q"),
        ("contactor_three_pole", "Q"),
        ("breaker_three_pole", "Q"),
        ("bjt_npn", "Q"),
        ("bjt_pnp", "Q"),
        ("switch_spst", "S"),
        ("push_button_no", "S"),
        ("push_button_nc", "S"),
        ("pmdc_motor", "M"),
        ("induction_motor_three_phase", "M"),
        ("transformer_three_phase", "T"),
        ("voltmeter", "P"),
        ("ammeter", "P"),
        ("ac_voltmeter", "P"),
        ("ac_ammeter", "P"),
        ("wattmeter", "P"),
    ];
    let catalog = component_catalog();
    let prefixes: Vec<(String, String)> = catalog
        .component_types
        .iter()
        .map(|entry| {
            let type_id = serde_json::to_value(entry.type_id).unwrap();
            (
                type_id.as_str().unwrap().to_owned(),
                entry.designator_prefix.clone(),
            )
        })
        .collect();

    for (type_id, prefix) in &prefixes {
        let first = prefix.chars().next().expect("a part needs a prefix");
        assert!(
            DESIGNATOR_FIRST_LETTERS.contains(first),
            "{type_id}: {prefix}"
        );
        assert!(
            is_valid_designator(&format!("{prefix}1")),
            "{type_id}: {prefix}1"
        );
    }
    // A new part must be added to the table above on purpose.
    assert_eq!(prefixes.len(), expected.len());
    for (type_id, prefix) in expected {
        let found = prefixes
            .iter()
            .find(|(id, _)| id == type_id)
            .unwrap_or_else(|| panic!("{type_id} is not in the catalog"));
        assert_eq!(found.1, prefix, "{type_id}");
    }
}
