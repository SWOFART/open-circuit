use open_circuit_engine::{
    AdvanceRequest, CheckCriterion, CheckOutcome, CheckRequest, CheckVerdict, CompileRequest,
    CompileResult, ComponentSpec, CriterionResult, DiagnosticCode, Engine, ParameterId,
    ResultLayout, ResultQuantity, ResultSnapshot, SetParameterRequest, SetParameterResult,
    StableId, Unit, component_catalog, evaluate_checks,
};
use serde_json::{Value, from_value, json};
use std::path::PathBuf;

const SOURCE_ID: &str = "00000000-0000-4000-8000-000000000001";
const RESISTOR_ID: &str = "00000000-0000-4000-8000-000000000002";
const CAPACITOR_ID: &str = "00000000-0000-4000-8000-000000000003";
const REFERENCE_ID: &str = "00000000-0000-4000-8000-000000000004";

fn source_request(
    frequency_hz: f64,
    phase_offset_degrees: f64,
    internal_resistance_ohms: f64,
    load_resistance_ohms: f64,
    capacitance_farads: Option<f64>,
) -> CompileRequest {
    let mut components = vec![
        json!({
            "id": SOURCE_ID,
            "typeId": "ac_voltage_source",
            "position": { "x": 0, "y": 0 },
            "rotationDeg": 0,
            "parameters": {
                "voltageRmsVolts": 10.0,
                "frequencyHz": frequency_hz,
                "phaseOffsetDegrees": phase_offset_degrees,
                "internalResistanceOhms": internal_resistance_ohms
            }
        }),
        json!({
            "id": RESISTOR_ID,
            "typeId": "resistor",
            "position": { "x": 1, "y": 0 },
            "rotationDeg": 0,
            "parameters": { "resistanceOhms": load_resistance_ohms }
        }),
    ];
    let mut geometry_components = vec![
        json!({
            "componentId": SOURCE_ID,
            "pins": [
                { "pinId": "pin1", "position": { "x": 0, "y": 10 } },
                { "pinId": "pin2", "position": { "x": 0, "y": 0 } }
            ]
        }),
        json!({
            "componentId": RESISTOR_ID,
            "pins": [
                { "pinId": "pin1", "position": { "x": 0, "y": 10 } },
                { "pinId": "pin2", "position": { "x": 0, "y": 0 } }
            ]
        }),
    ];
    if let Some(capacitance_farads) = capacitance_farads {
        components[1]["parameters"]["resistanceOhms"] = json!(100.0);
        components.push(json!({
            "id": CAPACITOR_ID,
            "typeId": "capacitor",
            "position": { "x": 2, "y": 0 },
            "rotationDeg": 0,
            "parameters": {
                "capacitanceFarads": capacitance_farads,
                "initialVoltageVolts": 0.0
            }
        }));
        geometry_components[1]["pins"][1]["position"] = json!({ "x": 4, "y": 6 });
        geometry_components.push(json!({
            "componentId": CAPACITOR_ID,
            "pins": [
                { "pinId": "pin1", "position": { "x": 4, "y": 6 } },
                { "pinId": "pin2", "position": { "x": 0, "y": 0 } }
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
                { "id": REFERENCE_ID, "position": { "x": 0, "y": 0 } }
            ],
            "rails": []
        }))
        .unwrap(),
        geometry: from_value(json!({ "components": geometry_components })).unwrap(),
        document_revision: format!("ac-{frequency_hz}-{phase_offset_degrees}"),
        faults: None,
    }
}

fn compile(request: CompileRequest) -> (Engine, StableId, ResultLayout, ResultSnapshot, u32) {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        internal_substeps,
        diagnostics,
    } = engine.compile(request)
    else {
        panic!("AC source fixture should compile");
    };
    assert!(
        diagnostics
            .iter()
            .all(|item| item.code != DiagnosticCode::InvalidParameter)
    );
    (engine, run_id, layout, initial_snapshot, internal_substeps)
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
        .expect("the layout contains the requested result");
    snapshot.values[slot.offset as usize]
}

fn golden(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn close(actual: f64, expected: f64, tolerance: f64, description: &str) {
    assert!(
        actual.is_finite() && (actual - expected).abs() <= tolerance,
        "{description}: {actual} differs from {expected} by more than {tolerance}"
    );
}

#[test]
fn ac_source_catalog_and_parameter_domains_match_the_contract() {
    let catalog = component_catalog();
    let entry = catalog
        .component_types
        .iter()
        .find(|entry| entry.type_id == open_circuit_engine::ComponentTypeId::AcVoltageSource)
        .expect("the catalog contains the Stage 9 source");
    assert_eq!(
        entry
            .pins
            .iter()
            .map(|pin| pin.id.as_str())
            .collect::<Vec<_>>(),
        ["pin1", "pin2"]
    );
    assert!(entry.fault_kinds.is_empty());
    let parameter = |id| {
        entry
            .parameters
            .iter()
            .find(|parameter| parameter.id == id)
            .unwrap()
    };
    assert_eq!(parameter(ParameterId::VoltageRmsVolts).default_value, 230.0);
    assert_eq!(parameter(ParameterId::VoltageRmsVolts).unit, Unit::Volt);
    assert_eq!(
        parameter(ParameterId::VoltageRmsVolts).minimum_value,
        Some(0.001)
    );
    assert_eq!(
        parameter(ParameterId::VoltageRmsVolts).maximum_value,
        Some(100_000.0)
    );
    assert_eq!(parameter(ParameterId::FrequencyHz).default_value, 50.0);
    assert_eq!(parameter(ParameterId::FrequencyHz).unit, Unit::Hertz);
    assert_eq!(parameter(ParameterId::FrequencyHz).minimum_value, Some(1.0));
    assert_eq!(
        parameter(ParameterId::FrequencyHz).maximum_value,
        Some(400.0)
    );
    assert_eq!(
        parameter(ParameterId::PhaseOffsetDegrees).default_value,
        0.0
    );
    assert_eq!(
        parameter(ParameterId::PhaseOffsetDegrees).unit,
        Unit::Degree
    );
    assert_eq!(
        parameter(ParameterId::PhaseOffsetDegrees).minimum_value,
        Some(-360.0)
    );
    assert_eq!(
        parameter(ParameterId::PhaseOffsetDegrees).maximum_value,
        Some(360.0)
    );
    assert_eq!(
        parameter(ParameterId::InternalResistanceOhms).default_value,
        0.1
    );

    let mut schematic = source_request(50.0, 0.0, 0.1, 100.0, None).schematic;
    for (parameter_id, invalid) in [
        (ParameterId::VoltageRmsVolts, 0.0009),
        (ParameterId::VoltageRmsVolts, 100_000.1),
        (ParameterId::FrequencyHz, 0.9),
        (ParameterId::FrequencyHz, 400.1),
        (ParameterId::PhaseOffsetDegrees, -360.1),
        (ParameterId::PhaseOffsetDegrees, 360.1),
        (ParameterId::InternalResistanceOhms, -0.1),
    ] {
        let previous = {
            let Some(ComponentSpec::AcVoltageSource { parameters, .. }) =
                schematic.components.first_mut()
            else {
                panic!("the fixture starts with an AC source");
            };
            match parameter_id {
                ParameterId::VoltageRmsVolts => {
                    let old = parameters.voltage_rms_volts;
                    parameters.voltage_rms_volts = invalid;
                    old
                }
                ParameterId::FrequencyHz => {
                    let old = parameters.frequency_hz;
                    parameters.frequency_hz = invalid;
                    old
                }
                ParameterId::PhaseOffsetDegrees => {
                    let old = parameters.phase_offset_degrees;
                    parameters.phase_offset_degrees = invalid;
                    old
                }
                ParameterId::InternalResistanceOhms => {
                    let old = parameters.internal_resistance_ohms;
                    parameters.internal_resistance_ohms = invalid;
                    old
                }
                _ => unreachable!(),
            }
        };
        assert!(
            schematic
                .validate()
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidParameter),
            "{parameter_id:?}={invalid} must be rejected"
        );
        let Some(ComponentSpec::AcVoltageSource { parameters, .. }) =
            schematic.components.first_mut()
        else {
            panic!("the fixture starts with an AC source");
        };
        match parameter_id {
            ParameterId::VoltageRmsVolts => parameters.voltage_rms_volts = previous,
            ParameterId::FrequencyHz => parameters.frequency_hz = previous,
            ParameterId::PhaseOffsetDegrees => parameters.phase_offset_degrees = previous,
            ParameterId::InternalResistanceOhms => parameters.internal_resistance_ohms = previous,
            _ => unreachable!(),
        }
    }
}

#[test]
fn rc_startup_matches_closed_form_references_at_50_and_60_hz() {
    let analytical = golden("stage-9-analytical.expected.json");
    for (case_id, frequency_hz, substeps) in
        [("rc-50-phase-0", 50.0, 2), ("rc-60-phase-0", 60.0, 4)]
    {
        let reference = analytical["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == case_id)
            .expect("the closed-form reference includes this case");
        let startup_tolerance = reference["summary"]["engineTolerance"]["startupSignalAbsolute"]
            .as_f64()
            .unwrap();
        let expected_by_step = reference["observations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|observation| {
                let time = observation["timeSeconds"].as_f64()?;
                let step = (time / 0.001).round() as u64;
                ((step > 0) && (step as f64 * 0.001 - time).abs() <= 1e-12).then(|| {
                    (
                        step,
                        observation["nodeVoltagesVolts"]["cap"].as_f64().unwrap(),
                    )
                })
            })
            .collect::<Vec<_>>();
        assert!(
            !expected_by_step.is_empty(),
            "{case_id} has public-step samples"
        );

        let (mut engine, run_id, layout, _, actual_substeps) =
            compile(source_request(frequency_hz, 0.0, 0.1, 100.0, Some(0.0001)));
        assert_eq!(actual_substeps, substeps);
        let mut expected_index = 0;
        let mut steady_state_samples = Vec::with_capacity(200);
        for step in 1..=999 {
            let snapshot = engine
                .advance(AdvanceRequest {
                    run_id: run_id.clone(),
                    step_count: 1,
                })
                .unwrap();
            if expected_by_step
                .get(expected_index)
                .is_some_and(|(expected_step, _)| *expected_step == step)
            {
                close(
                    value(&layout, &snapshot, CAPACITOR_ID, ResultQuantity::Voltage),
                    expected_by_step[expected_index].1,
                    startup_tolerance,
                    &format!("{case_id}, step {step}"),
                );
                expected_index += 1;
            }
            if (800..=999).contains(&step) {
                steady_state_samples.push(value(
                    &layout,
                    &snapshot,
                    CAPACITOR_ID,
                    ResultQuantity::Voltage,
                ));
            }
        }
        assert_eq!(expected_index, expected_by_step.len());
        assert_eq!(steady_state_samples.len(), 200);

        // Fit y = a sin(ωt) + b cos(ωt) over an integer number of cycles, then
        // compare the fitted peak and phase against the analytical steady-state reference.
        let (sin_coefficient, cos_coefficient) = steady_state_samples.iter().enumerate().fold(
            (0.0, 0.0),
            |(sin_sum, cos_sum), (index, sample)| {
                let time = (800 + index) as f64 * 0.001;
                let phase = std::f64::consts::TAU * frequency_hz * time;
                (
                    sin_sum + sample * phase.sin(),
                    cos_sum + sample * phase.cos(),
                )
            },
        );
        let amplitude = 2.0 / steady_state_samples.len() as f64;
        let peak = (sin_coefficient.hypot(cos_coefficient)) * amplitude;
        let phase_degrees = (cos_coefficient.atan2(sin_coefficient)).to_degrees();
        let summary = &reference["summary"];
        let tolerance = &summary["engineTolerance"];
        close(
            peak,
            summary["steadyStatePeak"].as_f64().unwrap(),
            summary["steadyStatePeak"].as_f64().unwrap()
                * tolerance["steadyStateAmplitudeRelative"].as_f64().unwrap(),
            &format!("{case_id} steady-state amplitude"),
        );
        close(
            phase_degrees,
            summary["steadyStatePhaseDegrees"].as_f64().unwrap(),
            tolerance["steadyStatePhaseDegrees"].as_f64().unwrap(),
            &format!("{case_id} steady-state phase"),
        );
    }
}

#[test]
fn phase_live_updates_restart_refusal_and_determinism_follow_the_contract() {
    let initial_request = source_request(50.0, 90.0, 10.0, 10.0, None);
    let (mut engine, run_id, layout, initial, substeps) = compile(initial_request.clone());
    assert_eq!(substeps, 2);
    close(
        value(&layout, &initial, SOURCE_ID, ResultQuantity::Voltage),
        2.0_f64.sqrt() * 10.0 / 2.0,
        1e-12,
        "phase offset sets the initial source value",
    );

    let mut snapshot = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 1,
        })
        .unwrap();
    let SetParameterResult::Success {
        snapshot: frequency_snapshot,
        internal_substeps,
        ..
    } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: snapshot.metadata.document_revision.clone(),
        document_revision: "ac-frequency-update".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 60.0,
    })
    else {
        panic!("the in-range frequency update should succeed");
    };
    assert_eq!(internal_substeps, 4);
    close(
        value(
            &layout,
            &frequency_snapshot,
            SOURCE_ID,
            ResultQuantity::Voltage,
        ),
        2.0_f64.sqrt()
            * 10.0
            * (std::f64::consts::TAU * 60.0 * 0.001 + std::f64::consts::FRAC_PI_2).sin()
            / 2.0,
        1e-11,
        "frequency changes preserve phase at the start of the recomputed step",
    );

    let SetParameterResult::Success {
        snapshot: phase_snapshot,
        ..
    } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: frequency_snapshot.metadata.document_revision.clone(),
        document_revision: "ac-phase-update".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::PhaseOffsetDegrees,
        value: 0.0,
    })
    else {
        panic!("the phase update should succeed");
    };
    close(
        value(&layout, &phase_snapshot, SOURCE_ID, ResultQuantity::Voltage),
        2.0_f64.sqrt() * 10.0 * (std::f64::consts::TAU * 60.0 * 0.001).sin() / 2.0,
        1e-11,
        "a live phase update applies the new offset at the current simulation time",
    );

    let SetParameterResult::Success {
        snapshot: amplitude_snapshot,
        ..
    } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: phase_snapshot.metadata.document_revision.clone(),
        document_revision: "ac-amplitude-update".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::VoltageRmsVolts,
        value: 20.0,
    })
    else {
        panic!("the in-range RMS voltage update should succeed");
    };
    close(
        value(
            &layout,
            &amplitude_snapshot,
            SOURCE_ID,
            ResultQuantity::Voltage,
        ),
        2.0 * value(&layout, &phase_snapshot, SOURCE_ID, ResultQuantity::Voltage),
        1e-11,
        "changing RMS voltage preserves the source phase",
    );

    let SetParameterResult::Success {
        snapshot: resistance_snapshot,
        ..
    } = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: amplitude_snapshot.metadata.document_revision.clone(),
        document_revision: "ac-resistance-update".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::InternalResistanceOhms,
        value: 5.0,
    })
    else {
        panic!("the non-negative internal resistance update should succeed");
    };
    close(
        value(
            &layout,
            &resistance_snapshot,
            SOURCE_ID,
            ResultQuantity::Voltage,
        ),
        2.0_f64.sqrt() * 20.0 * (std::f64::consts::TAU * 60.0 * 0.001).sin() * 10.0 / 15.0,
        1e-11,
        "changing source resistance preserves phase and applies its new divider ratio",
    );

    let refused = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: resistance_snapshot.metadata.document_revision.clone(),
        document_revision: "ac-refused-frequency".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 400.1,
    });
    assert!(matches!(refused, SetParameterResult::Failure { .. }));
    assert_eq!(engine.latest_snapshot(), Some(&resistance_snapshot));

    snapshot = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 1,
        })
        .unwrap();
    close(
        value(&layout, &snapshot, SOURCE_ID, ResultQuantity::Voltage),
        2.0_f64.sqrt() * 20.0 * (std::f64::consts::TAU * 60.0 * 0.002).sin() * 10.0 / 15.0,
        1e-11,
        "a refused frequency update leaves the next committed phase and parameters intact",
    );

    let CompileResult::Success {
        initial_snapshot: restarted,
        ..
    } = engine.compile(initial_request)
    else {
        panic!("recompiling the source should restart its phase");
    };
    close(
        value(&layout, &restarted, SOURCE_ID, ResultQuantity::Voltage),
        2.0_f64.sqrt() * 10.0 / 2.0,
        1e-12,
        "a compile resets accumulated phase to t=0",
    );

    let mut first = Engine::default();
    let mut second = Engine::default();
    let (first_run, first_layout) = match first.compile(source_request(60.0, 15.0, 3.0, 40.0, None))
    {
        CompileResult::Success { run_id, layout, .. } => (run_id, layout),
        _ => panic!("the deterministic source fixture should compile"),
    };
    let second_request = source_request(60.0, 15.0, 3.0, 40.0, None);
    let second_run = match second.compile(second_request) {
        CompileResult::Success { run_id, .. } => run_id,
        _ => panic!("the deterministic source fixture should compile"),
    };
    let first_midpoint = first
        .advance(AdvanceRequest {
            run_id: first_run.clone(),
            step_count: 25,
        })
        .unwrap();
    let second_midpoint = second
        .advance(AdvanceRequest {
            run_id: second_run.clone(),
            step_count: 25,
        })
        .unwrap();

    let SetParameterResult::Success {
        snapshot: first_updated,
        ..
    } = first.set_parameter(SetParameterRequest {
        run_id: first_run.clone(),
        expected_document_revision: first_midpoint.metadata.document_revision.clone(),
        document_revision: "ac-deterministic-frequency-update".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 57.0,
    })
    else {
        panic!("the deterministic frequency update should succeed");
    };
    let SetParameterResult::Success {
        snapshot: second_updated,
        ..
    } = second.set_parameter(SetParameterRequest {
        run_id: second_run.clone(),
        expected_document_revision: second_midpoint.metadata.document_revision.clone(),
        document_revision: "ac-deterministic-frequency-update".to_owned(),
        component_id: StableId::parse(SOURCE_ID).unwrap(),
        parameter_id: ParameterId::FrequencyHz,
        value: 57.0,
    })
    else {
        panic!("the deterministic frequency update should succeed");
    };
    assert_eq!(first_updated.values, second_updated.values);

    let first_snapshot = first
        .advance(AdvanceRequest {
            run_id: first_run,
            step_count: 25,
        })
        .unwrap();
    let second_snapshot = second
        .advance(AdvanceRequest {
            run_id: second_run,
            step_count: 25,
        })
        .unwrap();
    assert_eq!(
        first_snapshot
            .values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        second_snapshot
            .values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
    );
    assert_eq!(first_layout.value_count, first_snapshot.values.len() as u32);
}

#[test]
fn assignment_checks_refuse_stage_9_types() {
    let compile_request = source_request(50.0, 0.0, 0.1, 100.0, None);
    let request = CheckRequest {
        checks: vec![CheckCriterion::ComponentCount {
            id: StableId::parse("00000000-0000-4000-8000-000000000005").unwrap(),
            label: None,
            type_id: open_circuit_engine::ComponentTypeId::AcVoltageSource,
            min: 1,
            max: Some(1),
        }],
        locked_components: Vec::new(),
        starting_circuit: compile_request.schematic.clone(),
        schematic: compile_request.schematic,
        geometry: compile_request.geometry,
        document_revision: "ac-assignment-scope".to_owned(),
    };
    let result = evaluate_checks(request);

    assert_eq!(result.outcome, CheckOutcome::Failed);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::UnknownComponentType
    );
    assert!(matches!(
        &result.criteria[0],
        CriterionResult::ComponentCount {
            verdict: CheckVerdict::NotEvaluated,
            measured: None,
            ..
        }
    ));
}
