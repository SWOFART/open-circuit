//! The Stage 8 bridge (issue #132): rail-bearing documents run by the real Engine behind
//! `SimulationCore` and `SimulationService`, saved and opened by the application's project
//! persistence, and carried the way the application carries everything (JSON for commands,
//! binary OCF2 frames, OCT1 traces). The circuits and the answers are `tests/golden/stage-8-*`
//! (independent data, never Engine output). Nothing here adds a rail formula: the bridge carries
//! rails as drawn input and the Engine alone resolves them.

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{Run, assert_close};
use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_app_lib::simulation::{
    FrameSink, SimulationConfig, SimulationCore, SimulationService, SinkClosed,
};
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, OperatorPosition, ParameterId, ResultLayout,
    ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest,
    SetParameterResult, StableId, StreamNotice, TraceQuery, TraceRequest, decode_project_json,
};
use serde_json::{Value, json};

const CIRCUITS: &str = include_str!("../../../tests/golden/stage-8-circuits.json");
const MANIFEST: &str = include_str!("../../../tests/golden/stage-8-manifest.json");
const LEGACY: &str = include_str!("../../../tests/golden/stage-8-legacy-divider.ocircuit");

fn pairs() -> Vec<Value> {
    let all: Value = serde_json::from_str(CIRCUITS).unwrap();
    all["circuits"].as_array().unwrap().clone()
}

fn pair(id: &str) -> Value {
    pairs()
        .into_iter()
        .find(|pair| pair["id"] == id)
        .unwrap_or_else(|| panic!("no circuit {id}"))
}

fn text(value: &Value) -> &str {
    value.as_str().expect("a string")
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

fn rail_request(pair: &Value) -> CompileRequest {
    CompileRequest {
        schematic: serde_json::from_value(pair["railSchematic"].clone()).unwrap(),
        geometry: serde_json::from_value(pair["geometry"].clone()).unwrap(),
        document_revision: "rev-1".to_owned(),
        faults: None,
    }
}

/// The same circuit drawn with explicit wires, read through the versioned decoder.
fn wire_request(pair: &Value) -> CompileRequest {
    CompileRequest {
        schematic: decode_project_json(&serde_json::to_vec(&pair["wireSchematic"]).unwrap())
            .unwrap(),
        geometry: serde_json::from_value(pair["geometry"].clone()).unwrap(),
        document_revision: "rev-1".to_owned(),
        faults: None,
    }
}

fn with_rails(request: &CompileRequest, edit: impl Fn(&mut Vec<Value>)) -> CompileRequest {
    let mut json = serde_json::to_value(request).unwrap();
    let mut rails = json["schematic"]["rails"].as_array().unwrap().clone();
    edit(&mut rails);
    json["schematic"]["rails"] = Value::Array(rails);
    serde_json::from_value(json).unwrap()
}

/// Every value and state at each public step, after the events of that step, in step order.
struct Replay {
    layout: ResultLayout,
    snapshots: Vec<ResultSnapshot>,
}

fn observe(pair: &Value, row: &Value, layout: &ResultLayout, snapshot: &ResultSnapshot) {
    for value in row["values"].as_array().unwrap() {
        let slot = layout
            .slots
            .iter()
            .find(|slot| slot.key == text(&value["key"]))
            .unwrap_or_else(|| panic!("no slot {}", value["key"]));
        assert_eq!(serde_json::to_value(slot.unit).unwrap(), value["unit"]);
        let expected = number(&value["expected"]);
        let tolerance =
            number(&value["toleranceAbs"]) + number(&value["toleranceRel"]) * expected.abs();
        assert_close(
            snapshot.values[slot.offset as usize],
            expected,
            tolerance,
            &format!("{} step {} {}", pair["id"], row["step"], value["key"]),
        );
    }
    for expected in row["states"].as_array().unwrap() {
        let actual = snapshot
            .states
            .iter()
            .find(|state| state.component_id().as_str() == text(&expected["componentId"]))
            .unwrap();
        let actual = serde_json::to_value(actual).unwrap();
        for (key, value) in expected.as_object().unwrap() {
            assert_eq!(
                &actual[key], value,
                "{} step {}: {key}",
                pair["id"], row["step"]
            );
        }
    }
}

/// Runs a circuit through the application core, applies the fixture's events at their steps
/// as the application sends them, and checks every observation of the fixture on the frames
/// the bridge delivers.
fn replay(pair: &Value, request: CompileRequest) -> Replay {
    let mut run = Run::start_every_step(request);
    let layout = run.layout.clone();
    let observations = pair["observations"].as_array().unwrap();
    let last = observations
        .iter()
        .map(|row| row["step"].as_u64().unwrap())
        .max()
        .unwrap();
    let mut snapshot = run.initial_snapshot.clone();
    let mut snapshots = Vec::new();
    for step in 0..=last {
        if step > 0 {
            assert_eq!(run.advance(1), step);
            snapshot = run.delivered.last().unwrap().0.clone();
            assert_eq!(snapshot.metadata.simulation_step, step);
        }
        let rows = observations
            .iter()
            .filter(|row| row["step"] == step)
            .collect::<Vec<_>>();
        let after = |row: &&Value| !row["afterEvent"].is_null() || row["afterEventsAtStep"] == true;
        for row in rows.iter().filter(|row| !after(row)) {
            observe(pair, row, &layout, &snapshot);
        }
        for event in pair["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["newestStep"] == step)
        {
            let component = text(&event["componentId"]);
            snapshot = if event["kind"] == "parameter" {
                let parameter: ParameterId =
                    serde_json::from_value(event["parameterId"].clone()).unwrap();
                match run
                    .set_parameter(component, parameter, number(&event["value"]))
                    .1
                {
                    SetParameterResult::Success { snapshot, .. } => snapshot,
                    other => panic!("parameter event refused: {other:?}"),
                }
            } else {
                let position: OperatorPosition =
                    serde_json::from_value(event["position"].clone()).unwrap();
                match run.set_operator_state(component, position).1 {
                    SetOperatorStateResult::Success { snapshot, .. } => snapshot,
                    other => panic!("operator event refused: {other:?}"),
                }
            };
        }
        for row in rows.iter().filter(|row| after(row)) {
            observe(pair, row, &layout, &snapshot);
        }
        snapshots.push(snapshot.clone());
    }

    Replay { layout, snapshots }
}

#[test]
fn every_rail_drawing_runs_through_the_bridge_to_its_independent_answers() {
    let all = pairs();
    assert_eq!(all.len(), 6);
    for pair in &all {
        // Rail-drawn, then the same circuit drawn with explicit wires: both must satisfy the
        // fixture's observations through the same transport, and agree with each other.
        let rails = replay(pair, rail_request(pair));
        let wires = replay(pair, wire_request(pair));

        assert_eq!(rails.layout, wires.layout, "{}: one layout", pair["id"]);
        assert_eq!(rails.snapshots.len(), wires.snapshots.len());
        let equivalence = serde_json::from_str::<Value>(MANIFEST).unwrap()["equivalence"].clone();
        let (abs, rel) = (
            number(&equivalence["absolute"]),
            number(&equivalence["relative"]),
        );
        for (rail, wire) in rails.snapshots.iter().zip(&wires.snapshots) {
            assert_eq!(rail.metadata.simulation_step, wire.metadata.simulation_step);
            assert_eq!(rail.states, wire.states, "{}: discrete states", pair["id"]);
            for (a, b) in rail.values.iter().zip(&wire.values) {
                assert_close(
                    *a,
                    *b,
                    abs + rel * b.abs(),
                    &format!("{}: rail vs wire", pair["id"]),
                );
            }
        }
    }
}

#[test]
fn rails_add_no_slot_state_trace_signal_or_frame_field() {
    for pair in pairs() {
        let rails = Run::start_every_step(rail_request(&pair));
        let wires = Run::start_every_step(wire_request(&pair));

        // The layout, and so the OCF2 frame and the OCT1 envelope that follow it, are those of
        // the wire drawing: a rail is not a component, a slot, a state, or a signal.
        assert_eq!(rails.layout, wires.layout);
        let ids = pair["railSchematic"]["rails"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rail| text(&rail["id"]).to_owned())
            .collect::<Vec<_>>();
        for slot in &rails.layout.slots {
            for id in &ids {
                assert!(!slot.key.contains(id.as_str()), "a slot key names a rail");
                assert_ne!(slot.component_id.as_str(), id);
            }
        }
        assert_eq!(rails.layout.layout_version, 1);
        assert_eq!(
            rails.initial_snapshot.states.len(),
            wires.initial_snapshot.states.len()
        );
    }
}

#[test]
fn a_rail_document_reads_traces_and_replaces_the_current_sample_on_a_live_edit() {
    let pair = pair("divider");
    let source = text(&pair["railSchematic"]["components"][0]["id"]).to_owned();
    let resistor = text(&pair["railSchematic"]["components"][2]["id"]).to_owned();
    let mut run = Run::start_every_step(rail_request(&pair));
    run.advance(5);
    let keys = vec![format!("{source}:current"), format!("{resistor}:voltage")];
    let before = run.samples(&keys, 0, 5);

    let (_, result) = run.set_parameter(&resistor, ParameterId::ResistanceOhms, 3000.0);
    assert!(matches!(result, SetParameterResult::Success { .. }));
    let after = run.samples(&keys, 0, 5);

    // History keeps the old circuit; the newest sample is the edited one.
    assert_eq!(before[..5], after[..5]);
    assert_ne!(before[5], after[5]);
}

#[test]
fn a_saved_and_opened_rail_project_keeps_inputs_and_geometry_and_stores_nothing_derived() {
    let directory = tempfile::tempdir().unwrap();
    for pair in pairs() {
        let request = rail_request(&pair);
        let path = directory
            .path()
            .join(format!("{}.ocircuit", text(&pair["id"])));
        let mut run = Run::start_every_step(request.clone());
        run.advance(5);

        write_project_file(&path, &request.schematic).expect("saved after the run");
        let file = std::fs::read_to_string(&path).unwrap();
        for derived in [
            "nodeId",
            "netId",
            "island",
            "snapshot",
            "trace",
            "runId",
            "layout",
            "operating",
            "nodes",
            "conducting",
        ] {
            assert!(
                !file.contains(derived),
                "{}: the file holds `{derived}`",
                pair["id"]
            );
        }
        let saved: Value = serde_json::from_str(&file).unwrap();
        assert_eq!(saved["schemaVersion"], 2);
        assert_eq!(
            saved["rails"], pair["railSchematic"]["rails"],
            "IDs, names, geometry"
        );

        let reopened = read_project_file(&path).expect("reopens");
        assert_eq!(reopened, request.schematic, "{}", pair["id"]);

        // A run of the reopened document starts at step 0 and recomputes the values.
        let mut again = CompileRequest {
            schematic: reopened,
            ..request.clone()
        };
        again.document_revision = "rev-1".to_owned();
        let mut second = Run::start_every_step(again);
        assert_eq!(second.initial_snapshot.metadata.simulation_step, 0);
        second.advance(5);
        let bits = |run: &Run| {
            run.delivered
                .iter()
                .map(|(frame, _)| frame.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&run), bits(&second), "{}: recalculated", pair["id"]);
    }
}

#[test]
fn a_legacy_project_opens_as_version_two_and_the_file_is_not_rewritten() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.ocircuit");
    std::fs::write(&path, LEGACY).unwrap();

    let schematic = read_project_file(&path).expect("a version 1 file opens");
    assert!(schematic.rails.is_empty());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        LEGACY,
        "opening does not rewrite"
    );

    // An explicit save writes version 2 and keeps every ID, position and parameter.
    let saved = directory.path().join("saved.ocircuit");
    write_project_file(&saved, &schematic).unwrap();
    let before: Value = serde_json::from_str(LEGACY).unwrap();
    let after: Value = serde_json::from_slice(&std::fs::read(&saved).unwrap()).unwrap();
    assert_eq!(after["schemaVersion"], 2);
    assert_eq!(after["rails"], json!([]));
    for key in ["components", "wires", "junctions", "referenceMarkers"] {
        assert!(same_json(&after[key], &before[key]), "{key}");
    }
}

#[test]
fn a_refused_open_or_save_never_touches_the_current_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("current.ocircuit");
    let request = rail_request(&pair("divider"));
    write_project_file(&path, &request.schematic).unwrap();
    let good = std::fs::read(&path).unwrap();

    // A document the writer refuses (two rails with one name) leaves the file as it was.
    let duplicate = with_rails(&request, |rails| {
        rails[1]["name"] = rails[0]["name"].clone()
    });
    assert!(write_project_file(&path, &duplicate.schematic).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), good);

    // Files the reader refuses: a rail that is not vertical, a bad name, a null list, an extra
    // field, a missing list in version 2, and a future version.
    let valid: Value = serde_json::from_slice(&good).unwrap();
    for (what, edit) in [
        (
            "slanted",
            Box::new(|d: &mut Value| d["rails"][0]["end"]["x"] = json!(99))
                as Box<dyn Fn(&mut Value)>,
        ),
        (
            "bad name",
            Box::new(|d: &mut Value| d["rails"][0]["name"] = json!(" L+")),
        ),
        (
            "null rails",
            Box::new(|d: &mut Value| d["rails"] = Value::Null),
        ),
        (
            "extra field",
            Box::new(|d: &mut Value| d["rails"][0]["voltageVolts"] = json!(24)),
        ),
        (
            "missing rails",
            Box::new(|d: &mut Value| {
                d.as_object_mut().unwrap().remove("rails");
            }),
        ),
        (
            "future version",
            Box::new(|d: &mut Value| d["schemaVersion"] = json!(3)),
        ),
    ] {
        let mut document = valid.clone();
        edit(&mut document);
        let bad = directory.path().join("bad.ocircuit");
        std::fs::write(&bad, serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(read_project_file(&bad).is_err(), "{what} must be refused");
    }
    assert_eq!(std::fs::read(&path).unwrap(), good);
}

#[test]
fn rail_diagnostics_carry_typed_references_and_leave_the_previous_run_alone() {
    let pair = pair("divider");
    let request = rail_request(&pair);
    let first = text(&pair["railSchematic"]["rails"][0]["id"]).to_owned();
    let second = text(&pair["railSchematic"]["rails"][1]["id"]).to_owned();
    let mut run = Run::start_every_step(request.clone());
    run.advance(3);
    let revision = run
        .delivered
        .last()
        .unwrap()
        .0
        .metadata
        .document_revision
        .clone();

    // A bad name references its rail; two equal names reference both.
    let result = run.core.compile(with_rails(&request, |rails| {
        rails[0]["name"] = json!("bad name")
    }));
    let CompileResult::Failure { diagnostics } = &result else {
        panic!("a bad rail name does not compile");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidRailName);
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["status"], "failure");
    assert_eq!(
        json["diagnostics"][0]["localizationKey"],
        "diagnostic.invalidRailName"
    );
    assert_eq!(
        json["diagnostics"][0]["references"],
        json!([{ "kind": "rail", "railId": first }])
    );

    let result = run.core.compile(with_rails(&request, |rails| {
        rails[1]["name"] = rails[0]["name"].clone()
    }));
    let CompileResult::Failure { diagnostics } = &result else {
        panic!("equal names do not compile");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::DuplicateRailName);
    let references = serde_json::to_value(&diagnostics[0].references).unwrap();
    for id in [&first, &second] {
        assert!(
            references
                .as_array()
                .unwrap()
                .contains(&json!({ "kind": "rail", "railId": id })),
            "both conflicting rails are named"
        );
    }

    // Too many rails is a document-wide error without individual references.
    let many = with_rails(&request, |rails| {
        for index in 0..257 {
            rails.push(json!({
                "id": format!("00000000-0000-4000-8000-{:012x}", 0x9000 + index),
                "name": format!("X{index}"),
                "start": { "x": 100 + index, "y": 0 },
                "end": { "x": 100 + index, "y": 4 },
            }));
        }
    });
    let result = run.core.compile(many);
    let CompileResult::Failure { diagnostics } = &result else {
        panic!("too many rails do not compile");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::RailLimitExceeded);
    assert!(diagnostics[0].references.is_empty());

    // None of the failed compiles touched the running circuit.
    let length = run.delivered.len();
    run.advance(3);
    assert!(run.delivered.len() > length);
    assert_eq!(
        run.delivered.last().unwrap().0.metadata.document_revision,
        revision
    );
    assert_eq!(run.delivered.last().unwrap().0.metadata.simulation_step, 6);
}

#[test]
fn an_isolated_rail_is_a_floating_node_that_names_the_rail() {
    let request = rail_request(&pair("divider"));
    let isolated = with_rails(&request, |rails| {
        rails.push(json!({
            "id": "00000000-0000-4000-8000-0000000090aa",
            "name": "LOOSE",
            "start": { "x": 200, "y": 0 },
            "end": { "x": 200, "y": 10 },
        }));
    });
    let mut core = SimulationCore::new(SimulationConfig::default());
    let result = core.compile(isolated);
    let CompileResult::Failure { diagnostics } = &result else {
        panic!("a pinless rail floats");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::FloatingNode);
    let references = serde_json::to_value(&diagnostics[0].references).unwrap();
    assert!(
        references
            .as_array()
            .unwrap()
            .contains(&json!({ "kind": "rail", "railId": "00000000-0000-4000-8000-0000000090aa" }))
    );
}

#[test]
fn creating_renaming_moving_resizing_and_deleting_a_rail_each_start_a_fresh_run() {
    let pair = pair("start-stop");
    let request = rail_request(&pair);
    let start = text(&pair["railSchematic"]["components"][2]["id"]).to_owned();
    let contactor = text(&pair["railSchematic"]["components"][3]["id"]).to_owned();
    let mut run = Run::start_every_step(request.clone());
    let old_run = run.run_id.clone();
    run.advance(3);
    run.set_operator_state(&start, OperatorPosition::Pressed);
    run.advance(3);
    let picked = |snapshot: &ResultSnapshot| {
        snapshot.states.iter().any(|state| {
            serde_json::to_value(state).unwrap()["state"] == "picked"
                && state.component_id().as_str() == contactor
        })
    };
    assert!(
        picked(&run.delivered.last().unwrap().0),
        "the contactor is picked"
    );

    // A rename changes no connection and is still a document edit: a complete compile, a new
    // run at step 0 with the operator and the discrete state reset.
    let renamed = with_rails(&request, |rails| rails[0]["name"] = json!("RAIL_A"));
    let mut revision = 2;
    let mut check = |run: &mut Run, edited: CompileRequest, expect_success: bool, what: &str| {
        let mut edited = edited;
        edited.document_revision = format!("rev-{revision}");
        revision += 1;
        match run.core.compile(edited) {
            CompileResult::Success {
                run_id,
                initial_snapshot,
                layout,
                ..
            } => {
                assert!(expect_success, "{what}: compiled but should not");
                assert_ne!(run_id, old_run, "{what}: a new run");
                assert_eq!(initial_snapshot.metadata.simulation_step, 0, "{what}");
                assert!(!picked(&initial_snapshot), "{what}: states reset");
                assert_eq!(layout.layout_version, 1);
            }
            CompileResult::Failure { diagnostics } => {
                assert!(!expect_success, "{what}: failed: {diagnostics:?}");
            }
        }
    };
    check(&mut run, renamed.clone(), true, "rename");
    // Move the whole 0V rail one unit and extend the supply rail: both still touch their pins
    // in the fixtures only where the drawing says so, so the results are the Engine's call.
    let moved = with_rails(&request, |rails| {
        let x = rails[1]["start"]["x"].as_i64().unwrap();
        rails[1]["start"]["x"] = json!(x + 1);
        rails[1]["end"]["x"] = json!(x + 1);
    });
    check(&mut run, moved, false, "move away from its pins");
    let resized = with_rails(&request, |rails| {
        let start = rails[0]["start"]["y"].as_i64().unwrap();
        rails[0]["start"]["y"] = json!(start - 2);
    });
    check(&mut run, resized, true, "extend the supply rail upward");
    let deleted = with_rails(&request, |rails| {
        rails.pop();
    });
    check(&mut run, deleted, true, "delete the last rail");
    // Creating a rail that joins nothing new is a document edit as well.
    let created = with_rails(&request, |rails| {
        rails.push(json!({
            "id": "00000000-0000-4000-8000-0000000090bb",
            "name": "SPARE",
            "start": { "x": rails[0]["start"]["x"], "y": json!(rails[0]["start"]["y"].as_i64().unwrap() - 4) },
            "end": { "x": rails[0]["start"]["x"], "y": rails[0]["start"]["y"] },
        }));
    });
    check(
        &mut run,
        created,
        true,
        "create a rail on the top end of the supply rail",
    );
}

#[test]
fn a_replaced_rail_document_leaves_no_old_run_value_frame_or_trace_to_read() {
    let pair = pair("divider");
    let request = rail_request(&pair);
    let resistor = text(&pair["railSchematic"]["components"][1]["id"]).to_owned();
    let mut run = Run::start_every_step(request.clone());
    run.advance(3);
    let old_run = run.run_id.clone();
    let keys = vec![format!("{resistor}:voltage")];

    // The rename compiles a new run; the old stream ends and no later frame carries the old run.
    let renamed = with_rails(&request, |rails| rails[0]["name"] = json!("RENAMED"));
    let CompileResult::Success { run_id, .. } = run.core.compile(renamed) else {
        panic!("a rename compiles");
    };
    assert_ne!(run_id, old_run);
    assert!(!run.core.is_streaming());

    // A late parameter update and a late trace read for the old run are stale.
    let late = SetParameterRequest {
        run_id: old_run.clone(),
        expected_document_revision: "rev-1".to_owned(),
        document_revision: "rev-9".to_owned(),
        component_id: StableId::parse(&resistor).unwrap(),
        parameter_id: ParameterId::ResistanceOhms,
        value: 2000.0,
    };
    let SetParameterResult::Failure { diagnostics } = run.core.set_parameter(late) else {
        panic!("the replaced run is stale");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);
    let read = TraceRequest {
        run_id: old_run,
        layout_version: run.layout.layout_version,
        signal_keys: keys,
        query: TraceQuery::History {
            start_step: 0,
            end_step: 3,
        },
        max_points: 16,
    };
    let refused = run
        .core
        .read_traces(read, Box::new(Capture::default()), Instant::now());
    assert_eq!(refused.unwrap_err()[0].code, DiagnosticCode::StaleRun);
}

#[derive(Clone, Default)]
struct Capture {
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    notices: Arc<Mutex<Vec<StreamNotice>>>,
}

impl FrameSink for Capture {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.frames.lock().unwrap().push(frame);
        Ok(())
    }

    fn notice(&mut self, notice: &StreamNotice) -> Result<(), SinkClosed> {
        self.notices.lock().unwrap().push(notice.clone());
        Ok(())
    }
}

impl open_circuit_app_lib::simulation::TraceSink for Capture {
    fn send(&mut self, _response: Vec<u8>) -> Result<(), SinkClosed> {
        Ok(())
    }
}

#[test]
fn the_service_runs_a_rail_circuit_with_live_operator_and_parameter_updates() {
    let pair = pair("start-stop");
    let start = text(&pair["railSchematic"]["components"][2]["id"]).to_owned();
    let source = text(&pair["railSchematic"]["components"][0]["id"]).to_owned();
    let service = SimulationService::spawn(SimulationConfig {
        max_in_flight: 10_000,
        ..SimulationConfig::default()
    });
    let CompileResult::Success { run_id, layout, .. } =
        service.compile(rail_request(&pair)).unwrap()
    else {
        panic!("the rail circuit compiles");
    };
    let capture = Capture::default();
    service
        .start(run_id.clone(), Box::new(capture.clone()))
        .unwrap()
        .unwrap();
    std::thread::sleep(Duration::from_millis(60));

    let operator = |expected: &str, next: &str, position| SetOperatorStateRequest {
        run_id: run_id.clone(),
        expected_document_revision: expected.to_owned(),
        document_revision: next.to_owned(),
        component_id: StableId::parse(&start).unwrap(),
        position,
    };
    let started = Instant::now();
    assert!(matches!(
        service
            .set_operator_state(operator("rev-1", "rev-2", OperatorPosition::Pressed))
            .unwrap(),
        SetOperatorStateResult::Success { .. }
    ));
    // A refused update (a negative internal resistance) changes neither revision nor stream.
    let refused = service
        .set_parameter(SetParameterRequest {
            run_id: run_id.clone(),
            expected_document_revision: "rev-2".to_owned(),
            document_revision: "rev-3".to_owned(),
            component_id: StableId::parse(&source).unwrap(),
            parameter_id: ParameterId::InternalResistanceOhms,
            value: -1.0,
        })
        .unwrap();
    assert!(matches!(refused, SetParameterResult::Failure { .. }));
    assert!(matches!(
        service
            .set_parameter(SetParameterRequest {
                run_id: run_id.clone(),
                expected_document_revision: "rev-2".to_owned(),
                document_revision: "rev-3".to_owned(),
                component_id: StableId::parse(&source).unwrap(),
                parameter_id: ParameterId::VoltageVolts,
                value: 6.0,
            })
            .unwrap(),
        SetParameterResult::Success { .. }
    ));
    assert!(matches!(
        service
            .set_operator_state(operator("rev-3", "rev-4", OperatorPosition::Released))
            .unwrap(),
        SetOperatorStateResult::Success { .. }
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(60));
    service.stop(Some(run_id)).unwrap();

    let frames = capture
        .frames
        .lock()
        .unwrap()
        .iter()
        .map(|bytes| decode_frame(bytes).unwrap())
        .collect::<Vec<_>>();
    assert!(frames.len() > 3, "{} frames", frames.len());
    let mut last_step = 0;
    let mut last_sequence: Option<u64> = None;
    for frame in &frames {
        assert!(frame.metadata.simulation_step >= last_step);
        assert!(last_sequence.is_none_or(|last| frame.metadata.frame_sequence > last));
        assert_eq!(frame.values.len(), layout.value_count as usize);
        assert!(frame.values.iter().all(|value| value.is_finite()));
        last_step = frame.metadata.simulation_step;
        last_sequence = Some(frame.metadata.frame_sequence);
    }
    // The seal-in circuit keeps the contactor picked after START is released.
    let last = frames.last().unwrap();
    assert_eq!(last.metadata.document_revision, "rev-4");
    assert!(
        last.states
            .iter()
            .any(|state| serde_json::to_value(state).unwrap()["state"] == "picked"),
        "the contactor stays picked through the seal-in contact"
    );
    service.shutdown();
}

#[test]
fn a_rail_circuit_that_fails_while_running_stops_with_a_notice_and_keeps_the_document() {
    // An ideal 1e154 V source through a fuse overflows in the first step; the same notice as
    // without rails reaches the sink, with the last committed revision.
    let pair = pair("divider");
    let mut json = serde_json::to_value(rail_request(&pair)).unwrap();
    json["schematic"]["components"][0]["parameters"] =
        json!({ "voltageVolts": 1e154, "internalResistanceOhms": 0.0 });
    json["schematic"]["components"][2]["parameters"] = json!({ "resistanceOhms": 1.0 });
    json["schematic"]["components"][1]["typeId"] = json!("fuse");
    json["schematic"]["components"][1]["parameters"] =
        json!({ "intactResistanceOhms": 0.01, "tripI2tAmpereSquaredSeconds": 1e300 });
    let request: CompileRequest = serde_json::from_value(json).unwrap();

    let mut core = SimulationCore::new(SimulationConfig {
        frame_interval: Duration::from_millis(1),
        ..SimulationConfig::default()
    });
    let CompileResult::Success { run_id, .. } = core.compile(request) else {
        panic!("the circuit compiles");
    };
    let capture = Capture::default();
    let now = Instant::now();
    core.start(run_id.clone(), Box::new(capture.clone()), now)
        .unwrap();
    core.tick(now + Duration::from_millis(5));

    assert!(!core.is_streaming());
    assert!(capture.frames.lock().unwrap().is_empty());
    let notices = capture.notices.lock().unwrap().clone();
    assert_eq!(notices.len(), 1);
    let StreamNotice::RunStopped {
        run_id: noticed,
        document_revision,
        simulation_step,
        diagnostics,
    } = &notices[0];
    assert_eq!(noticed, &run_id);
    assert_eq!(document_revision, "rev-1");
    assert_eq!(*simulation_step, 0);
    assert_eq!(diagnostics[0].code, DiagnosticCode::NonFiniteState);
}

/// JSON equality that reads 5 and 5.0 as the same number, as a reader of the file does.
fn same_json(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_json(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, x)| b.get(key).is_some_and(|y| same_json(x, y)))
        }
        _ => left == right,
    }
}

const DRAWN_START_STOP: &str =
    include_str!("../../../tests/golden/stage-8-drawn-start-stop.request.json");
const DRAWN_NO_SEAL_IN: &str =
    include_str!("../../../tests/golden/stage-8-drawn-start-stop-no-seal-in.request.json");

fn drawn(text: &str) -> CompileRequest {
    serde_json::from_str(text).expect("a drawn compile request")
}

#[test]
fn the_start_stop_circuit_drawn_between_rails_runs_to_the_independent_answers() {
    // Drawn from a blank document by editor commands (`src/store/stage8Drawing.test.ts`), with
    // the same component IDs as the fixture, so its observations and events apply as they are.
    for (text, id) in [
        (DRAWN_START_STOP, "start-stop"),
        (DRAWN_NO_SEAL_IN, "start-stop-no-seal-in"),
    ] {
        replay(&pair(id), drawn(text));
    }
}

#[test]
fn the_wrong_wiring_is_visible_and_gives_the_distinct_result() {
    // Without the seal-in contact the contactor drops out when START is released; with it, it
    // stays picked. Both drawings are accepted as drawn: nothing repairs the missing wires.
    let correct = replay(&pair("start-stop"), drawn(DRAWN_START_STOP));
    let wrong = replay(&pair("start-stop-no-seal-in"), drawn(DRAWN_NO_SEAL_IN));
    let contactor = text(&pair("start-stop")["railSchematic"]["components"][3]["id"]).to_owned();
    let picked = |snapshot: &ResultSnapshot| {
        snapshot.states.iter().any(|state| {
            state.component_id().as_str() == contactor
                && serde_json::to_value(state).unwrap()["state"] == "picked"
        })
    };

    // Step 12 is after START was released (step 10) and before STOP was pressed (step 15).
    assert!(
        picked(&correct.snapshots[12]),
        "the seal-in circuit stays picked"
    );
    assert!(
        !picked(&wrong.snapshots[12]),
        "without the contact it drops out"
    );
    let coil = format!("{contactor}/coil/current");
    let at = |replay: &Replay| {
        let slot = replay
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == coil)
            .unwrap();
        replay.snapshots[12].values[slot.offset as usize]
    };
    assert!(at(&correct) > 0.049);
    assert!(at(&wrong).abs() < 1e-4);
}

#[test]
fn a_saved_and_reopened_rail_drawing_recomputes_a_fresh_run_without_the_operator_positions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("start-stop.ocircuit");
    let request = drawn(DRAWN_START_STOP);
    let switch = pair("start-stop")["railSchematic"]["components"][2]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Operate the circuit, then save: no position, state or result reaches the file.
    let mut run = Run::start_every_step(request.clone());
    run.advance(3);
    run.set_operator_state(&switch, OperatorPosition::Pressed);
    run.advance(3);
    write_project_file(&path, &request.schematic).expect("saved after operating");
    let file = std::fs::read_to_string(&path).unwrap();
    for operated in [
        "pressed",
        "picked",
        "released",
        "operator",
        "conducting",
        "coil/current",
    ] {
        assert!(!file.contains(operated), "the file holds `{operated}`");
    }
    let saved: Value = serde_json::from_str(&file).unwrap();
    assert_eq!(saved["rails"][0]["name"], "L+");
    assert_eq!(saved["rails"][1]["name"], "0V");

    // The reopened drawing is the drawn one, and a fresh run of it passes the independent
    // observations again from step 0.
    let reopened = read_project_file(&path).expect("reopens");
    assert_eq!(reopened, request.schematic);
    replay(
        &pair("start-stop"),
        CompileRequest {
            schematic: reopened,
            ..request
        },
    );
}

const DRAWN_DIVIDER: &str =
    include_str!("../../../tests/golden/stage-8-drawn-divider.request.json");
const DRAWN_RC_HOLD: &str =
    include_str!("../../../tests/golden/stage-8-drawn-rc-hold.request.json");
const DRAWN_TWO_ISLANDS: &str =
    include_str!("../../../tests/golden/stage-8-drawn-two-islands.request.json");
const DRAWN_ISLANDS_N_PE: &str =
    include_str!("../../../tests/golden/stage-8-drawn-two-islands-N-PE.request.json");

fn bits(replay: &Replay) -> Vec<Vec<u64>> {
    replay
        .snapshots
        .iter()
        .map(|snapshot| {
            snapshot
                .values
                .iter()
                .map(|value| value.to_bits())
                .collect()
        })
        .collect()
}

#[test]
fn every_circuit_drawn_between_rails_matches_its_answers_its_wired_twin_and_itself() {
    let equivalence = serde_json::from_str::<Value>(MANIFEST).unwrap()["equivalence"].clone();
    let (abs, rel) = (
        number(&equivalence["absolute"]),
        number(&equivalence["relative"]),
    );
    let all = [
        ("divider", DRAWN_DIVIDER),
        ("rc-hold", DRAWN_RC_HOLD),
        ("start-stop", DRAWN_START_STOP),
        ("start-stop-no-seal-in", DRAWN_NO_SEAL_IN),
        ("two-islands", DRAWN_TWO_ISLANDS),
        ("two-islands-N-PE", DRAWN_ISLANDS_N_PE),
    ];
    assert_eq!(all.len(), pairs().len(), "every fixture circuit is drawn");

    for (id, text) in all {
        let pair = pair(id);
        let request = drawn(text);

        // The independent observations and events, applied to the drawing.
        let first = replay(&pair, request.clone());
        // The same inputs and events again: the same bits, in this environment.
        let second = replay(&pair, request.clone());
        assert_eq!(bits(&first), bits(&second), "{id}: deterministic");
        assert_eq!(first.snapshots.len(), second.snapshots.len());

        // The explicitly wired reference of the same circuit, within the contract's tolerance.
        let wired = replay(&pair, wire_request(&pair));
        assert_eq!(first.layout, wired.layout, "{id}: one layout");
        for (rail, wire) in first.snapshots.iter().zip(&wired.snapshots) {
            assert_eq!(rail.states, wire.states, "{id}: discrete states");
            for (a, b) in rail.values.iter().zip(&wire.values) {
                assert_close(
                    *a,
                    *b,
                    abs + rel * b.abs(),
                    &format!("{id}: drawn vs wired"),
                );
            }
        }

        // Only the warnings the fixture expects.
        let run = Run::start_every_step(request);
        let CompileResult::Success { diagnostics, .. } = &run.compile_result else {
            panic!("{id} compiles");
        };
        let mut warnings = diagnostics
            .iter()
            .map(|diagnostic| format!("{:?}", diagnostic.code))
            .collect::<Vec<_>>();
        warnings.sort();
        let mut expected = pair["expectedWarningCodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(text_of)
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(warnings, expected, "{id}: warnings");
    }
}

fn text_of(value: &Value) -> String {
    value.as_str().expect("a string").to_owned()
}
