use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentSpec, DiagnosticCode, Engine,
    ParameterId, ResultLayout, ResultQuantity, ResultSnapshot, Schematic, SetParameterRequest,
    SetParameterResult,
};
use serde_json::{Value, from_str, from_value};

const CHARGE_SCHEMATIC: &str =
    include_str!("../../../tests/golden/stage-3-rc-charge.schematic.json");
const CHARGE_GEOMETRY: &str = include_str!("../../../tests/golden/stage-3-rc-charge.geometry.json");
const CHARGE_EXPECTED: &str = include_str!("../../../tests/golden/stage-3-rc-charge.expected.json");
const CHARGE_NGSPICE: &str = include_str!("../../../tests/golden/stage-3-ngspice-rc-charge.csv");
const DISCHARGE_SCHEMATIC: &str =
    include_str!("../../../tests/golden/stage-3-rc-discharge.schematic.json");
const DISCHARGE_GEOMETRY: &str =
    include_str!("../../../tests/golden/stage-3-rc-discharge.geometry.json");
const DISCHARGE_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-3-rc-discharge.expected.json");
const DISCHARGE_NGSPICE: &str =
    include_str!("../../../tests/golden/stage-3-ngspice-rc-discharge.csv");

const CAPACITOR_ID: &str = "00000000-0000-4000-8000-000000000003";
const DISCHARGE_CAPACITOR_ID: &str = "00000000-0000-4000-8000-000000000002";
const UNRELATED_CAPACITOR_ID: &str = "00000000-0000-4000-8000-000000000006";

fn compile_fixture(
    engine: &mut Engine,
    schematic_json: &str,
    geometry_json: &str,
    revision: &str,
) -> (
    open_circuit_engine::StableId,
    ResultLayout,
    ResultSnapshot,
    u32,
) {
    let result = engine.compile(CompileRequest {
        schematic: from_str(schematic_json).unwrap(),
        geometry: from_str(geometry_json).unwrap(),
        document_revision: revision.to_owned(),
    });
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        internal_substeps,
        diagnostics,
    } = result
    else {
        panic!("the RC fixture should compile: {result:?}");
    };
    assert!(
        !diagnostics
            .iter()
            .any(|item| item.severity == open_circuit_engine::DiagnosticSeverity::Error)
    );
    (run_id, layout, initial_snapshot, internal_substeps)
}

fn value(
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
    component_id: &str,
    quantity: ResultQuantity,
) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.component_id.as_str() == component_id && slot.quantity == quantity)
        .expect("the result layout contains the requested quantity");
    snapshot.values[slot.offset as usize]
}

fn assert_rc_fixture(
    schematic: &str,
    geometry: &str,
    expected_json: &str,
    ngspice_csv: &str,
    capacitor_id: &str,
    charging: bool,
) {
    let expected: Value = from_str(expected_json).unwrap();
    let ngspice = parse_ngspice_csv(ngspice_csv);
    let mut engine = Engine::default();
    let (run_id, layout, mut snapshot, internal_substeps) =
        compile_fixture(&mut engine, schematic, geometry, "rc-1");
    assert_eq!(internal_substeps, 1);

    let expected_initial_current = if charging { 0.01 } else { -0.01 };
    assert!(
        (value(&layout, &snapshot, capacitor_id, ResultQuantity::Current)
            - expected_initial_current)
            .abs()
            < 1e-12
    );

    for step in 1..=50 {
        snapshot = engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        let time = f64::from(step) * 0.001;
        let time_constant = 0.01;
        let analytical = if charging {
            10.0 * (1.0 - (-time / time_constant).exp())
        } else {
            10.0 * (-time / time_constant).exp()
        };
        let actual = value(&layout, &snapshot, capacitor_id, ResultQuantity::Voltage);
        assert!(
            (actual - analytical).abs() <= expected["analyticalToleranceVolts"].as_f64().unwrap(),
            "step {step}: {actual} V differs from analytic value {analytical} V"
        );

        for reference in expected["analytical"].as_array().unwrap() {
            if (time - reference["timeSeconds"].as_f64().unwrap()).abs() < 1e-12 {
                assert!((actual - reference["voltageVolts"].as_f64().unwrap()).abs() <= 0.01);
                let ngspice_value = ngspice
                    .iter()
                    .find(|(reference_time, _)| (time - reference_time).abs() < 1e-12)
                    .expect("ngspice reference includes the requested time")
                    .1;
                assert!(
                    (actual - ngspice_value).abs()
                        <= expected["ngspiceToleranceVolts"].as_f64().unwrap(),
                    "step {step}: {actual} V differs from ngspice value {ngspice_value} V"
                );
            }
        }
    }
}

fn parse_ngspice_csv(contents: &str) -> Vec<(f64, f64)> {
    contents
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("timeSeconds"))
        .map(|line| {
            let (time, voltage) = line.split_once(',').unwrap();
            (time.parse().unwrap(), voltage.parse().unwrap())
        })
        .collect()
}

#[test]
fn rc_charge_matches_analytical_and_ngspice_references() {
    assert_rc_fixture(
        CHARGE_SCHEMATIC,
        CHARGE_GEOMETRY,
        CHARGE_EXPECTED,
        CHARGE_NGSPICE,
        CAPACITOR_ID,
        true,
    );
}

#[test]
fn rc_discharge_matches_analytical_and_ngspice_references() {
    assert_rc_fixture(
        DISCHARGE_SCHEMATIC,
        DISCHARGE_GEOMETRY,
        DISCHARGE_EXPECTED,
        DISCHARGE_NGSPICE,
        DISCHARGE_CAPACITOR_ID,
        false,
    );
}

#[test]
fn unspecified_capacitor_initializes_from_dc_and_holds_its_operating_point() {
    let mut schematic_json: Value = from_str(CHARGE_SCHEMATIC).unwrap();
    let capacitor = schematic_json["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == "capacitor")
        .unwrap();
    capacitor["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("initialVoltageVolts");
    let schematic_json = serde_json::to_string(&schematic_json).unwrap();

    let mut engine = Engine::default();
    let (run_id, layout, initial_snapshot, internal_substeps) =
        compile_fixture(&mut engine, &schematic_json, CHARGE_GEOMETRY, "dc-init-1");
    assert_eq!(internal_substeps, 1);
    assert!(
        (value(
            &layout,
            &initial_snapshot,
            CAPACITOR_ID,
            ResultQuantity::Voltage
        ) - 10.0)
            .abs()
            < 1e-9
    );
    assert!(
        value(
            &layout,
            &initial_snapshot,
            CAPACITOR_ID,
            ResultQuantity::Current
        )
        .abs()
            < 1e-12
    );

    let held = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1000,
        })
        .unwrap();
    assert!((value(&layout, &held, CAPACITOR_ID, ResultQuantity::Voltage) - 10.0).abs() < 1e-9);
    assert!(value(&layout, &held, CAPACITOR_ID, ResultQuantity::Current).abs() < 1e-12);
}

#[test]
fn capacitor_parameter_is_optional_but_null_and_non_positive_capacitance_are_invalid() {
    let mut schematic_json: Value = from_str(CHARGE_SCHEMATIC).unwrap();
    let capacitor = schematic_json["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == "capacitor")
        .unwrap();
    capacitor["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("initialVoltageVolts");
    let without_initial: Schematic = from_value(schematic_json.clone()).unwrap();
    assert!(without_initial.validate().is_empty());
    let serialized = serde_json::to_value(without_initial).unwrap();
    assert!(
        serialized["components"][2]["parameters"]
            .get("initialVoltageVolts")
            .is_none()
    );

    schematic_json["components"][2]["parameters"]["initialVoltageVolts"] = Value::Null;
    assert!(from_value::<Schematic>(schematic_json).is_err());

    let mut invalid: Schematic = from_str(CHARGE_SCHEMATIC).unwrap();
    let ComponentSpec::Capacitor { parameters, .. } = &mut invalid.components[2] else {
        panic!("fixture component is a capacitor");
    };
    parameters.capacitance_farads = 0.0;
    let mut engine = Engine::default();
    let result = engine.compile(CompileRequest {
        schematic: invalid,
        geometry: from_str(CHARGE_GEOMETRY).unwrap(),
        document_revision: "invalid-capacitance".to_owned(),
    });
    assert!(matches!(
        result,
        CompileResult::Failure { diagnostics }
            if diagnostics.iter().any(|item| item.code == DiagnosticCode::InvalidParameter)
    ));
}

#[test]
fn incompatible_explicit_capacitor_voltage_reports_inconsistent_state() {
    let mut schematic_json: Value = from_str(CHARGE_SCHEMATIC).unwrap();
    schematic_json["components"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": UNRELATED_CAPACITOR_ID,
            "typeId": "capacitor",
            "position": { "x": 10, "y": 10 },
            "rotationDeg": 0,
            "parameters": { "capacitanceFarads": 0.00001, "initialVoltageVolts": 2.0 }
        }));
    let mut schematic: Schematic = from_value(schematic_json).unwrap();
    let mut geometry_json: Value = from_str(CHARGE_GEOMETRY).unwrap();
    geometry_json["components"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "componentId": UNRELATED_CAPACITOR_ID,
            "pins": [
                { "pinId": "pin1", "position": { "x": 20, "y": 20 } },
                { "pinId": "pin2", "position": { "x": 22, "y": 20 } }
            ]
        }));
    let mut geometry: open_circuit_engine::GeometryEnvelope = from_value(geometry_json).unwrap();
    let capacitor = schematic
        .components
        .iter_mut()
        .find_map(|component| match component {
            ComponentSpec::Capacitor { parameters, .. } => Some(parameters),
            _ => None,
        })
        .unwrap();
    capacitor.initial_voltage_volts = Some(0.0);
    let capacitor_geometry = geometry
        .components
        .iter_mut()
        .find(|component| component.component_id.as_str() == CAPACITOR_ID)
        .unwrap();
    capacitor_geometry.pins[0].position = open_circuit_engine::GridPoint { x: 0, y: 10 };
    capacitor_geometry.pins[1].position = open_circuit_engine::GridPoint { x: 0, y: 0 };

    let result = Engine::default().compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "inconsistent-ic".to_owned(),
    });
    let CompileResult::Failure { diagnostics } = result else {
        panic!("the incompatible explicit voltage should fail compilation");
    };
    let diagnostic = diagnostics
        .iter()
        .find(|item| item.code == DiagnosticCode::InconsistentState)
        .expect("the explicit capacitor state conflict should be diagnosed");
    let component_id: open_circuit_engine::StableId =
        serde_json::from_str(&format!("\"{CAPACITOR_ID}\"")).unwrap();
    assert_eq!(
        diagnostic.references,
        vec![open_circuit_engine::DiagnosticReference::Component { component_id }]
    );
}

#[test]
fn signed_initial_voltage_is_valid_and_a_fast_rc_rate_emits_accuracy_warning() {
    let mut negative_initial: Value = from_str(DISCHARGE_SCHEMATIC).unwrap();
    negative_initial["components"][1]["parameters"]["initialVoltageVolts"] = (-10.0).into();
    let negative_initial = serde_json::to_string(&negative_initial).unwrap();
    let mut engine = Engine::default();
    let (_, layout, snapshot, _) = compile_fixture(
        &mut engine,
        &negative_initial,
        DISCHARGE_GEOMETRY,
        "negative-initial-1",
    );
    assert!(
        (value(
            &layout,
            &snapshot,
            DISCHARGE_CAPACITOR_ID,
            ResultQuantity::Voltage,
        ) + 10.0)
            .abs()
            < 1e-12
    );
    assert!(
        (value(
            &layout,
            &snapshot,
            DISCHARGE_CAPACITOR_ID,
            ResultQuantity::Current,
        ) - 0.01)
            .abs()
            < 1e-12
    );

    let mut fast_schematic: Schematic = from_str(CHARGE_SCHEMATIC).unwrap();
    if let ComponentSpec::Resistor { parameters, .. } = &mut fast_schematic.components[1] {
        parameters.resistance_ohms = 0.01;
    }
    if let ComponentSpec::Capacitor { parameters, .. } = &mut fast_schematic.components[2] {
        parameters.capacitance_farads = 1e-6;
    }
    let result = Engine::default().compile(CompileRequest {
        schematic: fast_schematic,
        geometry: from_str(CHARGE_GEOMETRY).unwrap(),
        document_revision: "fast-rc-1".to_owned(),
    });
    let CompileResult::Success {
        internal_substeps,
        diagnostics,
        ..
    } = result
    else {
        panic!("the fast RC fixture should compile: {result:?}");
    };
    assert_eq!(internal_substeps, 64);
    assert!(
        diagnostics
            .iter()
            .any(|item| item.code == DiagnosticCode::StepAccuracyLimited)
    );
}

#[test]
fn changing_capacitance_recomputes_the_current_step_from_its_start_state() {
    let mut engine = Engine::default();
    let (run_id, layout, _, _) = compile_fixture(
        &mut engine,
        CHARGE_SCHEMATIC,
        CHARGE_GEOMETRY,
        "cap-update-1",
    );
    let snapshot = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 3,
        })
        .unwrap();
    let updated = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "cap-update-1".to_owned(),
        document_revision: "cap-update-2".to_owned(),
        component_id: serde_json::from_str(&format!("\"{CAPACITOR_ID}\"")).unwrap(),
        parameter_id: ParameterId::CapacitanceFarads,
        value: 2e-5,
    });
    let SetParameterResult::Success {
        snapshot: updated,
        internal_substeps,
        ..
    } = updated
    else {
        panic!("valid capacitance change should be accepted");
    };
    assert_eq!(updated.metadata.simulation_step, 3);
    assert_eq!(updated.metadata.document_revision, "cap-update-2");
    assert_eq!(internal_substeps, 1);

    let ratio_old = (1.0_f64 - 0.001 / (2.0 * 0.01)) / (1.0 + 0.001 / (2.0 * 0.01));
    let voltage_at_step_2 = 10.0 * (1.0 - ratio_old.powi(2));
    let ratio_new = (1.0_f64 - 0.001 / (2.0 * 0.02)) / (1.0 + 0.001 / (2.0 * 0.02));
    let expected_voltage = 10.0 + (voltage_at_step_2 - 10.0) * ratio_new;
    let actual_voltage = value(&layout, &updated, CAPACITOR_ID, ResultQuantity::Voltage);
    assert!((actual_voltage - expected_voltage).abs() < 1e-9);
    assert_ne!(
        value(&layout, &snapshot, CAPACITOR_ID, ResultQuantity::Voltage),
        actual_voltage
    );
}

#[test]
fn initial_voltage_cannot_be_changed_as_a_live_parameter() {
    let mut engine = Engine::default();
    let (run_id, _, snapshot, _) = compile_fixture(
        &mut engine,
        CHARGE_SCHEMATIC,
        CHARGE_GEOMETRY,
        "initial-voltage-live-edit-1",
    );
    let result = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "initial-voltage-live-edit-1".to_owned(),
        document_revision: "initial-voltage-live-edit-2".to_owned(),
        component_id: serde_json::from_str(&format!("\"{CAPACITOR_ID}\"")).unwrap(),
        parameter_id: ParameterId::InitialVoltageVolts,
        value: 2.0,
    });

    assert!(matches!(
        result,
        SetParameterResult::Failure { diagnostics }
            if diagnostics.iter().any(|item| item.code == DiagnosticCode::InvalidParameter)
    ));
    assert_eq!(engine.latest_snapshot(), Some(&snapshot));
}
