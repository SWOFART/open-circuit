use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, Engine, ParameterId,
    SetParameterRequest, SetParameterResult, StableId,
};
use serde_json::json;

const SOURCE: &str = "00000000-0000-4000-8000-000000001400";
const TIMER: &str = "00000000-0000-4000-8000-000000001401";

fn request(type_id: &str, delay: f64) -> CompileRequest {
    let mut schematic: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.schematic.json"
    ))
    .unwrap();
    schematic["components"] = json!([
        {"id": SOURCE, "position":{"x":0,"y":-2}, "rotationDeg":0,
         "typeId":"dc_voltage_source", "parameters":{"voltageVolts":24,"internalResistanceOhms":0.1}},
        {"id": TIMER, "position":{"x":0,"y":0}, "rotationDeg":0,
         "typeId": type_id, "parameters":{"coilResistanceOhms":240,
         "pickupCurrentAmps":0.08,"dropoutCurrentAmps":0.04,"closedResistanceOhms":0.01,
         "delaySeconds":delay}}
    ]);
    schematic["wires"] = json!([]);
    schematic["referenceMarkers"] = json!([]);
    serde_json::from_value(json!({
        "schematic": schematic,
        "geometry": {"components":[
            {"componentId":SOURCE,"pins":[
                {"pinId":"pin1","position":{"x":-3,"y":-2}},
                {"pinId":"pin2","position":{"x":3,"y":-2}}
            ]},
            {"componentId":TIMER,"pins":[
                {"pinId":"a1","position":{"x":-3,"y":-2}}, {"pinId":"a2","position":{"x":3,"y":-2}},
                {"pinId":"no1","position":{"x":-3,"y":0}}, {"pinId":"no2","position":{"x":3,"y":0}},
                {"pinId":"nc1","position":{"x":-3,"y":2}}, {"pinId":"nc2","position":{"x":3,"y":2}}
            ]}
        ]},
        "documentRevision":"time-relay"
    }))
    .unwrap()
}

#[test]
fn on_delay_steps_match_fractional_deadline_and_batching() {
    let mut engine = Engine::default();
    let mut batch = Engine::default();
    let compile = request("relay_on_delay", 0.0035);
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = engine.compile(compile.clone())
    else {
        panic!("timer failed to compile");
    };
    let CompileResult::Success {
        run_id: batch_id, ..
    } = batch.compile(compile)
    else {
        panic!("timer failed to compile for batched advance");
    };
    assert!(matches!(
        initial_snapshot.states.as_slice(),
        [DeviceState::RelayOnDelay {
            energized: true,
            output_on: false,
            timing: true,
            ..
        }]
    ));
    let elapsed = layout
        .slots
        .iter()
        .find(|slot| slot.key.ends_with("/timerElapsedSeconds"))
        .unwrap();
    for index in 1..=3 {
        let frame = engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        assert!((frame.values[elapsed.offset as usize] - f64::from(index) * 0.001).abs() < 1e-12);
    }
    let end = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1,
        })
        .unwrap();
    let batch_end = batch
        .advance(AdvanceRequest {
            run_id: batch_id,
            step_count: 4,
        })
        .unwrap();
    assert_eq!(end.values, batch_end.values);
    assert_eq!(end.states, batch_end.states);
    assert!(matches!(
        end.states.as_slice(),
        [DeviceState::RelayOnDelay {
            output_on: true,
            timing: false,
            last_transition_time_seconds: Some(0.004),
            ..
        }]
    ));
}

#[test]
fn off_delay_picks_at_compile_and_zero_on_delay_also_picks() {
    for type_id in ["relay_off_delay", "relay_on_delay"] {
        let result = Engine::default().compile(request(type_id, 0.0));
        let CompileResult::Success {
            initial_snapshot, ..
        } = result
        else {
            panic!("timer failed to compile: {result:?}");
        };
        assert!(matches!(
            initial_snapshot.states.as_slice(),
            [DeviceState::RelayOffDelay {
                output_on: true,
                ..
            } | DeviceState::RelayOnDelay {
                output_on: true,
                ..
            }]
        ));
    }
}

#[test]
fn off_delay_counts_from_dropout_and_refuses_live_delay_changes() {
    let mut engine = Engine::default();
    let input = request("relay_off_delay", 0.003);
    let CompileResult::Success { run_id, .. } = engine.compile(input.clone()) else {
        panic!("timer failed to compile");
    };
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 1,
        })
        .unwrap();
    let edit = |id: &str, parameter_id, value| SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "time-relay".into(),
        document_revision: "dropout".into(),
        component_id: StableId::parse(id).unwrap(),
        parameter_id,
        value,
    };
    assert!(matches!(
        engine.set_parameter(edit(TIMER, ParameterId::DelaySeconds, 0.1)),
        SetParameterResult::Failure { .. }
    ));
    let SetParameterResult::Success { snapshot, .. } =
        engine.set_parameter(edit(SOURCE, ParameterId::VoltageVolts, 0.0))
    else {
        panic!("source voltage edit failed");
    };
    assert!(matches!(
        snapshot.states.as_slice(),
        [DeviceState::RelayOffDelay {
            energized: false,
            timing: true,
            output_on: true,
            ..
        }]
    ));
    let end = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 3,
        })
        .unwrap();
    assert!(matches!(
        end.states.as_slice(),
        [DeviceState::RelayOffDelay {
            energized: false,
            timing: false,
            output_on: false,
            last_transition_time_seconds: Some(0.004),
            ..
        }]
    ));
    let CompileResult::Success {
        initial_snapshot, ..
    } = engine.compile(input)
    else {
        panic!("timer reset compile failed");
    };
    assert!(matches!(
        initial_snapshot.states.as_slice(),
        [DeviceState::RelayOffDelay {
            energized: true,
            timing: false,
            output_on: true,
            ..
        }]
    ));
}
