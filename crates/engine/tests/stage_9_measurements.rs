use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentTypeId, Engine, ParameterId,
    ResultLayout, ResultSignConvention, ResultSnapshot, SetParameterRequest, SetParameterResult,
    StableId, Unit, component_catalog,
};
use serde_json::{Value, from_value, json};

const SOURCE_ID: &str = "00000000-0000-4000-8000-000000000001";
const VOLTMETER_ID: &str = "00000000-0000-4000-8000-000000000002";
const AMMETER_ID: &str = "00000000-0000-4000-8000-000000000003";
const WATTMETER_ID: &str = "00000000-0000-4000-8000-000000000004";
const LOAD_ID: &str = "00000000-0000-4000-8000-000000000005";
const INDUCTOR_ID: &str = "00000000-0000-4000-8000-000000000006";
const REFERENCE_ID: &str = "00000000-0000-4000-8000-000000000007";
const THIRD_LOAD_ID: &str = "00000000-0000-4000-8000-000000000008";

fn measurement_request() -> CompileRequest {
    CompileRequest {
        schematic: from_value(json!({
            "schemaVersion": 3, "documentation": {"designators":[],"wireNumbers":[],"terminals":[],"notes":[],"titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},"sheets":[]},
            "components": [
                {
                    "id": SOURCE_ID, "typeId": "ac_voltage_source",
                    "position": {"x": 0, "y": 0}, "rotationDeg": 0,
                    "parameters": {
                        "voltageRmsVolts": 10.0, "frequencyHz": 50.0,
                        "phaseOffsetDegrees": 0.0, "internalResistanceOhms": 0.1
                    }
                },
                {
                    "id": VOLTMETER_ID, "typeId": "ac_voltmeter",
                    "position": {"x": 1, "y": 0}, "rotationDeg": 0,
                    "parameters": {"inputResistanceOhms": 10000000.0, "frequencyHz": 50.0}
                },
                {
                    "id": AMMETER_ID, "typeId": "ac_ammeter",
                    "position": {"x": 2, "y": 0}, "rotationDeg": 0,
                    "parameters": {"burdenResistanceOhms": 0.1, "frequencyHz": 50.0}
                },
                {
                    "id": WATTMETER_ID, "typeId": "wattmeter",
                    "position": {"x": 3, "y": 0}, "rotationDeg": 0,
                    "parameters": {
                        "inputResistanceOhms": 10000000.0,
                        "burdenResistanceOhms": 0.1, "frequencyHz": 50.0
                    }
                },
                {
                    "id": LOAD_ID, "typeId": "resistor",
                    "position": {"x": 4, "y": 0}, "rotationDeg": 0,
                    "parameters": {"resistanceOhms": 100.0}
                },
                {
                    "id": INDUCTOR_ID, "typeId": "inductor",
                    "position": {"x": 5, "y": 0}, "rotationDeg": 0,
                    "parameters": {"inductanceHenries": 0.1}
                }
            ],
            "wires": [], "junctions": [],
            "referenceMarkers": [{"id": REFERENCE_ID, "position": {"x": 0, "y": 0}}],
            "rails": []
        }))
        .unwrap(),
        geometry: from_value(json!({
            "components": [
                {"componentId": SOURCE_ID, "pins": [
                    {"pinId": "pin1", "position": {"x": 0, "y": 10}},
                    {"pinId": "pin2", "position": {"x": 0, "y": 0}}
                ]},
                {"componentId": VOLTMETER_ID, "pins": [
                    {"pinId": "pin1", "position": {"x": 10, "y": 10}},
                    {"pinId": "pin2", "position": {"x": 0, "y": 0}}
                ]},
                {"componentId": AMMETER_ID, "pins": [
                    {"pinId": "pin1", "position": {"x": 0, "y": 10}},
                    {"pinId": "pin2", "position": {"x": 5, "y": 10}}
                ]},
                {"componentId": WATTMETER_ID, "pins": [
                    {"pinId": "v1", "position": {"x": 10, "y": 10}},
                    {"pinId": "v2", "position": {"x": 0, "y": 0}},
                    {"pinId": "i1", "position": {"x": 5, "y": 10}},
                    {"pinId": "i2", "position": {"x": 10, "y": 10}}
                ]},
                {"componentId": LOAD_ID, "pins": [
                    {"pinId": "pin1", "position": {"x": 10, "y": 10}},
                    {"pinId": "pin2", "position": {"x": 10, "y": 0}}
                ]},
                {"componentId": INDUCTOR_ID, "pins": [
                    {"pinId": "pin1", "position": {"x": 10, "y": 0}},
                    {"pinId": "pin2", "position": {"x": 0, "y": 0}}
                ]}
            ]
        }))
        .unwrap(),
        document_revision: "measurements-1".to_owned(),
        faults: None,
    }
}

fn wattmeter_request(reversed_current: bool) -> CompileRequest {
    let mut request = measurement_request();
    request.schematic.components.retain(|component| {
        matches!(
            component.type_id(),
            open_circuit_engine::ComponentTypeId::AcVoltageSource
                | open_circuit_engine::ComponentTypeId::Wattmeter
                | open_circuit_engine::ComponentTypeId::Resistor
                | open_circuit_engine::ComponentTypeId::Inductor
        )
    });
    request.geometry.components.retain(|component| {
        [SOURCE_ID, WATTMETER_ID, LOAD_ID, INDUCTOR_ID].contains(&component.component_id.as_str())
    });
    let wattmeter = request
        .geometry
        .components
        .iter_mut()
        .find(|component| component.component_id.as_str() == WATTMETER_ID)
        .unwrap();
    wattmeter
        .pins
        .iter_mut()
        .for_each(|pin| match pin.pin_id.as_str() {
            "i1" => {
                pin.position = if reversed_current {
                    open_circuit_engine::GridPoint { x: 10, y: 10 }
                } else {
                    open_circuit_engine::GridPoint { x: 0, y: 10 }
                }
            }
            "i2" => {
                pin.position = if reversed_current {
                    open_circuit_engine::GridPoint { x: 0, y: 10 }
                } else {
                    open_circuit_engine::GridPoint { x: 10, y: 10 }
                }
            }
            _ => {}
        });
    request
}

fn three_phase_meter_request(
    load_resistances_ohms: [f64; 3],
    neutral_connected: bool,
) -> CompileRequest {
    let source_pins = [
        ("l1", open_circuit_engine::GridPoint { x: 0, y: 2 }),
        ("l2", open_circuit_engine::GridPoint { x: 0, y: 4 }),
        ("l3", open_circuit_engine::GridPoint { x: 0, y: 6 }),
        ("n", open_circuit_engine::GridPoint { x: 4, y: 4 }),
    ];
    let load_neutral = if neutral_connected {
        open_circuit_engine::GridPoint { x: 4, y: 4 }
    } else {
        open_circuit_engine::GridPoint { x: 8, y: 4 }
    };
    let meter_node = open_circuit_engine::GridPoint { x: 2, y: 2 };
    let load_ids = [LOAD_ID, INDUCTOR_ID, THIRD_LOAD_ID];
    let mut components = vec![
        json!({
            "id": SOURCE_ID,
            "typeId": "three_phase_source_star",
            "position": {"x": 0, "y": 0},
            "rotationDeg": 0,
            "parameters": {
                "phaseVoltageRmsVolts": 230.0,
            "frequencyHz": 60.0,
                "phaseOffsetDegrees": 0.0,
                "internalResistanceOhms": 0.1,
                "reversedSequence": 0.0
            }
        }),
        json!({
            "id": VOLTMETER_ID,
            "typeId": "ac_voltmeter",
            "position": {"x": 1, "y": 0},
            "rotationDeg": 0,
        "parameters": {"inputResistanceOhms": 10000000.0, "frequencyHz": 60.0}
        }),
        json!({
            "id": AMMETER_ID,
            "typeId": "ac_ammeter",
            "position": {"x": 2, "y": 0},
            "rotationDeg": 0,
        "parameters": {"burdenResistanceOhms": 0.1, "frequencyHz": 60.0}
        }),
    ];
    let mut geometry = vec![
        json!({
            "componentId": SOURCE_ID,
            "pins": source_pins.iter().map(|(pin_id, position)| json!({
                "pinId": pin_id,
                "position": {"x": position.x, "y": position.y}
            })).collect::<Vec<_>>()
        }),
        json!({
            "componentId": VOLTMETER_ID,
            "pins": [
                {"pinId": "pin1", "position": {"x": meter_node.x, "y": meter_node.y}},
                {"pinId": "pin2", "position": {"x": load_neutral.x, "y": load_neutral.y}}
            ]
        }),
        json!({
            "componentId": AMMETER_ID,
            "pins": [
                {"pinId": "pin1", "position": {"x": 0, "y": 2}},
                {"pinId": "pin2", "position": {"x": meter_node.x, "y": meter_node.y}}
            ]
        }),
    ];
    for index in 0..3 {
        components.push(json!({
            "id": load_ids[index],
            "typeId": "resistor",
            "position": {"x": 4, "y": 4},
            "rotationDeg": 0,
            "parameters": {"resistanceOhms": load_resistances_ohms[index]}
        }));
        let phase = source_pins[index].1;
        geometry.push(json!({
            "componentId": load_ids[index],
            "pins": [
                {"pinId": "pin1", "position": if index == 0 {
                    json!({"x": meter_node.x, "y": meter_node.y})
                } else {
                    json!({"x": phase.x, "y": phase.y})
                }},
                {"pinId": "pin2", "position": {"x": load_neutral.x, "y": load_neutral.y}}
            ]
        }));
    }

    CompileRequest {
        schematic: from_value(json!({
            "schemaVersion": 3, "documentation": {"designators":[],"wireNumbers":[],"terminals":[],"notes":[],"titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},"sheets":[]},
            "components": components,
            "wires": [],
            "junctions": [],
            "referenceMarkers": [
                {"id": REFERENCE_ID, "position": {"x": 4, "y": 4}}
            ],
            "rails": []
        }))
        .unwrap(),
        geometry: from_value(json!({ "components": geometry })).unwrap(),
        document_revision: if neutral_connected {
            "balanced-star-meter-reference"
        } else {
            "unbalanced-floating-star-meter-reference"
        }
        .to_owned(),
        faults: None,
    }
}

fn compile(request: CompileRequest) -> (Engine, StableId, ResultLayout, ResultSnapshot) {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        diagnostics,
        ..
    } = engine.compile(request)
    else {
        panic!("measurement fixture should compile");
    };
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    (engine, run_id, layout, initial_snapshot)
}

fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, key: &str) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("missing result slot {key}"));
    snapshot.values[slot.offset as usize]
}

fn golden_wattmeter_summary() -> Value {
    let text = include_str!("../../../tests/golden/stage-9-analytical.expected.json");
    let document: Value = serde_json::from_str(text).unwrap();
    document["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "wattmeter-rl")
        .unwrap()["summary"]
        .clone()
}

fn close_relative(actual: f64, expected: f64, relative_tolerance: f64) {
    assert!(actual.is_finite());
    assert!(
        (actual - expected).abs() <= expected.abs() * relative_tolerance,
        "{actual} differs from {expected} by more than {relative_tolerance} relative"
    );
}

fn close_scaled(actual: f64, expected: f64, signal_rms: f64, relative_tolerance: f64) {
    let tolerance = expected.abs().max(signal_rms.abs()) * relative_tolerance;
    assert!(
        actual.is_finite() && (actual - expected).abs() <= tolerance,
        "{actual} differs from {expected} by more than {tolerance}"
    );
}

fn expected_star_phase_one_meter_reading(
    load_resistances_ohms: [f64; 3],
    neutral_connected: bool,
) -> (f64, f64) {
    let meter_input_ohms: f64 = 10_000_000.0;
    let phase_degrees: [f64; 3] = [0.0, -120.0, 120.0];
    let mut source_phasors = [(0.0, 0.0); 3];
    let mut branch_load_resistances = [0.0; 3];
    let mut branch_total_resistances = [0.0; 3];
    let mut neutral_numerator = (0.0, 0.0);
    let mut neutral_denominator = 0.0;
    for index in 0..3 {
        let phase = phase_degrees[index].to_radians();
        source_phasors[index] = (230.0 * phase.cos(), 230.0 * phase.sin());
        branch_load_resistances[index] = if index == 0 {
            1.0 / (1.0 / load_resistances_ohms[index] + 1.0 / meter_input_ohms)
        } else {
            load_resistances_ohms[index]
        };
        let meter_burden_ohms = if index == 0 { 0.1 } else { 0.0 };
        branch_total_resistances[index] = 0.1 + meter_burden_ohms + branch_load_resistances[index];
        let conductance = 1.0 / branch_total_resistances[index];
        neutral_numerator.0 += source_phasors[index].0 * conductance;
        neutral_numerator.1 += source_phasors[index].1 * conductance;
        neutral_denominator += conductance;
    }
    let neutral = if neutral_connected {
        (0.0, 0.0)
    } else {
        (
            neutral_numerator.0 / neutral_denominator,
            neutral_numerator.1 / neutral_denominator,
        )
    };
    let phase_current = (
        (source_phasors[0].0 - neutral.0) / branch_total_resistances[0],
        (source_phasors[0].1 - neutral.1) / branch_total_resistances[0],
    );
    let phase_voltage = (
        phase_current.0 * branch_load_resistances[0],
        phase_current.1 * branch_load_resistances[0],
    );
    (
        phase_voltage.0.hypot(phase_voltage.1),
        phase_current.0.hypot(phase_current.1),
    )
}

#[test]
fn meter_catalog_and_result_slots_match_the_measurement_contract() {
    let catalog = component_catalog();
    for (type_id, pin_ids, parameter_ids) in [
        (
            ComponentTypeId::AcVoltmeter,
            vec!["pin1", "pin2"],
            vec![ParameterId::InputResistanceOhms, ParameterId::FrequencyHz],
        ),
        (
            ComponentTypeId::AcAmmeter,
            vec!["pin1", "pin2"],
            vec![ParameterId::BurdenResistanceOhms, ParameterId::FrequencyHz],
        ),
        (
            ComponentTypeId::Wattmeter,
            vec!["v1", "v2", "i1", "i2"],
            vec![
                ParameterId::InputResistanceOhms,
                ParameterId::BurdenResistanceOhms,
                ParameterId::FrequencyHz,
            ],
        ),
    ] {
        let entry = catalog
            .component_types
            .iter()
            .find(|entry| entry.type_id == type_id)
            .unwrap();
        assert_eq!(
            entry
                .pins
                .iter()
                .map(|pin| pin.id.as_str())
                .collect::<Vec<_>>(),
            pin_ids
        );
        assert!(entry.fault_kinds.is_empty());
        assert_eq!(
            entry
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>(),
            parameter_ids
        );
        let frequency = entry
            .parameters
            .iter()
            .find(|parameter| parameter.id == ParameterId::FrequencyHz)
            .unwrap();
        assert_eq!(frequency.unit, Unit::Hertz);
        assert_eq!(frequency.default_value, 50.0);
        assert_eq!(frequency.minimum_value, Some(1.0));
        assert_eq!(frequency.maximum_value, Some(400.0));
    }

    let (_, _, layout, _) = compile(measurement_request());
    let expected = [
        (
            VOLTMETER_ID,
            vec![
                format!("{VOLTMETER_ID}:voltage"),
                format!("{VOLTMETER_ID}:current"),
                format!("{VOLTMETER_ID}/rms_voltage"),
                format!("{VOLTMETER_ID}/mean_voltage"),
                format!("{VOLTMETER_ID}/peak_voltage"),
                format!("{VOLTMETER_ID}/frequency"),
                format!("{VOLTMETER_ID}/frequency_valid"),
                format!("{VOLTMETER_ID}/valid"),
            ],
        ),
        (
            AMMETER_ID,
            vec![
                format!("{AMMETER_ID}:voltage"),
                format!("{AMMETER_ID}:current"),
                format!("{AMMETER_ID}/rms_current"),
                format!("{AMMETER_ID}/mean_current"),
                format!("{AMMETER_ID}/peak_current"),
                format!("{AMMETER_ID}/frequency"),
                format!("{AMMETER_ID}/frequency_valid"),
                format!("{AMMETER_ID}/valid"),
            ],
        ),
    ];
    for (component_id, keys) in expected {
        let slots = layout
            .slots
            .iter()
            .filter(|slot| slot.component_id.as_str() == component_id)
            .collect::<Vec<_>>();
        assert_eq!(
            slots
                .iter()
                .map(|slot| slot.key.as_str())
                .collect::<Vec<_>>(),
            keys.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }
    let power = layout
        .slots
        .iter()
        .find(|slot| slot.key == format!("{WATTMETER_ID}/real_power"))
        .unwrap();
    assert_eq!(power.unit, Unit::Watt);
    assert_eq!(power.sign_convention, ResultSignConvention::LoadConvention);
}

#[test]
fn ac_meter_layout_validity_and_frequency_follow_internal_samples() {
    let (mut engine, run_id, layout, initial) = compile(measurement_request());
    let expected_wattmeter_keys = [
        "voltage",
        "current",
        "rms_voltage",
        "rms_current",
        "real_power",
        "reactive_power",
        "apparent_power",
        "power_factor",
        "phase_angle",
        "valid",
    ]
    .map(|suffix| format!("{WATTMETER_ID}/{suffix}"));
    assert_eq!(
        layout
            .slots
            .iter()
            .filter(|slot| slot.component_id.as_str() == WATTMETER_ID)
            .map(|slot| slot.key.as_str())
            .collect::<Vec<_>>(),
        expected_wattmeter_keys
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        value(&layout, &initial, &format!("{VOLTMETER_ID}/valid")),
        0.0
    );
    assert_eq!(
        value(&layout, &initial, &format!("{AMMETER_ID}/valid")),
        0.0
    );
    assert_eq!(
        value(&layout, &initial, &format!("{WATTMETER_ID}/valid")),
        0.0
    );
    assert_eq!(
        value(&layout, &initial, &format!("{VOLTMETER_ID}/rms_voltage")),
        0.0
    );

    let first_window = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 40,
        })
        .unwrap();
    assert_eq!(
        value(&layout, &first_window, &format!("{VOLTMETER_ID}/valid")),
        1.0
    );
    assert_eq!(
        value(&layout, &first_window, &format!("{AMMETER_ID}/valid")),
        1.0
    );
    assert_eq!(
        value(&layout, &first_window, &format!("{WATTMETER_ID}/valid")),
        1.0
    );
    assert_eq!(
        value(
            &layout,
            &first_window,
            &format!("{VOLTMETER_ID}/frequency_valid")
        ),
        1.0
    );
    assert_eq!(
        value(
            &layout,
            &first_window,
            &format!("{AMMETER_ID}/frequency_valid")
        ),
        0.0
    );
    let snapshot = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 21,
        })
        .unwrap();
    let (mut control_engine, control_run, _, _) = compile(measurement_request());
    control_engine
        .advance(AdvanceRequest {
            run_id: control_run.clone(),
            step_count: 61,
        })
        .unwrap();
    assert_eq!(
        value(&layout, &snapshot, &format!("{VOLTMETER_ID}/valid")),
        1.0
    );
    assert_eq!(
        value(&layout, &snapshot, &format!("{AMMETER_ID}/valid")),
        1.0
    );
    assert_eq!(
        value(&layout, &snapshot, &format!("{WATTMETER_ID}/valid")),
        1.0
    );
    assert_eq!(
        value(
            &layout,
            &snapshot,
            &format!("{VOLTMETER_ID}/frequency_valid")
        ),
        1.0
    );
    assert_eq!(
        value(&layout, &snapshot, &format!("{AMMETER_ID}/frequency_valid")),
        1.0
    );
    assert!((value(&layout, &snapshot, &format!("{VOLTMETER_ID}/frequency")) - 50.0).abs() < 0.1);
    let ammeter_frequency = value(&layout, &snapshot, &format!("{AMMETER_ID}/frequency"));
    assert!(
        (ammeter_frequency - 50.0).abs() < 0.1,
        "measured ammeter frequency was {ammeter_frequency} Hz"
    );

    // A refused live update leaves the committed rolling windows unchanged.
    let refused = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "measurements-1".to_owned(),
        document_revision: "measurements-invalid".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 0.5,
    });
    assert!(matches!(refused, SetParameterResult::Failure { .. }));
    let after_refusal = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1,
        })
        .unwrap();
    let after_control = control_engine
        .advance(AdvanceRequest {
            run_id: control_run,
            step_count: 1,
        })
        .unwrap();
    assert_eq!(after_refusal.values, after_control.values);
    assert!(!snapshot.values.is_empty());
}

#[test]
fn ac_meter_rms_mean_and_peak_match_balanced_and_unbalanced_star_references() {
    for (load_resistances, neutral_connected) in [
        ([100.0, 100.0, 100.0], true),
        ([100.0, 220.0, 330.0], false),
    ] {
        let (mut engine, run_id, layout, _) = compile(three_phase_meter_request(
            load_resistances,
            neutral_connected,
        ));
        let snapshot = engine
            .advance(AdvanceRequest {
                run_id,
                step_count: 300,
            })
            .unwrap();
        let (expected_voltage_rms, expected_current_rms) =
            expected_star_phase_one_meter_reading(load_resistances, neutral_connected);
        let voltage_rms = value(&layout, &snapshot, &format!("{VOLTMETER_ID}/rms_voltage"));
        let current_rms = value(&layout, &snapshot, &format!("{AMMETER_ID}/rms_current"));
        close_scaled(
            voltage_rms,
            expected_voltage_rms,
            expected_voltage_rms,
            0.002,
        );
        close_scaled(
            current_rms,
            expected_current_rms,
            expected_current_rms,
            0.002,
        );
        close_scaled(
            value(&layout, &snapshot, &format!("{VOLTMETER_ID}/mean_voltage")),
            0.0,
            expected_voltage_rms,
            0.002,
        );
        close_scaled(
            value(&layout, &snapshot, &format!("{AMMETER_ID}/mean_current")),
            0.0,
            expected_current_rms,
            0.002,
        );
        close_scaled(
            value(&layout, &snapshot, &format!("{VOLTMETER_ID}/peak_voltage")),
            std::f64::consts::SQRT_2 * expected_voltage_rms,
            expected_voltage_rms,
            0.002,
        );
        close_scaled(
            value(&layout, &snapshot, &format!("{AMMETER_ID}/peak_current")),
            std::f64::consts::SQRT_2 * expected_current_rms,
            expected_current_rms,
            0.002,
        );
    }
}

#[test]
fn meter_and_source_frequency_changes_restart_validity_windows() {
    let (mut engine, run_id, layout, _) = compile(measurement_request());
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 40,
        })
        .unwrap();

    let SetParameterResult::Success { snapshot, .. } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "measurements-1".to_owned(),
        document_revision: "measurements-2".to_owned(),
        component_id: StableId::parse(VOLTMETER_ID).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 60.0,
    }) else {
        panic!("changing the voltmeter's measurement frequency should succeed");
    };
    assert_eq!(
        value(&layout, &snapshot, &format!("{VOLTMETER_ID}/valid")),
        0.0
    );
    assert_eq!(
        value(&layout, &snapshot, &format!("{AMMETER_ID}/valid")),
        1.0
    );
    assert_eq!(
        value(&layout, &snapshot, &format!("{WATTMETER_ID}/valid")),
        1.0
    );
    let after_meter_window = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 17,
        })
        .unwrap();
    assert_eq!(
        value(
            &layout,
            &after_meter_window,
            &format!("{VOLTMETER_ID}/valid")
        ),
        1.0
    );

    let SetParameterResult::Success { snapshot, .. } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "measurements-2".to_owned(),
        document_revision: "measurements-3".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 60.0,
    }) else {
        panic!("changing the AC source frequency should succeed");
    };
    for id in [VOLTMETER_ID, AMMETER_ID, WATTMETER_ID] {
        assert_eq!(value(&layout, &snapshot, &format!("{id}/valid")), 0.0);
    }
    let after_source_window = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 20,
        })
        .unwrap();
    for id in [VOLTMETER_ID, AMMETER_ID, WATTMETER_ID] {
        assert_eq!(
            value(&layout, &after_source_window, &format!("{id}/valid")),
            1.0
        );
    }
}

#[test]
fn wattmeter_matches_rl_reference_and_reverses_load_convention_signs() {
    let summary = golden_wattmeter_summary();
    let (mut engine, run_id, layout, _) = compile(wattmeter_request(false));
    let snapshot = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 300,
        })
        .unwrap();
    let tolerance = summary["engineTolerance"]["measuredRmsMeanPeakPowerRelative"]
        .as_f64()
        .unwrap();
    for (suffix, expected_key) in [
        ("rms_voltage", "rmsVoltageVolts"),
        ("rms_current", "rmsCurrentAmps"),
        ("real_power", "realPowerWatts"),
        ("reactive_power", "reactivePowerVars"),
        ("apparent_power", "apparentPowerVoltAmps"),
    ] {
        close_relative(
            value(&layout, &snapshot, &format!("{WATTMETER_ID}/{suffix}")),
            summary[expected_key].as_f64().unwrap(),
            tolerance,
        );
    }
    close_relative(
        value(&layout, &snapshot, &format!("{WATTMETER_ID}/power_factor")),
        summary["powerFactor"].as_f64().unwrap(),
        0.002,
    );
    assert!(
        (value(&layout, &snapshot, &format!("{WATTMETER_ID}/phase_angle"))
            - summary["phaseAngleDegrees"].as_f64().unwrap())
        .abs()
            < 0.3
    );

    let (mut reversed_engine, reversed_run, reversed_layout, _) = compile(wattmeter_request(true));
    let reversed = reversed_engine
        .advance(AdvanceRequest {
            run_id: reversed_run,
            step_count: 300,
        })
        .unwrap();
    for suffix in ["real_power", "reactive_power", "power_factor"] {
        assert!(
            value(
                &reversed_layout,
                &reversed,
                &format!("{WATTMETER_ID}/{suffix}")
            ) < 0.0
        );
    }
}

#[test]
fn resistive_fifty_hertz_measurements_match_the_loaded_divider() {
    let mut request = measurement_request();
    // Connect the load directly to the return and remove the series inductor.
    request
        .schematic
        .components
        .retain(|part| part.id().as_str() != INDUCTOR_ID);
    request
        .geometry
        .components
        .retain(|part| part.component_id.as_str() != INDUCTOR_ID);
    let load = request
        .geometry
        .components
        .iter_mut()
        .find(|part| part.component_id.as_str() == LOAD_ID)
        .unwrap();
    load.pins[1].position.x = 0;

    let (mut engine, run_id, layout, _) = compile(request);
    let snapshot = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 60,
        })
        .unwrap();
    let parallel_load = 1.0 / (1.0 / 100.0 + 2.0 / 10_000_000.0);
    let expected_current = 10.0 / (0.3 + parallel_load);
    let expected_voltage = expected_current * parallel_load;
    for (key, expected) in [
        (format!("{VOLTMETER_ID}/rms_voltage"), expected_voltage),
        (format!("{AMMETER_ID}/rms_current"), expected_current),
        (
            format!("{WATTMETER_ID}/real_power"),
            expected_voltage * expected_current,
        ),
        (
            format!("{WATTMETER_ID}/apparent_power"),
            expected_voltage * expected_current,
        ),
        (format!("{WATTMETER_ID}/power_factor"), 1.0),
    ] {
        close_relative(value(&layout, &snapshot, &key), expected, 0.002);
    }
}
