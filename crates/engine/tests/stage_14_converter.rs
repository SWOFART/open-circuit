//! The Stage 14 simplified frequency converter (docs/contracts/stage-14-more-parts.md, "Semiconductor
//! and controlled sources", issue #212): the enable path, the three outputs in phase order, the
//! reset of the phase, and a three-phase induction motor driven by it. Voltages and currents are
//! held to `1e-8 + 1e-3 * |expected|`; the enabled state and the step of its first change must
//! match exactly.

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, Engine, ParameterId, ResultLayout,
    ResultSnapshot, SetParameterRequest, SetParameterResult, StableId,
};
use serde_json::{Value, json};

const SUPPLY: &str = "00000000-0000-4000-8000-000000001420";
const CONVERTER: &str = "00000000-0000-4000-8000-000000001421";
const MOTOR: &str = "00000000-0000-4000-8000-000000001422";
const LOAD: [&str; 3] = [
    "00000000-0000-4000-8000-000000001430",
    "00000000-0000-4000-8000-000000001431",
    "00000000-0000-4000-8000-000000001432",
];
const MARKER: &str = "00000000-0000-4000-8000-0000000014f0";

fn component(id: &str, type_id: &str, parameters: Value) -> Value {
    json!({"id": id, "position": {"x": 0, "y": 0}, "rotationDeg": 0,
        "typeId": type_id, "parameters": parameters})
}

fn pins(id: &str, pins: &[(&str, (i32, i32))]) -> Value {
    json!({"componentId": id, "pins": pins.iter().map(|(pin, (x, y))| json!({
        "pinId": pin, "position": {"x": x, "y": y}})).collect::<Vec<_>>()})
}

fn request(components: Vec<Value>, geometry: Vec<Value>) -> CompileRequest {
    let mut schematic: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.schematic.json"
    ))
    .unwrap();
    schematic["components"] = Value::Array(components);
    schematic["wires"] = json!([]);
    schematic["referenceMarkers"] = json!([{"id": MARKER, "position": {"x": 8, "y": 6}}]);
    serde_json::from_value(json!({
        "schematic": schematic,
        "geometry": {"components": geometry},
        "documentRevision": "rev-1"
    }))
    .unwrap()
}

fn converter_parameters(enable: f64, output_rms: f64) -> Value {
    json!({"inputResistanceOhms": 10000.0, "enableVolts": enable,
        "outputRmsVolts": output_rms, "frequencyHz": 50.0, "outputResistanceOhms": 1.0})
}

/// A DC supply on the input, the converter, and a resistor of 100 Ohm from each output to the
/// neutral (a balanced star load).
fn with_loads(supply_volts: f64) -> CompileRequest {
    let mut components = vec![
        component(
            SUPPLY,
            "dc_voltage_source",
            json!({"voltageVolts": supply_volts, "internalResistanceOhms": 0.0}),
        ),
        component(
            CONVERTER,
            "frequency_converter",
            converter_parameters(24.0, 48.0),
        ),
    ];
    let mut geometry = vec![
        pins(SUPPLY, &[("pin1", (0, 0)), ("pin2", (0, 4))]),
        pins(
            CONVERTER,
            &[
                ("inP", (0, 0)),
                ("inN", (0, 4)),
                ("u", (10, 0)),
                ("v", (10, 2)),
                ("w", (10, 4)),
                ("neutral", (8, 6)),
            ],
        ),
    ];
    for (index, id) in LOAD.iter().enumerate() {
        components.push(component(id, "resistor", json!({"resistanceOhms": 100.0})));
        geometry.push(pins(
            id,
            &[("pin1", (10, 2 * index as i32)), ("pin2", (8, 6))],
        ));
    }
    request(components, geometry)
}

fn compile(request: CompileRequest) -> (Engine, StableId, ResultLayout, ResultSnapshot) {
    let mut engine = Engine::default();
    let result = engine.compile(request);
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = result
    else {
        panic!("the circuit should compile: {result:?}");
    };
    (engine, run_id, layout, initial_snapshot)
}

fn advance(engine: &mut Engine, run_id: &StableId, steps: u32) -> ResultSnapshot {
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: steps,
        })
        .expect("the run advances")
}

fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, id: &str, suffix: &str) -> f64 {
    let key = format!("{id}/{suffix}");
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot {key}"));
    snapshot.values[slot.offset as usize]
}

fn close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-3 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

fn enabled(snapshot: &ResultSnapshot) -> bool {
    match snapshot
        .states
        .iter()
        .find(|state| state.component_id().as_str() == CONVERTER)
    {
        Some(DeviceState::FrequencyConverter { enabled, .. }) => *enabled,
        other => panic!("no converter state: {other:?}"),
    }
}

/// The open-circuit voltage of output `phase` (0 = u, 1 = v, 2 = w) at `step` public steps.
fn source_voltage(phase: usize, step: u32) -> f64 {
    let offset = [0.0, -120.0_f64, 120.0][phase].to_radians();
    let angle = 2.0 * std::f64::consts::PI * 50.0 * f64::from(step) * 1e-3 + offset;
    2.0_f64.sqrt() * 48.0 * angle.sin()
}

#[test]
fn the_dc_input_is_a_resistor_and_the_converter_is_enabled_at_its_enable_voltage() {
    // Exactly the enable voltage enables; just below does not.
    for (volts, expected) in [(24.0, true), (23.9, false), (48.0, true), (0.0, false)] {
        let (_, _, layout, snapshot) = compile(with_loads(volts));
        assert_eq!(enabled(&snapshot), expected, "{volts} V");
        close(
            value(&layout, &snapshot, CONVERTER, "inputVoltageVolts"),
            volts,
            "input voltage",
        );
        close(
            value(&layout, &snapshot, CONVERTER, "inputCurrentAmps"),
            volts / 10_000.0,
            "input current",
        );
    }
}

#[test]
fn the_outputs_follow_the_phase_order_at_zero_minus_120_and_plus_120_degrees() {
    let (mut engine, run_id, layout, initial) = compile(with_loads(24.0));
    // At step 0 the outputs are 0, -sqrt(6)*24 ... and +; across a 100 Ohm load each output
    // drops by the 1 Ohm series resistance: V = e * 100 / 101.
    for (phase, name) in ["u", "v", "w"].into_iter().enumerate() {
        close(
            value(&layout, &initial, CONVERTER, &format!("{name}VoltageVolts")),
            source_voltage(phase, 0) * 100.0 / 101.0,
            &format!("{name} at step 0"),
        );
    }
    for step in 1..=40_u32 {
        let snapshot = advance(&mut engine, &run_id, 1);
        for (phase, name) in ["u", "v", "w"].into_iter().enumerate() {
            let expected_voltage = source_voltage(phase, step) * 100.0 / 101.0;
            close(
                value(
                    &layout,
                    &snapshot,
                    CONVERTER,
                    &format!("{name}VoltageVolts"),
                ),
                expected_voltage,
                &format!("{name} voltage at step {step}"),
            );
            // A source that delivers power has a negative current (positive into the pin).
            close(
                value(&layout, &snapshot, CONVERTER, &format!("{name}CurrentAmps")),
                -expected_voltage / 100.0,
                &format!("{name} current at step {step}"),
            );
        }
        // A balanced load leaves no neutral current: the three phase currents sum to zero.
        let sum: f64 = ["u", "v", "w"]
            .into_iter()
            .map(|name| value(&layout, &snapshot, CONVERTER, &format!("{name}CurrentAmps")))
            .sum();
        assert!(sum.abs() < 1e-8, "step {step}: phase currents sum to {sum}");
    }
}

#[test]
fn a_disabled_converter_has_open_outputs_and_reports_no_output_current() {
    let (mut engine, run_id, layout, initial) = compile(with_loads(10.0));
    assert!(!enabled(&initial));
    for name in ["u", "v", "w"] {
        close(
            value(&layout, &initial, CONVERTER, &format!("{name}CurrentAmps")),
            0.0,
            "open output current",
        );
        close(
            value(&layout, &initial, CONVERTER, &format!("{name}VoltageVolts")),
            0.0,
            "open output voltage",
        );
    }
    let later = advance(&mut engine, &run_id, 25);
    assert!(!enabled(&later));
    assert_eq!(value(&layout, &later, CONVERTER, "uCurrentAmps"), 0.0);
}

fn set_supply(engine: &mut Engine, run_id: &StableId, volts: f64, from: &str, to: &str) {
    let SetParameterResult::Success { .. } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: from.into(),
        document_revision: to.into(),
        component_id: StableId::parse(SUPPLY).unwrap(),
        parameter_id: ParameterId::VoltageVolts,
        value: volts,
    }) else {
        panic!("the supply voltage is a live parameter");
    };
}

#[test]
fn the_phase_is_absolute_so_a_disable_and_enable_resume_where_the_time_is() {
    let (mut engine, run_id, layout, _) = compile(with_loads(24.0));
    advance(&mut engine, &run_id, 7);
    set_supply(&mut engine, &run_id, 0.0, "rev-1", "rev-2");
    let off = advance(&mut engine, &run_id, 6);
    assert!(!enabled(&off));
    set_supply(&mut engine, &run_id, 24.0, "rev-2", "rev-3");
    let back = advance(&mut engine, &run_id, 5);
    assert!(enabled(&back));
    // 7 + 6 + 5 public steps have passed: the outputs read the source at step 18, not at step 5.
    close(
        value(&layout, &back, CONVERTER, "uVoltageVolts"),
        source_voltage(0, 18) * 100.0 / 101.0,
        "u after re-enabling",
    );
    close(
        value(&layout, &back, CONVERTER, "vVoltageVolts"),
        source_voltage(1, 18) * 100.0 / 101.0,
        "v after re-enabling",
    );
}

#[test]
fn a_recompile_restarts_the_phase_at_step_zero() {
    let (mut engine, run_id, _, _) = compile(with_loads(24.0));
    advance(&mut engine, &run_id, 13);
    let CompileResult::Success {
        layout,
        initial_snapshot,
        ..
    } = engine.compile(with_loads(24.0))
    else {
        panic!("compiles");
    };
    close(
        value(&layout, &initial_snapshot, CONVERTER, "vVoltageVolts"),
        source_voltage(1, 0) * 100.0 / 101.0,
        "v at step zero of the new run",
    );
}

#[test]
fn an_enable_change_is_one_event_at_the_step_of_the_edit() {
    let (mut engine, run_id, _, initial) = compile(with_loads(0.0));
    assert!(!enabled(&initial));
    advance(&mut engine, &run_id, 3);
    set_supply(&mut engine, &run_id, 24.0, "rev-1", "rev-2");
    let after = advance(&mut engine, &run_id, 1);
    assert!(enabled(&after));
}

#[test]
fn inputs_are_compile_only_and_out_of_range_values_are_refused() {
    let (mut engine, run_id, _, _) = compile(with_loads(24.0));
    let refused = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "rev-1".into(),
        document_revision: "rev-2".into(),
        component_id: StableId::parse(CONVERTER).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 60.0,
    });
    assert!(matches!(refused, SetParameterResult::Failure { .. }));

    for parameters in [
        json!({"inputResistanceOhms": 0.5, "enableVolts": 24.0, "outputRmsVolts": 24.0,
            "frequencyHz": 50.0, "outputResistanceOhms": 1.0}),
        json!({"inputResistanceOhms": 1e4, "enableVolts": 0.0, "outputRmsVolts": 24.0,
            "frequencyHz": 50.0, "outputResistanceOhms": 1.0}),
        json!({"inputResistanceOhms": 1e4, "enableVolts": 24.0, "outputRmsVolts": 1001.0,
            "frequencyHz": 50.0, "outputResistanceOhms": 1.0}),
        json!({"inputResistanceOhms": 1e4, "enableVolts": 24.0, "outputRmsVolts": 24.0,
            "frequencyHz": 0.5, "outputResistanceOhms": 1.0}),
        json!({"inputResistanceOhms": 1e4, "enableVolts": 24.0, "outputRmsVolts": 24.0,
            "frequencyHz": 101.0, "outputResistanceOhms": 1.0}),
        json!({"inputResistanceOhms": 1e4, "enableVolts": 24.0, "outputRmsVolts": 24.0,
            "frequencyHz": 50.0, "outputResistanceOhms": 0.0}),
    ] {
        let result = Engine::default().compile(request(
            vec![component(CONVERTER, "frequency_converter", parameters)],
            vec![pins(
                CONVERTER,
                &[
                    ("inP", (0, 0)),
                    ("inN", (0, 4)),
                    ("u", (10, 0)),
                    ("v", (10, 2)),
                    ("w", (10, 4)),
                    ("neutral", (8, 6)),
                ],
            )],
        ));
        assert!(matches!(result, CompileResult::Failure { .. }));
    }
}

fn motor_parameters() -> Value {
    json!({"statorResistanceOhms": 1.0, "rotorResistanceOhms": 1.0,
        "statorLeakageHenries": 0.005, "rotorLeakageHenries": 0.005,
        "magnetizingHenries": 0.1, "polePairs": 2.0,
        "rotorInertiaKilogramMetersSquared": 0.002,
        "viscousFrictionNewtonMeterSecondsPerRadian": 0.01, "loadTorqueNewtonMeters": 0.1})
}

/// A star-connected motor on either the converter or a star source with the same numbers.
fn motor_circuit(converter: bool) -> CompileRequest {
    let motor_pins = [
        ("u1", (10, 0)),
        ("v1", (10, 2)),
        ("w1", (10, 4)),
        ("u2", (8, 6)),
        ("v2", (8, 6)),
        ("w2", (8, 6)),
    ];
    let supply_geometry = if converter {
        pins(
            CONVERTER,
            &[
                ("inP", (0, 0)),
                ("inN", (0, 4)),
                ("u", (10, 0)),
                ("v", (10, 2)),
                ("w", (10, 4)),
                ("neutral", (8, 6)),
            ],
        )
    } else {
        pins(
            CONVERTER,
            &[
                ("l1", (10, 0)),
                ("l2", (10, 2)),
                ("l3", (10, 4)),
                ("n", (8, 6)),
            ],
        )
    };
    let mut components = vec![
        if converter {
            component(
                CONVERTER,
                "frequency_converter",
                converter_parameters(24.0, 48.0),
            )
        } else {
            component(
                CONVERTER,
                "three_phase_source_star",
                json!({"phaseVoltageRmsVolts": 48.0, "frequencyHz": 50.0,
                    "phaseOffsetDegrees": 0.0, "internalResistanceOhms": 1.0,
                    "reversedSequence": 0.0}),
            )
        },
        component(MOTOR, "induction_motor_three_phase", motor_parameters()),
    ];
    let mut geometry = vec![supply_geometry, pins(MOTOR, &motor_pins)];
    if converter {
        components.push(component(
            SUPPLY,
            "dc_voltage_source",
            json!({"voltageVolts": 24.0, "internalResistanceOhms": 0.0}),
        ));
        geometry.push(pins(SUPPLY, &[("pin1", (0, 0)), ("pin2", (0, 4))]));
    }
    request(components, geometry)
}

#[test]
fn the_converter_drives_an_induction_motor_like_a_star_source_with_the_same_numbers() {
    let (mut converter, converter_run, converter_layout, _) = compile(motor_circuit(true));
    let (mut source, source_run, source_layout, _) = compile(motor_circuit(false));

    for _ in 0..30 {
        let from_converter = advance(&mut converter, &converter_run, 10);
        let from_source = advance(&mut source, &source_run, 10);
        for suffix in ["angular_velocity", "electromagnetic_torque"] {
            close(
                value(&converter_layout, &from_converter, MOTOR, suffix),
                value(&source_layout, &from_source, MOTOR, suffix),
                suffix,
            );
        }
    }
    // After 0.3 s the motor is running forward, near the synchronous speed of 157 rad/s.
    let last = advance(&mut converter, &converter_run, 1);
    let speed = value(&converter_layout, &last, MOTOR, "angular_velocity");
    assert!(speed > 100.0 && speed < 157.1, "speed {speed} rad/s");
}
