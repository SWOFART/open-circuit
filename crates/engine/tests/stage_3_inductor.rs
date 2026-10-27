use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentSpec, DiagnosticCode, Engine,
    ParameterId, ResultLayout, ResultQuantity, ResultSnapshot, Schematic, SetParameterRequest,
    SetParameterResult,
};
use serde_json::{Value, from_str, from_value};

const RL_RISE_SCHEMATIC: &str =
    include_str!("../../../tests/golden/stage-3-rl-rise.schematic.json");
const RL_RISE_GEOMETRY: &str = include_str!("../../../tests/golden/stage-3-rl-rise.geometry.json");
const RL_RISE_EXPECTED: &str = include_str!("../../../tests/golden/stage-3-rl-rise.expected.json");
const RL_RISE_NGSPICE: &str = include_str!("../../../tests/golden/stage-3-ngspice-rl-rise.csv");
const RL_DECAY_SCHEMATIC: &str =
    include_str!("../../../tests/golden/stage-3-rl-decay.schematic.json");
const RL_DECAY_GEOMETRY: &str =
    include_str!("../../../tests/golden/stage-3-rl-decay.geometry.json");
const RL_DECAY_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-3-rl-decay.expected.json");
const RL_DECAY_NGSPICE: &str = include_str!("../../../tests/golden/stage-3-ngspice-rl-decay.csv");
const DC_INDUCTOR_SCHEMATIC: &str =
    include_str!("../../../tests/golden/stage-3-dc-inductor.schematic.json");
const DC_INDUCTOR_GEOMETRY: &str =
    include_str!("../../../tests/golden/stage-3-dc-inductor.geometry.json");
const LC_SCHEMATIC: &str = include_str!("../../../tests/golden/stage-3-lossless-lc.schematic.json");
const LC_GEOMETRY: &str = include_str!("../../../tests/golden/stage-3-lossless-lc.geometry.json");
const LC_EXPECTED: &str = include_str!("../../../tests/golden/stage-3-lossless-lc.expected.json");
const LC_NGSPICE: &str = include_str!("../../../tests/golden/stage-3-ngspice-lossless-lc.csv");

const RL_RISE_INDUCTOR_ID: &str = "00000000-0000-4000-8000-000000000003";
const RL_DECAY_INDUCTOR_ID: &str = "00000000-0000-4000-8000-000000000002";
const LC_CAPACITOR_ID: &str = "00000000-0000-4000-8000-000000000001";
const LC_INDUCTOR_ID: &str = "00000000-0000-4000-8000-000000000002";

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
        faults: None,
    });
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        internal_substeps,
        diagnostics,
    } = result
    else {
        panic!("the fixture should compile: {result:?}");
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

fn parse_ngspice_current(contents: &str) -> Vec<(f64, f64)> {
    contents
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("timeSeconds"))
        .map(|line| {
            let (time, current) = line.split_once(',').unwrap();
            (time.parse().unwrap(), current.parse().unwrap())
        })
        .collect()
}

fn assert_rl_fixture(
    schematic_json: &str,
    geometry_json: &str,
    expected_json: &str,
    ngspice_csv: &str,
    inductor_id: &str,
    rising: bool,
) {
    let expected: Value = from_str(expected_json).unwrap();
    let ngspice = parse_ngspice_current(ngspice_csv);
    let mut engine = Engine::default();
    let (run_id, layout, mut snapshot, internal_substeps) =
        compile_fixture(&mut engine, schematic_json, geometry_json, "rl-1");
    assert_eq!(internal_substeps, 1);
    assert_eq!(
        value(&layout, &snapshot, inductor_id, ResultQuantity::Current),
        if rising { 0.0 } else { 1.0 }
    );
    assert_eq!(
        value(&layout, &snapshot, inductor_id, ResultQuantity::Voltage),
        if rising { 10.0 } else { -10.0 }
    );

    for step in 1..=50 {
        snapshot = engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        let time = f64::from(step) * 0.001;
        let analytical = if rising {
            1.0 - (-time / 0.01).exp()
        } else {
            (-time / 0.01).exp()
        };
        let actual = value(&layout, &snapshot, inductor_id, ResultQuantity::Current);
        assert!(
            (actual - analytical).abs() <= expected["analyticalToleranceAmps"].as_f64().unwrap(),
            "step {step}: {actual} A differs from analytic value {analytical} A"
        );
        for reference in expected["analytical"].as_array().unwrap() {
            if (time - reference["timeSeconds"].as_f64().unwrap()).abs() < 1e-12 {
                assert!((actual - reference["currentAmps"].as_f64().unwrap()).abs() <= 0.001);
                let ngspice_value = ngspice
                    .iter()
                    .find(|(reference_time, _)| (time - reference_time).abs() < 1e-12)
                    .expect("ngspice reference includes the requested time")
                    .1;
                assert!(
                    (actual - ngspice_value).abs()
                        <= expected["ngspiceToleranceAmps"].as_f64().unwrap(),
                    "step {step}: {actual} A differs from ngspice value {ngspice_value} A"
                );
            }
        }
    }
}

#[test]
fn rl_current_rise_matches_analytical_and_ngspice_references() {
    assert_rl_fixture(
        RL_RISE_SCHEMATIC,
        RL_RISE_GEOMETRY,
        RL_RISE_EXPECTED,
        RL_RISE_NGSPICE,
        RL_RISE_INDUCTOR_ID,
        true,
    );
}

#[test]
fn rl_current_decay_matches_analytical_and_ngspice_references() {
    assert_rl_fixture(
        RL_DECAY_SCHEMATIC,
        RL_DECAY_GEOMETRY,
        RL_DECAY_EXPECTED,
        RL_DECAY_NGSPICE,
        RL_DECAY_INDUCTOR_ID,
        false,
    );
}

#[test]
fn unspecified_inductor_initializes_from_dc_operating_point() {
    let mut engine = Engine::default();
    let (run_id, layout, snapshot, internal_substeps) = compile_fixture(
        &mut engine,
        DC_INDUCTOR_SCHEMATIC,
        DC_INDUCTOR_GEOMETRY,
        "dc-inductor-1",
    );
    assert_eq!(internal_substeps, 8);
    assert!(
        (value(
            &layout,
            &snapshot,
            RL_RISE_INDUCTOR_ID,
            ResultQuantity::Current
        ) - 0.005)
            .abs()
            < 1e-12
    );
    assert!(
        value(
            &layout,
            &snapshot,
            RL_RISE_INDUCTOR_ID,
            ResultQuantity::Voltage
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
    assert!(
        (value(&layout, &held, RL_RISE_INDUCTOR_ID, ResultQuantity::Current) - 0.005).abs() < 1e-9
    );
    assert!(value(&layout, &held, RL_RISE_INDUCTOR_ID, ResultQuantity::Voltage).abs() < 1e-6);
}

#[test]
fn lossless_parallel_lc_meets_period_energy_and_ngspice_tolerances() {
    let expected: Value = from_str(LC_EXPECTED).unwrap();
    let references: Vec<(f64, f64, f64)> = LC_NGSPICE
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("timeSeconds"))
        .map(|line| {
            let mut values = line.split(',').map(|value| value.parse::<f64>().unwrap());
            (
                values.next().unwrap(),
                values.next().unwrap(),
                values.next().unwrap(),
            )
        })
        .collect();
    let ngspice_voltages = references
        .iter()
        .map(|(_, voltage, _)| *voltage)
        .collect::<Vec<_>>();
    let ngspice_currents = references
        .iter()
        .map(|(_, _, current)| *current)
        .collect::<Vec<_>>();
    let mut engine = Engine::default();
    let (run_id, layout, mut snapshot, internal_substeps) =
        compile_fixture(&mut engine, LC_SCHEMATIC, LC_GEOMETRY, "lc-1");
    assert_eq!(internal_substeps, 2);

    let mut voltages = vec![value(
        &layout,
        &snapshot,
        LC_CAPACITOR_ID,
        ResultQuantity::Voltage,
    )];
    for step in 1..=10_000 {
        snapshot = engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        let voltage = value(&layout, &snapshot, LC_CAPACITOR_ID, ResultQuantity::Voltage);
        let current = value(&layout, &snapshot, LC_INDUCTOR_ID, ResultQuantity::Current);
        let energy = 0.5 * 1e-4 * voltage * voltage + 0.5 * 0.1 * current * current;
        let relative_error = (energy - expected["storedEnergyJoules"].as_f64().unwrap()).abs()
            / expected["storedEnergyJoules"].as_f64().unwrap();
        assert!(
            relative_error <= expected["energyRelativeTolerance"].as_f64().unwrap(),
            "step {step}: stored energy {energy} J is outside tolerance"
        );

        if step < 200 {
            voltages.push(voltage);
        }
    }

    let estimate_period = |samples: &[f64]| {
        let numerator = (1..=198)
            .map(|index| samples[index] * (samples[index + 1] + samples[index - 1]))
            .sum::<f64>();
        let denominator = 2.0
            * samples[1..=198]
                .iter()
                .map(|sample| sample * sample)
                .sum::<f64>();
        2.0 * std::f64::consts::PI * 0.001 / (numerator / denominator).clamp(-1.0, 1.0).acos()
    };
    let expected_period = expected["periodSeconds"].as_f64().unwrap();
    let tolerance = expected["periodRelativeTolerance"].as_f64().unwrap();
    let engine_period = estimate_period(&voltages);
    let ngspice_voltage_period = estimate_period(&ngspice_voltages);
    let ngspice_current_period = estimate_period(&ngspice_currents);
    for (source, period) in [
        ("Engine", engine_period),
        ("ngspice voltage", ngspice_voltage_period),
        ("ngspice current", ngspice_current_period),
    ] {
        let relative_error = (period - expected_period).abs() / expected_period;
        assert!(
            relative_error <= tolerance,
            "{source} period {period} s exceeds the analytical tolerance"
        );
    }
    assert!(
        (engine_period - ngspice_voltage_period).abs() / ngspice_voltage_period <= tolerance,
        "Engine and ngspice periods differ by more than the contract tolerance"
    );
    assert!(
        (ngspice_voltage_period - ngspice_current_period).abs() / ngspice_voltage_period
            <= tolerance,
        "ngspice voltage and current periods disagree"
    );
}

#[test]
fn inductor_requires_positive_inductance_and_optional_initial_current_rejects_null() {
    let mut schematic_json: Value = from_str(RL_RISE_SCHEMATIC).unwrap();
    schematic_json["components"][2]["parameters"]["initialCurrentAmps"] = Value::Null;
    assert!(from_value::<Schematic>(schematic_json.clone()).is_err());

    schematic_json["components"][2]["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("initialCurrentAmps");
    let mut schematic: Schematic = from_value(schematic_json).unwrap();
    let ComponentSpec::Inductor { parameters, .. } = &mut schematic.components[2] else {
        panic!("fixture component is an inductor");
    };
    parameters.inductance_henries = 0.0;
    let result = Engine::default().compile(CompileRequest {
        schematic,
        geometry: from_str(RL_RISE_GEOMETRY).unwrap(),
        document_revision: "invalid-inductance".to_owned(),
        faults: None,
    });
    assert!(matches!(
        result,
        CompileResult::Failure { diagnostics }
            if diagnostics.iter().any(|item| item.code == DiagnosticCode::InvalidParameter)
    ));

    let mut negative: Schematic = from_str(RL_RISE_SCHEMATIC).unwrap();
    let ComponentSpec::Inductor { parameters, .. } = &mut negative.components[2] else {
        panic!("fixture component is an inductor");
    };
    parameters.inductance_henries = -0.1;
    let result = Engine::default().compile(CompileRequest {
        schematic: negative,
        geometry: from_str(RL_RISE_GEOMETRY).unwrap(),
        document_revision: "negative-inductance".to_owned(),
        faults: None,
    });
    assert!(matches!(
        result,
        CompileResult::Failure { diagnostics }
            if diagnostics.iter().any(|item| item.code == DiagnosticCode::InvalidParameter)
    ));
}

#[test]
fn signed_initial_current_is_valid_and_initial_current_is_not_a_live_parameter() {
    let mut schematic_json: Value = from_str(RL_DECAY_SCHEMATIC).unwrap();
    schematic_json["components"][1]["parameters"]["initialCurrentAmps"] = (-1.0).into();
    let signed_schematic = serde_json::to_string(&schematic_json).unwrap();
    let mut engine = Engine::default();
    let (run_id, layout, snapshot, _) = compile_fixture(
        &mut engine,
        &signed_schematic,
        RL_DECAY_GEOMETRY,
        "negative-inductor-current-1",
    );
    assert_eq!(
        value(
            &layout,
            &snapshot,
            RL_DECAY_INDUCTOR_ID,
            ResultQuantity::Current
        ),
        -1.0
    );
    assert_eq!(
        value(
            &layout,
            &snapshot,
            RL_DECAY_INDUCTOR_ID,
            ResultQuantity::Voltage
        ),
        10.0
    );

    let result = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "negative-inductor-current-1".to_owned(),
        document_revision: "negative-inductor-current-2".to_owned(),
        component_id: from_str(&format!("\"{RL_DECAY_INDUCTOR_ID}\"")).unwrap(),
        parameter_id: ParameterId::InitialCurrentAmps,
        value: 0.5,
    });
    assert!(matches!(
        result,
        SetParameterResult::Failure { diagnostics }
            if diagnostics.iter().any(|item| item.code == DiagnosticCode::InvalidParameter)
    ));
    assert_eq!(engine.latest_snapshot(), Some(&snapshot));
}

#[test]
fn unequal_explicit_currents_in_series_inductors_report_inconsistent_state() {
    let schematic_json = serde_json::json!({
        "schemaVersion": 1,
        "components": [
            {
                "id": "00000000-0000-4000-8000-000000000001",
                "typeId": "inductor", "position": { "x": 0, "y": 0 }, "rotationDeg": 0,
                "parameters": { "inductanceHenries": 0.1, "initialCurrentAmps": 1.0 }
            },
            {
                "id": "00000000-0000-4000-8000-000000000002",
                "typeId": "inductor", "position": { "x": 1, "y": 0 }, "rotationDeg": 0,
                "parameters": { "inductanceHenries": 0.1, "initialCurrentAmps": 0.5 }
            }
        ],
        "wires": [], "junctions": [],
        "referenceMarkers": [{
            "id": "00000000-0000-4000-8000-000000000003",
            "position": { "x": 0, "y": 0 }
        }]
    });
    let geometry_json = serde_json::json!({
        "components": [
            {"componentId":"00000000-0000-4000-8000-000000000001","pins":[
                {"pinId":"pin1","position":{"x":4,"y":6}},
                {"pinId":"pin2","position":{"x":2,"y":3}}
            ]},
            {"componentId":"00000000-0000-4000-8000-000000000002","pins":[
                {"pinId":"pin1","position":{"x":2,"y":3}},
                {"pinId":"pin2","position":{"x":0,"y":0}}
            ]}
        ]
    });
    let result = Engine::default().compile(CompileRequest {
        schematic: from_value(schematic_json).unwrap(),
        geometry: from_value(geometry_json).unwrap(),
        document_revision: "conflicting-inductor-currents".to_owned(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = result else {
        panic!("incompatible current states must not be silently accepted");
    };
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::InconsistentState)
        .expect("the incompatible current states should be diagnosed");
    assert_eq!(diagnostic.references.len(), 2);
}

#[test]
fn explicit_inductor_current_is_checked_against_unspecified_dc_state() {
    let schematic_json = serde_json::json!({
        "schemaVersion": 1,
        "components": [
            {
                "id": "00000000-0000-4000-8000-000000000001",
                "typeId": "dc_voltage_source", "position": { "x": 0, "y": 0 }, "rotationDeg": 0,
                "parameters": { "voltageVolts": 5, "internalResistanceOhms": 0 }
            },
            {
                "id": "00000000-0000-4000-8000-000000000002",
                "typeId": "resistor", "position": { "x": 1, "y": 0 }, "rotationDeg": 0,
                "parameters": { "resistanceOhms": 1000 }
            },
            {
                "id": "00000000-0000-4000-8000-000000000003",
                "typeId": "inductor", "position": { "x": 2, "y": 0 }, "rotationDeg": 0,
                "parameters": { "inductanceHenries": 1, "initialCurrentAmps": 0 }
            },
            {
                "id": "00000000-0000-4000-8000-000000000004",
                "typeId": "inductor", "position": { "x": 3, "y": 0 }, "rotationDeg": 0,
                "parameters": { "inductanceHenries": 1 }
            }
        ],
        "wires": [], "junctions": [],
        "referenceMarkers": [{
            "id": "00000000-0000-4000-8000-000000000005",
            "position": { "x": 0, "y": 0 }
        }]
    });
    let geometry_json = serde_json::json!({
        "components": [
            {"componentId":"00000000-0000-4000-8000-000000000001","pins":[
                {"pinId":"pin1","position":{"x":0,"y":10}},
                {"pinId":"pin2","position":{"x":0,"y":0}}
            ]},
            {"componentId":"00000000-0000-4000-8000-000000000002","pins":[
                {"pinId":"pin1","position":{"x":0,"y":10}},
                {"pinId":"pin2","position":{"x":4,"y":6}}
            ]},
            {"componentId":"00000000-0000-4000-8000-000000000003","pins":[
                {"pinId":"pin1","position":{"x":4,"y":6}},
                {"pinId":"pin2","position":{"x":8,"y":2}}
            ]},
            {"componentId":"00000000-0000-4000-8000-000000000004","pins":[
                {"pinId":"pin1","position":{"x":8,"y":2}},
                {"pinId":"pin2","position":{"x":0,"y":0}}
            ]}
        ]
    });
    let result = Engine::default().compile(CompileRequest {
        schematic: from_value(schematic_json).unwrap(),
        geometry: from_value(geometry_json).unwrap(),
        document_revision: "one-explicit-series-current".to_owned(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = result else {
        panic!("the explicit current must conflict with the DC state of L2");
    };
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::InconsistentState)
        .expect("the mixed explicit and DC initial currents should be diagnosed");
    assert_eq!(diagnostic.references.len(), 1);
    assert_eq!(
        diagnostic.references[0],
        open_circuit_engine::DiagnosticReference::Component {
            component_id: from_str("\"00000000-0000-4000-8000-000000000003\"").unwrap(),
        }
    );
}

#[test]
fn dc_inductor_short_conflicting_with_an_ideal_source_reports_constraint_error() {
    let schematic = serde_json::json!({
        "schemaVersion": 1,
        "components": [
            {
                "id": "00000000-0000-4000-8000-000000000001",
                "typeId": "dc_voltage_source", "position": { "x": 0, "y": 0 }, "rotationDeg": 0,
                "parameters": { "voltageVolts": 5, "internalResistanceOhms": 0 }
            },
            {
                "id": "00000000-0000-4000-8000-000000000002",
                "typeId": "inductor", "position": { "x": 1, "y": 0 }, "rotationDeg": 0,
                "parameters": { "inductanceHenries": 1 }
            }
        ],
        "wires": [], "junctions": [],
        "referenceMarkers": [{
            "id": "00000000-0000-4000-8000-000000000003",
            "position": { "x": 0, "y": 0 }
        }]
    });
    let geometry = serde_json::json!({
        "components": [
            {"componentId":"00000000-0000-4000-8000-000000000001","pins":[
                {"pinId":"pin1","position":{"x":0,"y":10}},
                {"pinId":"pin2","position":{"x":0,"y":0}}
            ]},
            {"componentId":"00000000-0000-4000-8000-000000000002","pins":[
                {"pinId":"pin1","position":{"x":0,"y":10}},
                {"pinId":"pin2","position":{"x":0,"y":0}}
            ]}
        ]
    });
    let result = Engine::default().compile(CompileRequest {
        schematic: from_value(schematic).unwrap(),
        geometry: from_value(geometry).unwrap(),
        document_revision: "source-across-unspecified-inductor".to_owned(),
        faults: None,
    });
    assert!(matches!(
        result,
        CompileResult::Failure { diagnostics }
            if diagnostics.iter().any(|item| item.code == DiagnosticCode::InconsistentIdealConstraint)
    ));
}

#[test]
fn changing_inductance_recomputes_the_current_step_from_its_start_state() {
    let mut engine = Engine::default();
    let (run_id, layout, _, _) = compile_fixture(
        &mut engine,
        RL_RISE_SCHEMATIC,
        RL_RISE_GEOMETRY,
        "inductor-update-1",
    );
    let previous = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 3,
        })
        .unwrap();
    let updated = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "inductor-update-1".to_owned(),
        document_revision: "inductor-update-2".to_owned(),
        component_id: from_str(&format!("\"{RL_RISE_INDUCTOR_ID}\"")).unwrap(),
        parameter_id: ParameterId::InductanceHenries,
        value: 0.2,
    });
    let SetParameterResult::Success { snapshot, .. } = updated else {
        panic!("valid inductance change should be accepted");
    };
    assert_eq!(snapshot.metadata.simulation_step, 3);
    assert_eq!(snapshot.metadata.document_revision, "inductor-update-2");
    assert_ne!(
        value(
            &layout,
            &previous,
            RL_RISE_INDUCTOR_ID,
            ResultQuantity::Current
        ),
        value(
            &layout,
            &snapshot,
            RL_RISE_INDUCTOR_ID,
            ResultQuantity::Current
        )
    );
}
