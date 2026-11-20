//! The Stage 12 bridge (issue #190): documentation is drawn input the Engine ignores. A circuit
//! with and without documentation compiles and runs to the same bytes, a live parameter update
//! keeps the run whatever the documentation says, and the project file round-trips it.

mod common;

use common::Run;
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_engine::{CompileRequest, ParameterId, SetParameterResult, decode_project_json};
use serde_json::{Value, json};

const CIRCUITS: &str = include_str!("../../../tests/golden/stage-8-circuits.json");

/// The golden divider drawn with explicit wires, read through the versioned decoder.
fn plain_request() -> CompileRequest {
    let all: Value = serde_json::from_str(CIRCUITS).unwrap();
    let pair = all["circuits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pair| pair["id"] == "divider")
        .expect("the divider circuit")
        .clone();
    CompileRequest {
        schematic: decode_project_json(&serde_json::to_vec(&pair["wireSchematic"]).unwrap())
            .expect("the divider opens"),
        geometry: serde_json::from_value(pair["geometry"].clone()).unwrap(),
        document_revision: "rev-1".to_owned(),
        faults: None,
    }
}

fn documented_request() -> CompileRequest {
    let mut request = plain_request();
    let first = request.schematic.components[0].id().to_string();
    let mut value = serde_json::to_value(&request.schematic).unwrap();
    value["documentation"] = json!({
        "designators": [{"componentId": first, "designator": "G1"}],
        "wireNumbers": [],
        "terminals": [{"id": "00000000-0000-4000-8000-000000001200", "strip": "X1",
            "label": "1", "position": {"x": 3, "y": 4}}],
        "notes": [{"id": "00000000-0000-4000-8000-000000001201",
            "position": {"x": 0, "y": 0}, "text": "Main supply"}],
        "titleBlock": {"title": "Divider", "drawingNumber": "D-1", "revision": "A",
            "author": "Student", "company": "", "date": ""},
        "sheets": [{"id": "00000000-0000-4000-8000-000000001202", "name": "Power",
            "size": "a4", "origin": {"x": 0, "y": 0}}],
    });
    request.schematic = serde_json::from_value(value).expect("documented schematic");
    request
}

/// What a run delivered, without the run ID: that is a fresh random token for every run.
fn delivered(run: &Run) -> Vec<(u64, u64, Vec<u64>, String)> {
    run.delivered
        .iter()
        .map(|(snapshot, _)| {
            (
                snapshot.metadata.simulation_step,
                snapshot.metadata.simulation_time_seconds.to_bits(),
                snapshot.values.iter().map(|v| v.to_bits()).collect(),
                format!("{:?}", snapshot.states),
            )
        })
        .collect()
}

#[test]
fn documentation_changes_neither_the_compile_nor_a_single_frame_byte() {
    let mut plain = Run::start_every_step(plain_request());
    let mut documented = Run::start_every_step(documented_request());
    assert_eq!(plain.layout, documented.layout);
    assert_eq!(
        plain.initial_snapshot.values,
        documented.initial_snapshot.values
    );

    plain.advance(20);
    documented.advance(20);
    assert!(!plain.delivered.is_empty());
    assert_eq!(delivered(&plain), delivered(&documented));
}

#[test]
fn a_live_parameter_update_works_on_a_documented_run() {
    let request = documented_request();
    let resistor = request.schematic.components[1].id().to_string();
    let mut run = Run::start_every_step(request);
    run.advance(3);
    let (_, result) = run.set_parameter(&resistor, ParameterId::ResistanceOhms, 2200.0);
    assert!(
        matches!(result, SetParameterResult::Success { .. }),
        "{result:?}"
    );
}

#[test]
fn the_documented_project_survives_save_and_open_and_runs_the_same() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("documented.ocircuit");
    let request = documented_request();

    write_project_file(&path, &request.schematic).expect("saves");
    let reopened = read_project_file(&path).expect("reopens");
    assert_eq!(reopened, request.schematic);

    let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["documentation"]["terminals"][0]["strip"], "X1");

    let mut first = Run::start_every_step(request.clone());
    let mut second = Run::start_every_step(CompileRequest {
        schematic: reopened,
        ..request
    });
    first.advance(10);
    second.advance(10);
    assert_eq!(delivered(&first), delivered(&second));
}
