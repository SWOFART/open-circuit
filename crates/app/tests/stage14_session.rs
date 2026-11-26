//! A Stage 14 control circuit through project persistence and the real Engine/OCF2 bridge.

mod common;

use common::{Run, assert_close};
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_engine::{
    CompileRequest, DeviceState, DiagnosticCode, EmergencyStopPosition, OperatorPosition,
    ParameterId, ResultSnapshot, SetOperatorStateResult, SetParameterResult,
};
use serde_json::{Value, json};

const SUPPLY: &str = "00000000-0000-4000-8000-000000000001";
const STOP: &str = "00000000-0000-4000-8000-000000000002";
const TIMER: &str = "00000000-0000-4000-8000-000000000003";
const LAMP: &str = "00000000-0000-4000-8000-000000000004";

fn part(id: &str, kind: &str, parameters: Value) -> Value {
    json!({
        "id": id, "typeId": kind, "parameters": parameters,
        "position": {"x": 0, "y": 0}, "rotationDeg": 0,
    })
}

fn geometry(id: &str, pins: &[(&str, i32)]) -> Value {
    json!({
        "componentId": id,
        "pins": pins.iter().map(|(pin, x)| json!({
            "pinId": pin, "position": {"x": x, "y": 0}
        })).collect::<Vec<_>>(),
    })
}

/// The stop controls the timer coil. The timer's normally open contact powers a lamp.
fn circuit() -> CompileRequest {
    serde_json::from_value(json!({
        "documentRevision": "rev-1",
        "schematic": {
            "schemaVersion": 3,
            "components": [
                part(SUPPLY, "dc_voltage_source", json!({
                    "voltageVolts": 24.0, "internalResistanceOhms": 0.0,
                })),
                part(STOP, "emergency_stop", json!({"closedResistanceOhms": 0.01})),
                part(TIMER, "relay_on_delay", json!({
                    "coilResistanceOhms": 240.0, "pickupCurrentAmps": 0.08,
                    "dropoutCurrentAmps": 0.04, "closedResistanceOhms": 0.01,
                    "delaySeconds": 0.003,
                })),
                part(LAMP, "lamp", json!({"resistanceOhms": 120.0})),
            ],
            "wires": [], "junctions": [], "rails": [], "referenceMarkers": [],
            "documentation": {
                "designators": [], "wireNumbers": [], "terminals": [], "notes": [],
                "titleBlock": {"title": "", "drawingNumber": "", "revision": "", "author": "",
                    "company": "", "date": ""},
                "sheets": [],
            },
        },
        "geometry": {"components": [
            geometry(SUPPLY, &[("pin1", 1), ("pin2", 0)]),
            geometry(STOP, &[("p", 1), ("n", 2)]),
            geometry(TIMER, &[("a1", 2), ("a2", 0), ("no1", 1), ("no2", 3), ("nc1", 0), ("nc2", 0)]),
            geometry(LAMP, &[("p", 3), ("n", 0)]),
        ]},
    }))
    .expect("Stage 14 compile request")
}

fn assert_states(
    snapshot: &ResultSnapshot,
    stop: EmergencyStopPosition,
    energized: bool,
    on: bool,
    timing: bool,
) {
    assert!(snapshot.states.iter().any(|state| matches!(state,
        DeviceState::EmergencyStop { component_id, position, .. }
            if component_id.as_str() == STOP && *position == stop
    )));
    assert!(snapshot.states.iter().any(|state| matches!(state,
        DeviceState::RelayOnDelay { component_id, energized: actual_energized, output_on, timing: actual_timing, .. }
            if component_id.as_str() == TIMER && *actual_energized == energized && *output_on == on
                && *actual_timing == timing
    )));
    assert!(snapshot.states.iter().any(|state| matches!(state,
        DeviceState::Lamp { component_id, lit } if component_id.as_str() == LAMP && *lit == on
    )));
}

fn result(run: &Run, frame: &ResultSnapshot, component: &str, key: &str) -> f64 {
    let slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == format!("{component}/{key}"))
        .unwrap_or_else(|| panic!("missing {component}/{key}"));
    frame.values[slot.offset as usize]
}

#[test]
fn operator_commands_and_timer_results_pass_through_the_real_bridge() {
    let mut run = Run::start_every_step(circuit());
    assert_states(
        &run.initial_snapshot,
        EmergencyStopPosition::Armed,
        true,
        false,
        true,
    );
    run.advance(3);
    let pending = &run.frame_at(2).0;
    assert_states(pending, EmergencyStopPosition::Armed, true, false, true);
    assert_close(
        result(&run, pending, TIMER, "timerElapsedSeconds"),
        0.002,
        1e-12,
        "timer",
    );
    let on = &run.frame_at(3).0;
    assert_states(on, EmergencyStopPosition::Armed, true, true, false);
    let lamp_voltage: f64 = 24.0 * 120.0 / 120.01;
    assert_close(
        result(&run, on, LAMP, "powerWatts"),
        lamp_voltage.powi(2) / 120.0,
        1e-8,
        "lamp power",
    );

    let (_, edit) = run.set_parameter(TIMER, ParameterId::DelaySeconds, 0.005);
    assert!(matches!(edit, SetParameterResult::Failure { .. }));

    let (press, reply) = run.set_operator_state(STOP, OperatorPosition::Pressed);
    assert!(matches!(reply, SetOperatorStateResult::Success { .. }));
    let stale = run.core.set_operator_state(press);
    assert!(
        matches!(stale, SetOperatorStateResult::Failure { diagnostics }
            if diagnostics.iter().any(|diagnostic| diagnostic.code == DiagnosticCode::StaleDocumentRevision)
        )
    );
    let (_, release) = run.set_operator_state(STOP, OperatorPosition::Released);
    assert!(matches!(release, SetOperatorStateResult::Success { .. }));
    run.advance(1);
    assert_states(
        &run.frame_at(4).0,
        EmergencyStopPosition::Latched,
        false,
        false,
        false,
    );

    let (_, reset) = run.set_operator_state(STOP, OperatorPosition::Reset);
    assert!(matches!(reset, SetOperatorStateResult::Success { .. }));
    run.advance(3);
    assert_states(
        &run.frame_at(5).0,
        EmergencyStopPosition::Armed,
        true,
        false,
        true,
    );
    assert_states(
        &run.frame_at(7).0,
        EmergencyStopPosition::Armed,
        true,
        true,
        false,
    );
}

#[test]
fn save_and_reopen_restarts_stage14_state_from_drawn_inputs() {
    let request = circuit();
    let mut run = Run::start_every_step(request.clone());
    run.advance(3);
    let (_, reply) = run.set_operator_state(STOP, OperatorPosition::Pressed);
    assert!(matches!(reply, SetOperatorStateResult::Success { .. }));
    run.advance(1);
    assert_states(
        &run.frame_at(4).0,
        EmergencyStopPosition::Latched,
        false,
        false,
        false,
    );

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stage14.ocircuit");
    write_project_file(&path, &request.schematic).unwrap();
    let file: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(file, serde_json::to_value(&request.schematic).unwrap());
    let reopened = read_project_file(&path).unwrap();
    assert_eq!(reopened, request.schematic);
    let restarted = Run::start_every_step(CompileRequest {
        schematic: reopened,
        ..request
    });
    assert_eq!(restarted.initial_snapshot.metadata.simulation_step, 0);
    assert_states(
        &restarted.initial_snapshot,
        EmergencyStopPosition::Armed,
        true,
        false,
        true,
    );
}

#[test]
fn all_stage14_catalog_parameters_round_trip_in_a_project() {
    use open_circuit_engine::{ComponentSpec, component_catalog};

    let mut schematic = circuit().schematic;
    schematic.components.clear();
    let catalog = component_catalog();
    for (index, kind) in [
        "relay_on_delay",
        "relay_off_delay",
        "limit_switch",
        "emergency_stop",
        "selector_switch",
        "key_switch",
        "lamp",
        "indicator",
        "solenoid",
        "valve",
        "buzzer",
        "led",
        "zener_diode",
        "mosfet_n",
        "operational_amplifier",
        "frequency_converter",
    ]
    .into_iter()
    .enumerate()
    {
        let entry = catalog
            .component_types
            .iter()
            .find(|entry| serde_json::to_value(entry.type_id).unwrap() == kind)
            .unwrap_or_else(|| panic!("missing catalog entry {kind}"));
        let parameters = entry
            .parameters
            .iter()
            .map(|parameter| {
                let key: String =
                    serde_json::from_value(serde_json::to_value(parameter.id).unwrap()).unwrap();
                let value = match (kind, key.as_str()) {
                    ("selector_switch", "initialPosition") => 2.0,
                    ("key_switch", "initialPosition") => 1.0,
                    _ => parameter.default_value,
                };
                (key, json!(value))
            })
            .collect::<serde_json::Map<_, _>>();
        let id = format!("00000000-0000-4000-8000-{index:012}");
        let component: ComponentSpec = serde_json::from_value(part(&id, kind, json!(parameters)))
            .unwrap_or_else(|error| panic!("{kind}: {error}"));
        schematic.components.push(component);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stage14-catalog.ocircuit");
    write_project_file(&path, &schematic).unwrap();
    assert_eq!(read_project_file(&path).unwrap(), schematic);
}
