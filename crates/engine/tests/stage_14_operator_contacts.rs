use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, Engine, OperatorPosition,
    ResultLayout, ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult, StableId,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000001410";
const SWITCH: &str = "00000000-0000-4000-8000-000000001411";
const LOAD_1: &str = "00000000-0000-4000-8000-000000001412";
const LOAD_2: &str = "00000000-0000-4000-8000-000000001413";

fn pin(id: &str, x: i32, y: i32) -> Value {
    json!({"pinId": id, "position": {"x":x,"y":y}})
}

fn request(type_id: &str, position: Option<u8>) -> CompileRequest {
    let mut schematic: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.schematic.json"
    ))
    .unwrap();
    let mut parameters = json!({"closedResistanceOhms":0.01});
    if let Some(position) = position {
        parameters["initialPosition"] = json!(position);
    }
    let mut components = vec![
        json!({"id":SOURCE, "typeId":"dc_voltage_source","rotationDeg":0,"position":{"x":0,"y":2},
        "parameters":{"voltageVolts":10,"internalResistanceOhms":0.1}}),
        json!({"id":SWITCH, "typeId":type_id,"rotationDeg":0,"position":{"x":2,"y":0},"parameters":parameters}),
        json!({"id":LOAD_1,"typeId":"resistor","rotationDeg":0,"position":{"x":3,"y":-1},"parameters":{"resistanceOhms":1000}}),
    ];
    let switch_pins = if type_id == "selector_switch" {
        vec![pin("com", 0, 0), pin("throw1", 4, -1), pin("throw2", 4, 1)]
    } else {
        vec![pin("p", 0, 0), pin("n", 4, -1)]
    };
    let mut geometry = vec![
        json!({"componentId":SOURCE,"pins":[pin("pin1",0,0),pin("pin2",0,4)]}),
        json!({"componentId":SWITCH,"pins":switch_pins}),
        json!({"componentId":LOAD_1,"pins":[pin("pin1",4,-1),pin("pin2",0,4)]}),
    ];
    if type_id == "selector_switch" {
        components.push(json!({"id":LOAD_2,"typeId":"resistor","rotationDeg":0,
            "position":{"x":3,"y":1},"parameters":{"resistanceOhms":2000}}));
        geometry.push(json!({"componentId":LOAD_2,"pins":[pin("pin1",4,1),pin("pin2",0,4)]}));
    }
    schematic["components"] = json!(components);
    schematic["wires"] = json!([]);
    schematic["referenceMarkers"] = json!([]);
    serde_json::from_value(json!({
        "schematic":schematic,"geometry":{"components":geometry},"documentRevision":"operator-0"
    }))
    .unwrap()
}

fn compile(engine: &mut Engine, input: CompileRequest) -> (StableId, ResultLayout, ResultSnapshot) {
    let result = engine.compile(input);
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = result
    else {
        panic!("compile failed: {result:?}");
    };
    (run_id, layout, initial_snapshot)
}

fn command(run_id: &StableId, position: OperatorPosition) -> SetOperatorStateRequest {
    SetOperatorStateRequest {
        run_id: run_id.clone(),
        component_id: StableId::parse(SWITCH).unwrap(),
        position,
        expected_document_revision: "operator-0".into(),
        document_revision: "operator-1".into(),
    }
}

fn operate(
    engine: &mut Engine,
    run_id: &StableId,
    revision: &mut u32,
    position: OperatorPosition,
) -> ResultSnapshot {
    let mut request = command(run_id, position);
    request.expected_document_revision = format!("operator-{revision}");
    *revision += 1;
    request.document_revision = format!("operator-{revision}");
    let result = engine.set_operator_state(request);
    let SetOperatorStateResult::Success { snapshot, .. } = result else {
        panic!("operator command failed: {result:?}");
    };
    snapshot
}

fn current(layout: &ResultLayout, frame: &ResultSnapshot, key: &str) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.key == format!("{SWITCH}/{key}"))
        .unwrap();
    frame.values[slot.offset as usize]
}

#[test]
fn selector_moves_at_current_time_and_only_conducts_on_the_selected_throw() {
    let mut engine = Engine::default();
    let mut revision = 0;
    let (id, layout, init) = compile(&mut engine, request("selector_switch", None));
    assert_eq!(current(&layout, &init, "throw1CurrentAmps"), 0.0);
    assert_eq!(current(&layout, &init, "throw2CurrentAmps"), 0.0);
    engine
        .advance(AdvanceRequest {
            run_id: id.clone(),
            step_count: 5,
        })
        .unwrap();
    let left = operate(&mut engine, &id, &mut revision, OperatorPosition::Left);
    assert!((current(&layout, &left, "throw1CurrentAmps") - 10.0 / 1000.11).abs() < 1e-9);
    assert_eq!(current(&layout, &left, "throw2CurrentAmps"), 0.0);
    assert!(matches!(
        left.states.as_slice(),
        [DeviceState::SelectorSwitch {
            last_transition_time_seconds: Some(0.005),
            ..
        }]
    ));
    let mut invalid = command(&id, OperatorPosition::Right);
    invalid.expected_document_revision = "stale".into();
    assert!(matches!(
        engine.set_operator_state(invalid),
        SetOperatorStateResult::Failure { .. }
    ));
    let continued = engine
        .advance(AdvanceRequest {
            run_id: id.clone(),
            step_count: 1,
        })
        .unwrap();
    assert_eq!(continued.values, left.values);
    let right = operate(&mut engine, &id, &mut revision, OperatorPosition::Right);
    assert_eq!(current(&layout, &right, "throw1CurrentAmps"), 0.0);
    assert!((current(&layout, &right, "throw2CurrentAmps") - 10.0 / 2000.11).abs() < 1e-9);
    let center = operate(&mut engine, &id, &mut revision, OperatorPosition::Center);
    assert_eq!(center.values, init.values);
}

#[test]
fn emergency_stop_latches_until_reset_and_other_controls_follow_their_positions() {
    let mut engine = Engine::default();
    let mut revision = 0;
    let (id, layout, armed) = compile(&mut engine, request("emergency_stop", None));
    assert!((current(&layout, &armed, "currentAmps") - 10.0 / 1000.11).abs() < 1e-9);
    let latched = operate(&mut engine, &id, &mut revision, OperatorPosition::Pressed);
    assert_eq!(current(&layout, &latched, "currentAmps"), 0.0);
    let released = operate(&mut engine, &id, &mut revision, OperatorPosition::Released);
    assert_eq!(released.states, latched.states);
    assert_eq!(released.values, latched.values);
    let reset = operate(&mut engine, &id, &mut revision, OperatorPosition::Reset);
    assert_eq!(reset.values, armed.values);

    revision = 0;
    let (id, layout, init) = compile(&mut engine, request("limit_switch", None));
    assert_eq!(current(&layout, &init, "currentAmps"), 0.0);
    let pressed = operate(&mut engine, &id, &mut revision, OperatorPosition::Pressed);
    assert!(current(&layout, &pressed, "currentAmps") > 0.0);
    assert_eq!(
        operate(&mut engine, &id, &mut revision, OperatorPosition::Released).values,
        init.values
    );
    assert!(matches!(
        engine.set_operator_state(command(&id, OperatorPosition::Reset)),
        SetOperatorStateResult::Failure { .. }
    ));

    revision = 0;
    let input = request("key_switch", Some(1));
    let (id, layout, init) = compile(&mut engine, input.clone());
    assert!(current(&layout, &init, "currentAmps") > 0.0);
    let off = operate(&mut engine, &id, &mut revision, OperatorPosition::Open);
    assert_eq!(current(&layout, &off, "currentAmps"), 0.0);
    assert_eq!(compile(&mut engine, input).2.values, init.values);
}

#[test]
fn selector_and_zero_delay_relay_settle_before_the_current_time_reply() {
    let mut input = serde_json::to_value(request("selector_switch", None)).unwrap();
    input["schematic"]["components"][2]["typeId"] = json!("relay_on_delay");
    input["schematic"]["components"][2]["parameters"] = json!({
        "coilResistanceOhms": 240, "pickupCurrentAmps": 0.025,
        "dropoutCurrentAmps": 0.02, "closedResistanceOhms": 0.01, "delaySeconds": 0
    });
    input["geometry"]["components"][2]["pins"] = json!([
        pin("a1", 4, -1),
        pin("a2", 0, 4),
        pin("no1", 8, 0),
        pin("no2", 10, 0),
        pin("nc1", 8, 2),
        pin("nc2", 10, 2)
    ]);
    let mut engine = Engine::default();
    let (id, _, _) = compile(&mut engine, serde_json::from_value(input).unwrap());
    engine
        .advance(AdvanceRequest {
            run_id: id.clone(),
            step_count: 5,
        })
        .unwrap();
    let mut revision = 0;
    let snapshot = operate(&mut engine, &id, &mut revision, OperatorPosition::Left);
    assert_eq!(snapshot.metadata.simulation_step, 5);
    assert!(snapshot.states.iter().any(|state| matches!(
        state,
        DeviceState::RelayOnDelay {
            output_on: true,
            timing: false,
            last_transition_time_seconds: Some(0.005),
            ..
        }
    )));
    let snapshot = operate(&mut engine, &id, &mut revision, OperatorPosition::Center);
    assert!(snapshot.states.iter().any(|state| matches!(
        state,
        DeviceState::RelayOnDelay {
            output_on: false,
            timing: false,
            ..
        }
    )));
}
