//! The Stage 6 bridge (issue #98): switches, push buttons, and a contactor run by the real
//! Engine behind `SimulationCore`, with position changes carried the way the application
//! carries them: a `set_operator_state` request, JSON results, and binary OCF2 frames whose
//! state table holds the positions. The circuits are those of the Stage 6 fixtures
//! (`tests/golden/stage-6-*.expected.json`), written here as compile requests.

mod common;

use common::{Run, assert_close, value_of};
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_app_lib::simulation::{SimulationConfig, SimulationService};
use open_circuit_engine::{
    CompileRequest, CompileResult, DeviceState, DiagnosticCode, OperatorPosition,
    PushButtonPosition, RelayContactState, ResultQuantity, ResultSnapshot, SetOperatorStateRequest,
    SetOperatorStateResult, StableId, SwitchPosition,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const CONTACT: &str = "00000000-0000-4000-8000-000000000002";
const R1: &str = "00000000-0000-4000-8000-000000000003";
const R2: &str = "00000000-0000-4000-8000-000000000004";
const STOP: &str = "00000000-0000-4000-8000-000000000012";
const START: &str = "00000000-0000-4000-8000-000000000013";
const CONTACTOR: &str = "00000000-0000-4000-8000-000000000014";
const MARKER: &str = "00000000-0000-4000-8000-000000000009";

fn point(x: i32, y: i32) -> Value {
    json!({ "x": x, "y": y })
}

fn component(type_id: &str, id: &str, at: Value, parameters: Value) -> Value {
    json!({
        "typeId": type_id,
        "id": id,
        "position": at,
        "rotationDeg": 0,
        "parameters": parameters,
    })
}

/// Geometry of a component: the grid point of each of its named pins.
fn pins(id: &str, pins: &[(&str, Value)]) -> Value {
    json!({
        "componentId": id,
        "pins": pins
            .iter()
            .map(|(pin, at)| json!({ "pinId": pin, "position": at }))
            .collect::<Vec<_>>(),
    })
}

fn request(components: Vec<Value>, geometry: Vec<Value>, ground: Value) -> CompileRequest {
    serde_json::from_value(json!({
        "schematic": {
            "schemaVersion": 1,
            "components": components,
            "wires": [],
            "junctions": [],
            "referenceMarkers": [{ "id": MARKER, "position": ground }],
        },
        "geometry": { "components": geometry },
        "documentRevision": "rev-1",
    }))
    .expect("a compile request")
}

/// An ideal 10 V source, a switch, and two 1 kOhm resistors in series.
fn divider(initial_closed: Option<f64>) -> CompileRequest {
    let (p0, p4, p6, ground) = (point(0, 0), point(4, 0), point(6, 0), point(0, 10));
    let mut switch_parameters = json!({ "closedResistanceOhms": 0.01 });
    if let Some(value) = initial_closed {
        switch_parameters["initialClosed"] = json!(value);
    }
    request(
        vec![
            component(
                "dc_voltage_source",
                SOURCE,
                p0.clone(),
                json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
            ),
            component("switch_spst", CONTACT, p0.clone(), switch_parameters),
            component(
                "resistor",
                R1,
                p4.clone(),
                json!({ "resistanceOhms": 1000.0 }),
            ),
            component(
                "resistor",
                R2,
                p6.clone(),
                json!({ "resistanceOhms": 1000.0 }),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", p0.clone()), ("pin2", ground.clone())]),
            pins(CONTACT, &[("pin1", p0), ("pin2", p4.clone())]),
            pins(R1, &[("pin1", p4), ("pin2", p6.clone())]),
            pins(R2, &[("pin1", p6), ("pin2", ground.clone())]),
        ],
        ground,
    )
}

/// The start/stop circuit of `stage-6-start-stop-latching`: a 5 V source, a normally closed
/// STOP button, START in parallel with the contactor's normally open contact, and the coil.
fn start_stop() -> CompileRequest {
    let (p0, p1, p2, ground) = (point(0, 0), point(4, 0), point(8, 0), point(0, 10));
    let button = json!({ "closedResistanceOhms": 0.01 });
    request(
        vec![
            component(
                "dc_voltage_source",
                SOURCE,
                p0.clone(),
                json!({ "voltageVolts": 5.0, "internalResistanceOhms": 0.0 }),
            ),
            component("push_button_nc", STOP, p0.clone(), button.clone()),
            component("push_button_no", START, p1.clone(), button),
            component(
                "contactor",
                CONTACTOR,
                p2.clone(),
                json!({
                    "coilResistanceOhms": 100.0,
                    "coilInductanceHenries": 0.01,
                    "pickupCurrentAmps": 0.04,
                    "dropoutCurrentAmps": 0.02,
                    "closedContactResistanceOhms": 0.1,
                }),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", p0.clone()), ("pin2", ground.clone())]),
            pins(STOP, &[("pin1", p0), ("pin2", p1.clone())]),
            pins(START, &[("pin1", p1.clone()), ("pin2", p2.clone())]),
            pins(
                CONTACTOR,
                &[
                    ("a1", p2.clone()),
                    ("a2", ground.clone()),
                    ("no_1", p1),
                    ("no_2", p2),
                    ("nc_1", point(40, 0)),
                    ("nc_2", point(42, 0)),
                ],
            ),
        ],
        ground,
    )
}

fn switch_state(snapshot: &ResultSnapshot) -> (SwitchPosition, bool, Option<f64>) {
    match snapshot.states.as_slice() {
        [
            DeviceState::Switch {
                position,
                conducting,
                last_transition_time_seconds,
                ..
            },
        ] => (*position, *conducting, *last_transition_time_seconds),
        other => panic!("the snapshot holds exactly one switch state, not {other:?}"),
    }
}

fn contactor_state(snapshot: &ResultSnapshot) -> RelayContactState {
    snapshot
        .states
        .iter()
        .find_map(|state| match state {
            DeviceState::Contactor { state, .. } => Some(*state),
            _ => None,
        })
        .expect("the snapshot holds a contactor state")
}

fn success(result: SetOperatorStateResult) -> ResultSnapshot {
    match result {
        SetOperatorStateResult::Success { snapshot, .. } => snapshot,
        SetOperatorStateResult::Failure { diagnostics } => {
            panic!("the position change should succeed: {diagnostics:?}")
        }
    }
}

fn refusal(result: SetOperatorStateResult) -> DiagnosticCode {
    match result {
        SetOperatorStateResult::Failure { diagnostics } => diagnostics[0].code,
        SetOperatorStateResult::Success { .. } => panic!("the request should be refused"),
    }
}

#[test]
fn a_position_change_reaches_the_result_and_every_later_frame_and_the_stream_goes_on() {
    let mut run = Run::start_every_step(divider(None));
    run.advance(3);
    assert_eq!(
        switch_state(&run.initial_snapshot),
        (SwitchPosition::Open, false, None)
    );

    let (_, result) = run.set_operator_state(CONTACT, OperatorPosition::Closed);
    let snapshot = success(result);
    assert_eq!(snapshot.metadata.document_revision, "rev-2");
    let (position, conducting, transition) = switch_state(&snapshot);
    assert_eq!((position, conducting), (SwitchPosition::Closed, true));
    assert!(transition.is_some());
    assert_close(
        value_of(&run.layout, &snapshot, R2, ResultQuantity::Voltage),
        4.999975000125,
        1e-9,
        "output of the divider in the result",
    );

    let before = run.delivered.len();
    run.advance(5);
    assert!(
        run.delivered.len() > before,
        "the stream keeps delivering frames"
    );

    // A frame never shows the state of another revision: frames of the old revision say
    // open, frames of the new one say closed, and the values agree with the state.
    let mut closed_frames = 0;
    for (frame, _) in &run.delivered[before..] {
        assert_eq!(frame.metadata.document_revision, "rev-2");
        assert_eq!(switch_state(frame).0, SwitchPosition::Closed);
        assert_close(
            value_of(&run.layout, frame, R2, ResultQuantity::Voltage),
            4.999975000125,
            1e-9,
            "output of the divider in a later frame",
        );
        closed_frames += 1;
    }
    assert!(closed_frames > 0);
    for (frame, _) in &run.delivered[..before] {
        assert_eq!(frame.metadata.document_revision, "rev-1");
        assert_eq!(switch_state(frame).0, SwitchPosition::Open);
    }

    // The state table in the bytes: kind 3, closed, with a transition time and `conducting`.
    let bytes = &run.delivered.last().unwrap().1;
    let record = &bytes[bytes.len() - 32..];
    assert_eq!(&record[16..20], &[3, 1, 3, 0]);

    // Opening it again goes the same way.
    let snapshot = success(run.set_operator_state(CONTACT, OperatorPosition::Open).1);
    assert_eq!(snapshot.metadata.document_revision, "rev-3");
    assert_eq!(switch_state(&snapshot).0, SwitchPosition::Open);
    assert_close(
        value_of(&run.layout, &snapshot, R2, ResultQuantity::Voltage),
        0.0,
        1e-12,
        "output with the switch open",
    );
}

#[test]
fn a_refused_request_leaves_the_run_the_stream_and_the_revision_as_they_were() {
    let mut run = Run::start_every_step(divider(None));
    run.advance(3);
    let _ = success(run.set_operator_state(CONTACT, OperatorPosition::Closed).1);
    run.advance(2);

    let current = |run: &Run| run.revision();
    assert_eq!(current(&run), "rev-2");

    // A request for another run.
    let other_run = SetOperatorStateRequest {
        run_id: StableId::parse("00000000-0000-4000-8000-0000000000aa").unwrap(),
        expected_document_revision: current(&run),
        document_revision: "rev-3".to_owned(),
        component_id: StableId::parse(CONTACT).unwrap(),
        position: OperatorPosition::Closed,
    };
    assert_eq!(
        refusal(run.core.set_operator_state(other_run)),
        DiagnosticCode::StaleRun
    );

    // An old revision, a revision that is reused, and one too long for a frame.
    let request_for = |run: &Run, expected: &str, next: &str, component: &str, position| {
        SetOperatorStateRequest {
            run_id: run.run_id.clone(),
            expected_document_revision: expected.to_owned(),
            document_revision: next.to_owned(),
            component_id: StableId::parse(component).unwrap(),
            position,
        }
    };
    assert_eq!(
        refusal(run.core.set_operator_state(request_for(
            &run,
            "rev-1",
            "rev-3",
            CONTACT,
            OperatorPosition::Closed
        ))),
        DiagnosticCode::StaleDocumentRevision
    );
    assert_eq!(
        refusal(run.core.set_operator_state(request_for(
            &run,
            "rev-2",
            "rev-2",
            CONTACT,
            OperatorPosition::Closed
        ))),
        DiagnosticCode::StaleDocumentRevision
    );
    assert_eq!(
        refusal(run.core.set_operator_state(request_for(
            &run,
            "rev-2",
            &"x".repeat(300),
            CONTACT,
            OperatorPosition::Closed
        ))),
        DiagnosticCode::InvalidParameter
    );

    // A part that cannot be operated, and a position its type does not have.
    assert_eq!(
        refusal(run.core.set_operator_state(request_for(
            &run,
            "rev-2",
            "rev-3",
            R1,
            OperatorPosition::Closed
        ))),
        DiagnosticCode::InvalidParameter
    );
    assert_eq!(
        refusal(run.core.set_operator_state(request_for(
            &run,
            "rev-2",
            "rev-3",
            CONTACT,
            OperatorPosition::Pressed
        ))),
        DiagnosticCode::InvalidParameter
    );

    // Nothing changed: the stream still delivers frames of rev-2, with the switch closed, and
    // the next valid request still expects rev-2.
    let before = run.delivered.len();
    run.advance(3);
    assert!(run.delivered.len() > before);
    for (frame, _) in &run.delivered[before..] {
        assert_eq!(frame.metadata.document_revision, "rev-2");
        assert_eq!(switch_state(frame).0, SwitchPosition::Closed);
    }
    let snapshot = success(run.set_operator_state(CONTACT, OperatorPosition::Open).1);
    assert_eq!(snapshot.metadata.document_revision, "rev-3");
}

#[test]
fn a_new_run_starts_at_the_initial_positions_and_the_old_run_is_stale() {
    let mut run = Run::start_every_step(divider(Some(1.0)));
    assert_eq!(
        switch_state(&run.initial_snapshot),
        (SwitchPosition::Closed, true, None)
    );
    run.advance(2);
    let _ = success(run.set_operator_state(CONTACT, OperatorPosition::Open).1);
    let old_run = run.run_id.clone();

    // A successful compile is a new run: the position is the saved one again, and a request
    // for the earlier run is refused.
    let CompileResult::Success {
        run_id,
        initial_snapshot,
        ..
    } = run.core.compile(divider(Some(1.0)))
    else {
        panic!("the divider should compile again");
    };
    assert_ne!(run_id, old_run);
    assert_eq!(switch_state(&initial_snapshot).0, SwitchPosition::Closed);
    let stale = SetOperatorStateRequest {
        run_id: old_run,
        expected_document_revision: "rev-1".to_owned(),
        document_revision: "rev-9".to_owned(),
        component_id: StableId::parse(CONTACT).unwrap(),
        position: OperatorPosition::Open,
    };
    assert_eq!(
        refusal(run.core.set_operator_state(stale)),
        DiagnosticCode::StaleRun
    );
}

#[test]
fn the_service_carries_a_request_through_the_simulation_thread() {
    let service = SimulationService::spawn(SimulationConfig::default());
    let CompileResult::Success { run_id, .. } = service.compile(divider(None)).unwrap() else {
        panic!("the divider should compile");
    };
    let request = SetOperatorStateRequest {
        run_id: run_id.clone(),
        expected_document_revision: "rev-1".to_owned(),
        document_revision: "rev-2".to_owned(),
        component_id: StableId::parse(CONTACT).unwrap(),
        position: OperatorPosition::Closed,
    };
    let snapshot = success(service.set_operator_state(request.clone()).unwrap());
    assert_eq!(switch_state(&snapshot).0, SwitchPosition::Closed);
    // The same request again names a revision the run no longer has.
    assert_eq!(
        refusal(service.set_operator_state(request).unwrap()),
        DiagnosticCode::StaleDocumentRevision
    );
    service.shutdown();
}

#[test]
fn a_start_stop_circuit_latches_through_the_bridge() {
    let mut run = Run::start_every_step(start_stop());
    run.advance(5);
    assert_eq!(
        contactor_state(&run.initial_snapshot),
        RelayContactState::Released
    );

    let coil = |snapshot: &ResultSnapshot, run: &Run| {
        run.layout
            .slots
            .iter()
            .find(|slot| slot.key == format!("{CONTACTOR}/coil/current"))
            .map(|slot| snapshot.values[slot.offset as usize])
            .expect("a coil current slot")
    };

    let pressed = success(run.set_operator_state(START, OperatorPosition::Pressed).1);
    assert_eq!(contactor_state(&pressed), RelayContactState::Picked);
    assert_close(
        coil(&pressed, &run),
        0.04999045636742077,
        1e-4,
        "coil current, START held",
    );

    run.advance(5);
    let released = success(run.set_operator_state(START, OperatorPosition::Released).1);
    assert_eq!(contactor_state(&released), RelayContactState::Picked);
    run.advance(2);
    let (latched, _) = run.delivered.last().unwrap();
    assert_eq!(contactor_state(latched), RelayContactState::Picked);
    assert_close(
        coil(latched, &run),
        0.04994506043352313,
        1e-4,
        "coil current, latched",
    );

    run.advance(3);
    let stopped = success(run.set_operator_state(STOP, OperatorPosition::Pressed).1);
    assert_eq!(contactor_state(&stopped), RelayContactState::Released);
    assert_close(coil(&stopped, &run), 0.0, 1e-4, "coil current, STOP held");
    // A press and a release while one step is the newest leave no trace (contract section
    // 4.2), so the button is held for a few steps.
    run.advance(3);
    let _ = success(run.set_operator_state(STOP, OperatorPosition::Released).1);
    run.advance(5);
    let (off, _) = run.delivered.last().unwrap();
    assert_eq!(contactor_state(off), RelayContactState::Released);

    // The push buttons report their own states in the same table.
    let buttons = off
        .states
        .iter()
        .filter(|state| matches!(state, DeviceState::PushButton { .. }))
        .count();
    assert_eq!(buttons, 2);
    assert!(off.states.iter().any(|state| matches!(
        state,
        DeviceState::PushButton {
            position: PushButtonPosition::Released,
            conducting: true,
            ..
        }
    )));
}

#[test]
fn the_project_file_keeps_only_the_initial_position() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("control.ocircuit");

    // A closed switch is saved as `initialClosed: 1`; a position reached while running
    // never reaches the file, because the Engine's run owns it.
    let mut run = Run::start_every_step(divider(Some(1.0)));
    run.advance(2);
    let _ = success(run.set_operator_state(CONTACT, OperatorPosition::Open).1);
    let CompileRequest { schematic, .. } = divider(Some(1.0));
    write_project_file(&path, &schematic).expect("the project should be saved");

    let text = std::fs::read_to_string(&path).unwrap();
    for computed in ["conducting", "pressed", "picked", "operator", "released"] {
        assert!(!text.contains(computed), "the file contains {computed}");
    }
    let saved: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(saved["components"][1]["typeId"], "switch_spst");
    assert_eq!(
        saved["components"][1]["parameters"],
        json!({ "closedResistanceOhms": 0.01, "initialClosed": 1.0 })
    );

    // It reads back identically, and compiling it starts the switch closed again.
    let loaded = read_project_file(&path).expect("the project should open");
    assert_eq!(loaded, schematic);

    // The control devices round-trip too: two buttons and a contactor.
    let control = start_stop().schematic;
    let control_path = directory.path().join("start-stop.ocircuit");
    write_project_file(&control_path, &control).unwrap();
    assert_eq!(read_project_file(&control_path).unwrap(), control);

    // A type this release does not know is refused and never read as something else.
    let unknown = text.replace("switch_spst", "switch_dpdt");
    let unknown_path = directory.path().join("unknown.ocircuit");
    std::fs::write(&unknown_path, unknown).unwrap();
    assert!(read_project_file(&unknown_path).is_err());

    // A run of the loaded project starts at the saved position.
    let reopened = Run::start_every_step(CompileRequest {
        schematic: loaded,
        ..divider(Some(1.0))
    });
    assert_eq!(
        switch_state(&reopened.initial_snapshot).0,
        SwitchPosition::Closed
    );
}
