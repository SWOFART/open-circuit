//! Stage 14 indicators and actuators (docs/contracts/stage-14-more-parts.md, "Indicators and
//! actuators", issue #210): a lamp, an indicator, a solenoid, a valve and a buzzer, each against
//! its closed form. Voltages and currents are held to `1e-8 + 1e-3 * |expected|`; discrete states
//! and the step of the first transition must match exactly.

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, Engine, OperatorPosition,
    ParameterId, ResultLayout, ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult,
    SetParameterRequest, SetParameterResult, StableId, SwitchPosition,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000001410";
const LOAD: &str = "00000000-0000-4000-8000-000000001411";

fn request(
    source_volts: f64,
    load_type: &str,
    parameters: Value,
    reversed: bool,
) -> CompileRequest {
    let mut schematic: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.schematic.json"
    ))
    .unwrap();
    schematic["components"] = json!([
        {"id": SOURCE, "position": {"x": 0, "y": -2}, "rotationDeg": 0,
         "typeId": "dc_voltage_source",
         "parameters": {"voltageVolts": source_volts, "internalResistanceOhms": 0.0}},
        {"id": LOAD, "position": {"x": 0, "y": 0}, "rotationDeg": 0,
         "typeId": load_type, "parameters": parameters}
    ]);
    schematic["wires"] = json!([]);
    schematic["referenceMarkers"] = json!([]);
    // A reversed load has its positive pin on the source's negative terminal.
    let (p, n) = if reversed { (3, -3) } else { (-3, 3) };
    serde_json::from_value(json!({
        "schematic": schematic,
        "geometry": {"components": [
            {"componentId": SOURCE, "pins": [
                {"pinId": "pin1", "position": {"x": -3, "y": -2}},
                {"pinId": "pin2", "position": {"x": 3, "y": -2}}]},
            {"componentId": LOAD, "pins": [
                {"pinId": "p", "position": {"x": p, "y": -2}},
                {"pinId": "n", "position": {"x": n, "y": -2}}]}
        ]},
        "documentRevision": "rev-1"
    }))
    .unwrap()
}

fn compile(request: CompileRequest) -> (Engine, StableId, ResultLayout, ResultSnapshot) {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = engine.compile(request)
    else {
        panic!("the circuit should compile");
    };
    (engine, run_id, layout, initial_snapshot)
}

fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, suffix: &str) -> f64 {
    let key = format!("{LOAD}/{suffix}");
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot {key}"));
    snapshot.values[slot.offset as usize]
}

fn advance(engine: &mut Engine, run_id: &StableId, steps: u32) -> ResultSnapshot {
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: steps,
        })
        .expect("the run advances")
}

fn close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-3 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

fn lamp(volts: f64, ohms: f64) -> ResultSnapshot {
    let (_, _, layout, snapshot) = compile(request(
        volts,
        "lamp",
        json!({"resistanceOhms": ohms}),
        false,
    ));
    let _ = layout;
    snapshot
}

fn lit(snapshot: &ResultSnapshot) -> bool {
    match snapshot
        .states
        .iter()
        .find(|state| state.component_id().as_str() == LOAD)
    {
        Some(DeviceState::Lamp { lit, .. } | DeviceState::Indicator { lit, .. }) => *lit,
        other => panic!("no lamp or indicator state: {other:?}"),
    }
}

#[test]
fn a_lamp_obeys_ohms_law_reports_its_power_and_is_lit_at_a_quarter_watt() {
    let (_, _, layout, snapshot) = compile(request(
        12.0,
        "lamp",
        json!({"resistanceOhms": 120.0}),
        false,
    ));
    close(value(&layout, &snapshot, "voltageVolts"), 12.0, "voltage");
    close(value(&layout, &snapshot, "currentAmps"), 0.1, "current");
    close(value(&layout, &snapshot, "powerWatts"), 1.2, "power");
    assert!(lit(&snapshot));

    // 5 V across 120 Ohm is 0.208 W: dark.
    assert!(!lit(&lamp(5.0, 120.0)));
    // Exactly a quarter watt (5 V across 100 Ohm) is lit; just below it is not.
    assert!(lit(&lamp(5.0, 100.0)));
    assert!(!lit(&lamp(4.999, 100.0)));
}

#[test]
fn an_indicator_follows_the_magnitude_of_its_voltage_in_either_polarity() {
    let parameters = json!({"resistanceOhms": 2400.0, "thresholdVolts": 12.0});
    for (volts, reversed, expected) in [
        (11.9, false, false),
        (12.0, false, true),
        (12.1, false, true),
        (11.9, true, false),
        (12.0, true, true),
        (24.0, true, true),
    ] {
        let (_, _, layout, snapshot) =
            compile(request(volts, "indicator", parameters.clone(), reversed));
        assert_eq!(lit(&snapshot), expected, "{volts} V, reversed {reversed}");
        let sign = if reversed { -1.0 } else { 1.0 };
        close(
            value(&layout, &snapshot, "voltageVolts"),
            sign * volts,
            "voltage",
        );
        close(
            value(&layout, &snapshot, "currentAmps"),
            sign * volts / 2400.0,
            "current",
        );
    }
}

#[test]
fn a_buzzer_is_active_at_its_threshold_and_not_below_it() {
    let parameters = json!({"resistanceOhms": 240.0, "thresholdVolts": 6.0});
    for (volts, expected) in [(5.9, false), (6.0, true), (12.0, true)] {
        let (_, _, layout, snapshot) = compile(request(volts, "buzzer", parameters.clone(), false));
        let state = snapshot
            .states
            .iter()
            .find(|state| state.component_id().as_str() == LOAD);
        assert!(
            matches!(state, Some(DeviceState::Buzzer { active, .. }) if *active == expected),
            "{volts} V: {state:?}"
        );
        close(
            value(&layout, &snapshot, "currentAmps"),
            volts / 240.0,
            "current",
        );
    }
}

fn actuator(volts: f64, type_id: &str) -> CompileRequest {
    request(
        volts,
        type_id,
        json!({"resistanceOhms": 24.0, "inductanceHenries": 0.05,
            "pickupCurrentAmps": 0.4, "dropoutCurrentAmps": 0.2}),
        false,
    )
}

fn energized(snapshot: &ResultSnapshot) -> bool {
    match snapshot
        .states
        .iter()
        .find(|state| state.component_id().as_str() == LOAD)
    {
        Some(DeviceState::Solenoid { energized, .. }) => *energized,
        Some(DeviceState::Valve { position, .. }) => *position == SwitchPosition::Open,
        other => panic!("no solenoid or valve state: {other:?}"),
    }
}

const SWITCH: &str = "00000000-0000-4000-8000-000000001412";

/// The source, an open switch and the load in series: closing the switch is the step input.
fn switched(volts: f64, load_type: &str) -> CompileRequest {
    let mut value = serde_json::to_value(actuator(volts, load_type)).unwrap();
    value["schematic"]["components"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({"id": SWITCH, "position": {"x": 0, "y": 2}, "rotationDeg": 0,
            "typeId": "switch_spst", "parameters": {"closedResistanceOhms": 0.01}}),
        );
    let geometry = value["geometry"]["components"].as_array_mut().unwrap();
    // The load's positive pin moves to the far side of the switch.
    geometry[1]["pins"][0]["position"] = json!({"x": -5, "y": -2});
    geometry.push(json!({"componentId": SWITCH, "pins": [
        {"pinId": "pin1", "position": {"x": -3, "y": -2}},
        {"pinId": "pin2", "position": {"x": -5, "y": -2}}]}));
    serde_json::from_value(value).unwrap()
}

fn operate(
    engine: &mut Engine,
    run_id: &StableId,
    position: OperatorPosition,
    from: &str,
    to: &str,
) -> ResultSnapshot {
    let SetOperatorStateResult::Success { snapshot, .. } =
        engine.set_operator_state(SetOperatorStateRequest {
            run_id: run_id.clone(),
            expected_document_revision: from.into(),
            document_revision: to.into(),
            component_id: StableId::parse(SWITCH).unwrap(),
            position,
        })
    else {
        panic!("the switch accepts {position:?}");
    };
    snapshot
}

#[test]
fn a_solenoid_is_energized_at_compile_when_its_operating_point_is_above_pickup() {
    // The coil is a short for the DC operating point: 24 V on 24 Ohm is 1 A, above pickup.
    let (_, _, layout, initial) = compile(actuator(24.0, "solenoid"));
    assert!(energized(&initial));
    close(
        value(&layout, &initial, "currentAmps"),
        1.0,
        "operating-point current",
    );
}

#[test]
fn a_solenoid_current_follows_the_rl_step_and_pulls_in_at_the_first_step_above_pickup() {
    // 24 V on 24 Ohm, the 10 mOhm of the closed switch and 50 mH, switched on after the first
    // public step: i(t) = 24 / R' (1 - exp(-t R' / L)) with R' = 24.01 Ohm, from the start of the
    // second step. It passes 0.4 A at about 1.06 ms, so the detector first sees pickup one step
    // after the closing step. The first internal step after the event is backward Euler (the
    // event rule of Stage 3), which leaves the transient within 2 % of the exponential; the
    // settled current is within the contract's `1e-8 + 1e-3 * |expected|`.
    let (mut engine, run_id, layout, initial) = compile(switched(24.0, "solenoid"));
    assert!(!energized(&initial), "a solenoid starts off");
    close(
        value(&layout, &initial, "currentAmps"),
        0.0,
        "current before the switch closes",
    );

    // A change at step 0 re-evaluates the operating point (the coil would be a DC short), so the
    // switch is closed after the first step, with the coil current still zero.
    advance(&mut engine, &run_id, 1);
    let closed = operate(
        &mut engine,
        &run_id,
        OperatorPosition::Closed,
        "rev-1",
        "rev-2",
    );
    assert!(!energized(&closed), "0.38 A is below pickup");
    let resistance = 24.01;
    let analytic = |milliseconds: f64| {
        24.0 / resistance * (1.0 - (-(milliseconds * 1e-3) * resistance / 0.05).exp())
    };
    for step in 1..=40_u32 {
        let snapshot = advance(&mut engine, &run_id, 1);
        // The event closed the switch at the start of the step that is now `step` steps old.
        let expected = analytic(f64::from(step) + 1.0);
        let actual = value(&layout, &snapshot, "currentAmps");
        let allowed = if step < 20 {
            0.02 * expected
        } else {
            1e-8 + 1e-3 * expected
        };
        assert!(
            (actual - expected).abs() <= allowed,
            "step {step}: {actual} A is not {expected} A"
        );
        assert!(energized(&snapshot), "step {step}: above pickup");
    }
}

#[test]
fn a_valve_is_closed_at_rest_and_open_while_energized() {
    let (mut engine, run_id, _, initial) = compile(switched(24.0, "valve"));
    assert!(matches!(
        initial
            .states
            .iter()
            .find(|state| state.component_id().as_str() == LOAD),
        Some(DeviceState::Valve {
            position: SwitchPosition::Closed,
            ..
        })
    ));
    operate(
        &mut engine,
        &run_id,
        OperatorPosition::Closed,
        "rev-1",
        "rev-2",
    );
    let snapshot = advance(&mut engine, &run_id, 5);
    assert!(matches!(
        snapshot
            .states
            .iter()
            .find(|state| state.component_id().as_str() == LOAD),
        Some(DeviceState::Valve {
            position: SwitchPosition::Open,
            ..
        })
    ));
}

#[test]
fn the_pull_in_has_hysteresis_in_both_directions() {
    // Steady currents of 0.3 A (7.2 V) lie between dropout (0.2 A) and pickup (0.4 A).
    let (mut engine, run_id, _, _) = compile(actuator(7.2, "solenoid"));
    assert!(
        !energized(&advance(&mut engine, &run_id, 100)),
        "0.3 A never reaches pickup"
    );

    let edit = |value: f64, from: &str, to: &str| SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: from.into(),
        document_revision: to.into(),
        component_id: StableId::parse(SOURCE).unwrap(),
        parameter_id: ParameterId::VoltageVolts,
        value,
    };
    let SetParameterResult::Success { .. } = engine.set_parameter(edit(24.0, "rev-1", "rev-2"))
    else {
        panic!("the source voltage is a live parameter");
    };
    assert!(
        energized(&advance(&mut engine, &run_id, 20)),
        "1 A pulls in"
    );

    let SetParameterResult::Success { .. } = engine.set_parameter(edit(7.2, "rev-2", "rev-3"))
    else {
        panic!("the source voltage is a live parameter");
    };
    assert!(
        energized(&advance(&mut engine, &run_id, 100)),
        "0.3 A is above dropout, so it stays pulled in"
    );

    let SetParameterResult::Success { .. } = engine.set_parameter(edit(3.6, "rev-3", "rev-4"))
    else {
        panic!("the source voltage is a live parameter");
    };
    assert!(
        !energized(&advance(&mut engine, &run_id, 100)),
        "0.15 A is below dropout, so it releases"
    );
}

#[test]
fn a_recompile_resets_every_state_to_off() {
    let mut engine = Engine::default();
    let CompileResult::Success { run_id, .. } = engine.compile(switched(24.0, "solenoid")) else {
        panic!("compiles");
    };
    operate(
        &mut engine,
        &run_id,
        OperatorPosition::Closed,
        "rev-1",
        "rev-2",
    );
    assert!(energized(&advance(&mut engine, &run_id, 10)));
    let CompileResult::Success {
        initial_snapshot, ..
    } = engine.compile(switched(24.0, "solenoid"))
    else {
        panic!("compiles");
    };
    assert!(!energized(&initial_snapshot));
}

#[test]
fn inputs_are_compile_only_and_out_of_range_values_are_refused() {
    let mut engine = Engine::default();
    let CompileResult::Success { run_id, .. } = engine.compile(actuator(24.0, "solenoid")) else {
        panic!("compiles");
    };
    let refused = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "rev-1".into(),
        document_revision: "rev-2".into(),
        component_id: StableId::parse(LOAD).unwrap(),
        parameter_id: ParameterId::PickupCurrentAmps,
        value: 0.5,
    });
    assert!(matches!(refused, SetParameterResult::Failure { .. }));

    // Out of range: a dropout current not below the pickup current, a zero resistance.
    for parameters in [
        json!({"resistanceOhms": 24.0, "inductanceHenries": 0.05,
            "pickupCurrentAmps": 0.4, "dropoutCurrentAmps": 0.4}),
        json!({"resistanceOhms": 0.0, "inductanceHenries": 0.05,
            "pickupCurrentAmps": 0.4, "dropoutCurrentAmps": 0.2}),
    ] {
        let result = Engine::default().compile(request(24.0, "valve", parameters, false));
        assert!(matches!(result, CompileResult::Failure { .. }));
    }
    let result = Engine::default().compile(request(
        24.0,
        "buzzer",
        json!({"resistanceOhms": 240.0, "thresholdVolts": 0.0}),
        false,
    ));
    assert!(matches!(result, CompileResult::Failure { .. }));
}

#[test]
fn the_catalog_lists_the_five_loads_with_the_contract_defaults() {
    let catalog = open_circuit_engine::component_catalog();
    let defaults = |type_id: &str| -> Vec<(String, f64)> {
        let entry = catalog
            .component_types
            .iter()
            .find(|entry| serde_json::to_value(entry.type_id).unwrap() == json!(type_id))
            .unwrap_or_else(|| panic!("no {type_id}"));
        assert_eq!(
            entry
                .pins
                .iter()
                .map(|pin| pin.id.as_str())
                .collect::<Vec<_>>(),
            ["p", "n"]
        );
        assert!(entry.fault_kinds.is_empty());
        entry
            .parameters
            .iter()
            .map(|parameter| {
                (
                    serde_json::to_value(parameter.id)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                    parameter.default_value,
                )
            })
            .collect()
    };
    let named = |pairs: &[(&str, f64)]| -> Vec<(String, f64)> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), *value))
            .collect()
    };
    assert_eq!(defaults("lamp"), named(&[("resistanceOhms", 120.0)]));
    assert_eq!(
        defaults("indicator"),
        named(&[("resistanceOhms", 2400.0), ("thresholdVolts", 12.0)])
    );
    assert_eq!(
        defaults("buzzer"),
        named(&[("resistanceOhms", 240.0), ("thresholdVolts", 6.0)])
    );
    let actuator = named(&[
        ("resistanceOhms", 24.0),
        ("inductanceHenries", 0.05),
        ("pickupCurrentAmps", 0.4),
        ("dropoutCurrentAmps", 0.2),
    ]);
    assert_eq!(defaults("solenoid"), actuator);
    assert_eq!(defaults("valve"), actuator);
}
