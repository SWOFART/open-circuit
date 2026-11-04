//! Issue #100: editor-drawn circuits, canonical analytical answers, real Engine/bridge,
//! and a recording replayed by stage6EndToEnd.test.tsx. No production doubles are used.
mod common;

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use common::{Capture, Run, assert_close, hex};
use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_app_lib::simulation::{SimulationConfig, SimulationCore};
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, OperatorPosition, SetOperatorStateRequest,
    SetOperatorStateResult, component_catalog,
};
use serde_json::{Value, json};

const SWITCH: &str = include_str!("../../../tests/golden/stage-6-drawn-switch.request.json");
const BUTTON: &str = include_str!("../../../tests/golden/stage-6-drawn-button.request.json");
const LATCHING: &str = include_str!("../../../tests/golden/stage-6-drawn-latching.request.json");
const UNLATCHED: &str = include_str!("../../../tests/golden/stage-6-drawn-unlatched.request.json");
const SWITCH_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-6-switch-divider.expected.json");
const BUTTON_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-6-button-capacitor-hold.expected.json");
const LATCHING_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-6-start-stop-latching.expected.json");

#[test]
fn stiff_control_circuit_catch_up_yields_before_starving_operator_requests() {
    let request: CompileRequest = serde_json::from_str(LATCHING).unwrap();
    let start_id = request.schematic.components[2].id().clone();
    let mut core = SimulationCore::new(SimulationConfig::default());
    let CompileResult::Success { run_id, .. } = core.compile(request) else {
        panic!("the drawn circuit must compile");
    };
    let frames = Capture::default();
    let now = Instant::now();
    core.start(run_id.clone(), Box::new(frames.clone()), now)
        .unwrap();
    // A long wall-clock stall must not demand a thousand stiff steps before the
    // worker can receive the user's START (or, critically, release) request.
    core.tick(now + Duration::from_secs(1));
    let snapshot = decode_frame(&frames.take().pop().unwrap()).unwrap();
    assert!(snapshot.metadata.simulation_step > 0);
    assert!(snapshot.metadata.simulation_step < 1000);
    for (expected, next, position) in [
        ("rev-1", "rev-2", OperatorPosition::Pressed),
        ("rev-2", "rev-3", OperatorPosition::Released),
    ] {
        assert!(matches!(
            core.set_operator_state(SetOperatorStateRequest {
                run_id: run_id.clone(),
                expected_document_revision: expected.to_owned(),
                document_revision: next.to_owned(),
                component_id: start_id.clone(),
                position,
            }),
            SetOperatorStateResult::Success { .. }
        ));
    }
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn shift_drawing(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            if fields.contains_key("x") && fields.contains_key("y") {
                fields["x"] = json!(fields["x"].as_i64().unwrap() + 1);
            } else {
                for child in fields.values_mut() {
                    shift_drawing(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                shift_drawing(item);
            }
        }
        _ => {}
    }
}

fn execute(text: &str, expected_text: &str, wrong: bool) -> (Value, Value) {
    let request: CompileRequest = serde_json::from_str(text).unwrap();
    let expected = parse(expected_text);
    let contact = request.schematic.components[1].id().clone();
    let start = request.schematic.components[2].id().clone();
    let device = request.schematic.components[3].id().clone();
    let mut run = Run::start_every_step(request.clone());
    let mut snapshots = vec![run.initial_snapshot.clone()];
    let mut updates = Vec::new();
    let mut frames = Vec::new();
    let last = if expected_text == BUTTON_EXPECTED {
        200
    } else {
        25
    };

    for step in 1..=last {
        run.advance(1);
        // Retain the pre-update frame too: the replay must reject it after the revision changes.
        let before = run.frame_at(step).clone();
        let mut snapshot = before.0.clone();
        for event in expected["operatorEvents"].as_array().unwrap() {
            if event["sentWhenNewestStep"].as_u64() != Some(step) {
                continue;
            }
            let id = if event["component"] == "START" {
                &start
            } else {
                &contact
            };
            let position: OperatorPosition =
                serde_json::from_value(event["position"].clone()).unwrap();
            let (sent, reply) = run.set_operator_state(id.as_str(), position);
            let SetOperatorStateResult::Success {
                snapshot: updated, ..
            } = &reply
            else {
                panic!("operator update failed: {reply:?}");
            };
            assert_eq!(updated.metadata.simulation_step, step);
            assert_eq!(updated.metadata.run_id, run.run_id);
            snapshot = updated.clone();
            updates.push(json!({"afterStep": step, "request": sent, "reply": reply}));
        }
        snapshots.push(snapshot);
        frames.push(json!({"step": step, "hex": hex(&before.1)}));
    }

    let rows = if wrong {
        &expected["wrongVariants"][0]["expected"]
    } else {
        &expected["expected"]
    };
    let key = if expected_text == LATCHING_EXPECTED {
        format!("{device}/coil/current")
    } else {
        format!("{device}:voltage")
    };
    let offset = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap()
        .offset as usize;
    for row in rows.as_array().unwrap() {
        let step = row["step"].as_u64().unwrap() as usize;
        let snapshot = &snapshots[step];
        let states = json!(snapshot.states);
        let state = states
            .as_array()
            .unwrap()
            .iter()
            .find(|s| {
                s["componentId"]
                    == if expected_text == LATCHING_EXPECTED {
                        json!(device)
                    } else {
                        json!(contact)
                    }
            })
            .unwrap();
        if expected_text == LATCHING_EXPECTED {
            assert_eq!(state["state"], row["contactor"], "step {step}");
            assert_close(
                snapshot.values[offset],
                row["coilCurrentAmps"].as_f64().unwrap(),
                expected["coilCurrentToleranceAmps"].as_f64().unwrap(),
                "coil current",
            );
        } else {
            let button = expected_text == BUTTON_EXPECTED;
            assert_eq!(
                state["position"],
                row[if button {
                    "buttonPosition"
                } else {
                    "switchPosition"
                }]
            );
            assert_eq!(
                state["conducting"],
                row[if button {
                    "buttonConducting"
                } else {
                    "switchConducting"
                }]
            );
            assert_close(
                snapshot.values[offset],
                row[if button {
                    "capacitorVolts"
                } else {
                    "outputVolts"
                }]
                .as_f64()
                .unwrap(),
                row[if button {
                    "toleranceVolts"
                } else {
                    "voltToleranceVolts"
                }]
                .as_f64()
                .unwrap(),
                "load voltage",
            );
            if let Some(current) = row["currentAmps"].as_f64() {
                let slot = run
                    .layout
                    .slots
                    .iter()
                    .find(|slot| slot.key == format!("{contact}:current"))
                    .unwrap();
                assert_close(
                    snapshot.values[slot.offset as usize],
                    current,
                    row["currentToleranceAmps"].as_f64().unwrap(),
                    "contact current",
                );
            }
        }
    }

    // History must hold the recomputed sample at each update step, never its old value.
    let samples = run.samples(std::slice::from_ref(&key), 0, last);
    for (step, values) in &samples {
        assert_eq!(
            values[0].to_bits(),
            snapshots[*step as usize].values[offset].to_bits()
        );
    }
    // Determinism excludes transport IDs/sequences; all numeric bits and discrete states count.
    let timeline = json!(
        snapshots
            .iter()
            .map(|s| json!({
                "step": s.metadata.simulation_step,
                "bits": s.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "states": s.states,
            }))
            .collect::<Vec<_>>()
    );

    // Activate the device again before save/recompile, so reset is checked against an
    // actually changed position (and a picked contactor), rather than an already idle run.
    let (active_id, position) = if expected_text == LATCHING_EXPECTED {
        (&start, OperatorPosition::Pressed)
    } else if expected_text == BUTTON_EXPECTED {
        (&contact, OperatorPosition::Pressed)
    } else {
        (&contact, OperatorPosition::Closed)
    };
    let (sent, reply) = run.set_operator_state(active_id.as_str(), position);
    let SetOperatorStateResult::Success { snapshot, .. } = &reply else {
        panic!("activate before reset: {reply:?}");
    };
    assert_ne!(json!(snapshot.states), json!(run.initial_snapshot.states));
    updates.push(json!({"afterStep": last, "request": sent, "reply": reply}));

    // Saving an active run still writes exactly the drawn schematic.
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "stage6-project-{:?}-{}.ocircuit",
        request.schematic.components[1].type_id(),
        wrong
    ));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_project_file(&path, &request.schematic).unwrap();
    let project_text = fs::read_to_string(&path).unwrap();
    let reopened = read_project_file(&path).unwrap();
    assert_eq!(json!(reopened), json!(request.schematic));
    assert_eq!(parse(&project_text), json!(request.schematic));
    fs::remove_file(path).unwrap();

    // A new revision and run resets every discrete state; the old run's update is refused.
    let mut shifted = json!(request);
    shift_drawing(&mut shifted);
    let mut reset_request: CompileRequest = serde_json::from_value(shifted).unwrap();
    reset_request.document_revision = format!("rev-{}", updates.len() + 2);
    let reset_reply = run.core.compile(reset_request.clone());
    let CompileResult::Success {
        run_id,
        initial_snapshot,
        ..
    } = &reset_reply
    else {
        panic!("reopened circuit must compile");
    };
    assert_ne!(run_id, &run.run_id);
    assert_eq!(
        json!(initial_snapshot.states),
        json!(run.initial_snapshot.states)
    );
    assert_eq!(initial_snapshot.values, run.initial_snapshot.values);
    let stale: SetOperatorStateRequest =
        serde_json::from_value(updates[0]["request"].clone()).unwrap();
    let SetOperatorStateResult::Failure { diagnostics } = run.core.set_operator_state(stale) else {
        panic!("old run request must be refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);

    (
        json!({"request": request, "compileReply": run.compile_result, "frames": frames,
        "updates": updates, "resetRequest": reset_request, "resetReply": reset_reply,
        "projectText": project_text}),
        timeline,
    )
}

#[test]
fn editor_drawn_control_circuits_match_the_contract_and_record_the_real_engine_session() {
    let mut circuits = serde_json::Map::new();
    for (name, text, expected, wrong) in [
        ("switch", SWITCH, SWITCH_EXPECTED, false),
        ("button", BUTTON, BUTTON_EXPECTED, false),
        ("latching", LATCHING, LATCHING_EXPECTED, false),
        ("unlatched", UNLATCHED, LATCHING_EXPECTED, true),
    ] {
        let (record, timeline) = execute(text, expected, wrong);
        let (_, repeated) = execute(text, expected, wrong);
        assert_eq!(timeline, repeated, "{name}: bit-identical repeated runs");
        circuits.insert(name.to_owned(), record);
    }
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("stage6-session.json");
    fs::write(
        path,
        serde_json::to_string_pretty(&json!({
            "catalog": component_catalog(), "circuits": circuits,
        }))
        .unwrap(),
    )
    .unwrap();
}
