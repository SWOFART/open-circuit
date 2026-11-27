//! Stage 16 mirror (#227): the optional `mirrored` flag of a component is drawn input only. The
//! saved project round-trips with and without it, an unmirrored file stays byte-identical, a
//! misspelled or mistyped field is still refused, and the Engine's result depends on the pin
//! geometry the editor sends, not on the flag (docs/contracts/stage-16-editor-improvements.md
//! section 5.2).

use open_circuit_engine::{CompileRequest, CompileResult, Engine, Schematic, decode_project_json};
use serde_json::{Value, from_str, json};

const SCHEMATIC_JSON: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY_JSON: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");

fn divider() -> Value {
    from_str(SCHEMATIC_JSON).unwrap()
}

fn with_mirrored(mut value: Value, mirrored: Value) -> Value {
    value["components"][1]["mirrored"] = mirrored;
    value
}

fn decode(value: &Value) -> Result<Schematic, open_circuit_engine::ProjectDocumentError> {
    decode_project_json(&serde_json::to_vec(value).unwrap())
}

#[test]
fn an_unmirrored_file_has_no_mirrored_key_and_round_trips_unchanged() {
    let value = divider();
    let schematic = decode(&value).unwrap();

    // The same document comes back: written out and read again it is equal, and its text has
    // no trace of the flag (numbers differ between `5` and `5.0` only as JSON values).
    let written = serde_json::to_vec(&schematic).unwrap();
    assert_eq!(decode_project_json(&written).unwrap(), schematic);
    assert!(
        !serde_json::to_string(&schematic)
            .unwrap()
            .contains("mirrored")
    );
    assert!(
        schematic
            .components
            .iter()
            .all(|component| !component.mirrored())
    );
}

#[test]
fn a_mirrored_part_round_trips_with_the_flag() {
    let value = with_mirrored(divider(), json!(true));
    let schematic = decode(&value).unwrap();

    assert!(schematic.components[1].mirrored());
    let written = serde_json::to_vec(&schematic).unwrap();
    assert_eq!(decode_project_json(&written).unwrap(), schematic);
    assert!(
        String::from_utf8(written)
            .unwrap()
            .contains("\"mirrored\":true")
    );
}

#[test]
fn an_unknown_or_mistyped_field_is_still_refused() {
    let mut flipped = divider();
    flipped["components"][1]["flipped"] = json!(true);
    assert!(decode(&flipped).is_err());

    assert!(decode(&with_mirrored(divider(), json!("yes"))).is_err());
    assert!(decode(&with_mirrored(divider(), json!(1))).is_err());
}

#[test]
fn the_flag_is_accepted_on_every_component_type() {
    // A part of each catalog type, with its default parameters, is read with the flag, writes it
    // back, and without it writes nothing: the flag is a field of every variant.
    for entry in open_circuit_engine::component_catalog().component_types {
        let parameters: serde_json::Map<String, Value> = entry
            .parameters
            .iter()
            .filter(|parameter| !parameter.optional)
            .map(|parameter| {
                (
                    serde_json::to_value(parameter.id)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                    json!(parameter.default_value),
                )
            })
            .collect();
        let part = json!({
            "typeId": entry.type_id,
            "id": "00000000-0000-4000-8000-000000000042",
            "position": {"x": 0, "y": 0},
            "rotationDeg": 0,
            "parameters": parameters,
        });
        let mut flagged = part.clone();
        flagged["mirrored"] = json!(true);

        let plain: open_circuit_engine::ComponentSpec = serde_json::from_value(part)
            .unwrap_or_else(|error| panic!("{:?} does not read: {error}", entry.type_id));
        let mirrored: open_circuit_engine::ComponentSpec = serde_json::from_value(flagged)
            .unwrap_or_else(|error| panic!("{:?} refuses the flag: {error}", entry.type_id));

        assert!(!plain.mirrored(), "{:?}", entry.type_id);
        assert!(mirrored.mirrored(), "{:?}", entry.type_id);
        assert!(!serde_json::to_string(&plain).unwrap().contains("mirrored"));
        assert!(
            serde_json::to_string(&mirrored)
                .unwrap()
                .contains("\"mirrored\":true")
        );
    }
}

#[test]
fn the_engine_result_depends_on_the_geometry_sent_not_on_the_flag() {
    let solve = |schematic: Value| {
        let request = CompileRequest {
            schematic: serde_json::from_value(schematic).unwrap(),
            geometry: from_str(GEOMETRY_JSON).unwrap(),
            document_revision: "divider-1".to_owned(),
            faults: None,
        };
        let CompileResult::Success {
            layout,
            initial_snapshot,
            diagnostics,
            ..
        } = Engine::default().compile(request)
        else {
            panic!("the divider must compile");
        };
        (layout, initial_snapshot.values, diagnostics)
    };

    assert_eq!(
        solve(divider()),
        solve(with_mirrored(divider(), json!(true)))
    );
}
