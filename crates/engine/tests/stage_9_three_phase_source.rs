use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentSpec, ComponentTypeId, DiagnosticCode,
    Engine, NumericDomain, ParameterId, ResultLayout, ResultSnapshot, SetParameterRequest,
    SetParameterResult, StableId, Unit, component_catalog,
};
use serde_json::{from_value, json};

const SOURCE_ID: &str = "00000000-0000-4000-8000-000000000001";
const LOAD_IDS: [&str; 3] = [
    "00000000-0000-4000-8000-000000000002",
    "00000000-0000-4000-8000-000000000003",
    "00000000-0000-4000-8000-000000000004",
];
const REFERENCE_ID: &str = "00000000-0000-4000-8000-000000000005";

struct Compiled {
    engine: Engine,
    run_id: StableId,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
    internal_substeps: u32,
}

fn star_request(
    load_resistances: [f64; 3],
    neutral_connected: bool,
    reversed_sequence: f64,
    internal_resistance_ohms: f64,
) -> CompileRequest {
    let source_pins = [
        ("l1", json!({"x": 0, "y": 2})),
        ("l2", json!({"x": 0, "y": 4})),
        ("l3", json!({"x": 0, "y": 6})),
        ("n", json!({"x": 4, "y": 4})),
    ];
    let load_neutral = if neutral_connected {
        json!({"x": 4, "y": 4})
    } else {
        json!({"x": 8, "y": 4})
    };
    let phases = [
        json!({"x": 0, "y": 2}),
        json!({"x": 0, "y": 4}),
        json!({"x": 0, "y": 6}),
    ];
    let mut components = vec![json!({
        "id": SOURCE_ID,
        "typeId": "three_phase_source_star",
        "position": { "x": 0, "y": 0 },
        "rotationDeg": 0,
        "parameters": {
            "phaseVoltageRmsVolts": 230.0,
            "frequencyHz": 50.0,
            "phaseOffsetDegrees": 0.0,
            "internalResistanceOhms": internal_resistance_ohms,
            "reversedSequence": reversed_sequence
        }
    })];
    let mut geometry = vec![json!({
        "componentId": SOURCE_ID,
        "pins": source_pins.iter().map(|(pin_id, position)| json!({
            "pinId": pin_id,
            "position": position
        })).collect::<Vec<_>>()
    })];
    for index in 0..3 {
        components.push(json!({
            "id": LOAD_IDS[index],
            "typeId": "resistor",
            "position": { "x": 4, "y": 4 },
            "rotationDeg": 0,
            "parameters": { "resistanceOhms": load_resistances[index] }
        }));
        geometry.push(json!({
            "componentId": LOAD_IDS[index],
            "pins": [
                { "pinId": "pin1", "position": phases[index] },
                { "pinId": "pin2", "position": load_neutral }
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
                { "id": REFERENCE_ID, "position": { "x": 4, "y": 4 } }
            ],
            "rails": []
        }))
        .unwrap(),
        geometry: from_value(json!({ "components": geometry })).unwrap(),
        document_revision: "three-phase-star".to_owned(),
        faults: None,
    }
}

fn delta_request(load_resistance_ohms: f64) -> CompileRequest {
    let line_points = [
        json!({"x": 0, "y": 2}),
        json!({"x": 0, "y": 4}),
        json!({"x": 0, "y": 6}),
    ];
    let mut components = vec![json!({
        "id": SOURCE_ID,
        "typeId": "three_phase_source_delta",
        "position": { "x": 0, "y": 0 },
        "rotationDeg": 0,
        "parameters": {
            "lineVoltageRmsVolts": 400.0,
            "frequencyHz": 50.0,
            "phaseOffsetDegrees": 0.0,
            "internalResistanceOhms": 0.1,
            "reversedSequence": 0.0
        }
    })];
    let mut geometry = vec![json!({
        "componentId": SOURCE_ID,
        "pins": [
            { "pinId": "l1", "position": line_points[0] },
            { "pinId": "l2", "position": line_points[1] },
            { "pinId": "l3", "position": line_points[2] }
        ]
    })];
    for index in 0..3 {
        let next = (index + 1) % 3;
        components.push(json!({
            "id": LOAD_IDS[index],
            "typeId": "resistor",
            "position": { "x": 0, "y": 0 },
            "rotationDeg": 0,
            "parameters": { "resistanceOhms": load_resistance_ohms }
        }));
        geometry.push(json!({
            "componentId": LOAD_IDS[index],
            "pins": [
                { "pinId": "pin1", "position": line_points[index] },
                { "pinId": "pin2", "position": line_points[next] }
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
                { "id": REFERENCE_ID, "position": line_points[0] }
            ],
            "rails": []
        }))
        .unwrap(),
        geometry: from_value(json!({ "components": geometry })).unwrap(),
        document_revision: "three-phase-delta".to_owned(),
        faults: None,
    }
}

fn compile(request: CompileRequest) -> Compiled {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        internal_substeps,
        ..
    } = engine.compile(request)
    else {
        panic!("three-phase source fixture should compile");
    };
    Compiled {
        engine,
        run_id,
        layout,
        snapshot: initial_snapshot,
        internal_substeps,
    }
}

fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, key: &str) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("result layout is missing {key}"));
    snapshot.values[slot.offset as usize]
}

fn advance(compiled: &mut Compiled, steps: u32) {
    compiled.snapshot = compiled
        .engine
        .advance(AdvanceRequest {
            run_id: compiled.run_id.clone(),
            step_count: steps,
        })
        .unwrap();
}

fn close(actual: f64, expected: f64, tolerance: f64, description: &str) {
    assert!(
        actual.is_finite() && (actual - expected).abs() <= tolerance,
        "{description}: {actual} differs from {expected} by more than {tolerance}"
    );
}

fn star_phase_sample(phase_degrees: f64, load_resistance_ohms: f64) -> f64 {
    let load_fraction = load_resistance_ohms / (load_resistance_ohms + 0.1);
    230.0 * std::f64::consts::SQRT_2 * load_fraction * phase_degrees.to_radians().sin()
}

fn catalog_parameter(
    entry: &open_circuit_engine::ComponentCatalogEntry,
    id: ParameterId,
) -> &open_circuit_engine::ParameterDefinition {
    entry
        .parameters
        .iter()
        .find(|parameter| parameter.id == id)
        .unwrap()
}

#[test]
fn three_phase_catalog_matches_the_star_and_delta_contract() {
    let catalog = component_catalog();
    let entry = |type_id| {
        catalog
            .component_types
            .iter()
            .find(|entry| entry.type_id == type_id)
            .unwrap()
    };
    let star = entry(ComponentTypeId::ThreePhaseSourceStar);
    assert_eq!(
        star.pins
            .iter()
            .map(|pin| pin.id.as_str())
            .collect::<Vec<_>>(),
        ["l1", "l2", "l3", "n"]
    );
    assert_eq!(star.fault_kinds, vec![open_circuit_engine::FaultKind::Leak]);
    assert_eq!(
        catalog_parameter(star, ParameterId::PhaseVoltageRmsVolts).default_value,
        230.0
    );
    assert_eq!(
        catalog_parameter(star, ParameterId::InternalResistanceOhms).domain,
        NumericDomain::NonNegative
    );
    assert_eq!(
        catalog_parameter(star, ParameterId::ReversedSequence).domain,
        NumericDomain::ZeroOrOne
    );

    let delta = entry(ComponentTypeId::ThreePhaseSourceDelta);
    assert_eq!(
        delta
            .pins
            .iter()
            .map(|pin| pin.id.as_str())
            .collect::<Vec<_>>(),
        ["l1", "l2", "l3"]
    );
    assert_eq!(
        delta.fault_kinds,
        vec![open_circuit_engine::FaultKind::Leak]
    );
    assert_eq!(
        catalog_parameter(delta, ParameterId::LineVoltageRmsVolts).default_value,
        400.0
    );
    assert_eq!(
        catalog_parameter(delta, ParameterId::LineVoltageRmsVolts).unit,
        Unit::Volt
    );
    assert_eq!(
        catalog_parameter(delta, ParameterId::InternalResistanceOhms).domain,
        NumericDomain::StrictlyPositive
    );
}

#[test]
fn star_source_reports_positive_and_reversed_phase_sequence() {
    let mut positive = compile(star_request([10_000.0; 3], true, 0.0, 0.1));
    let mut reversed = compile(star_request([10_000.0; 3], true, 1.0, 0.1));
    assert_eq!(positive.internal_substeps, 2);
    assert_eq!(reversed.internal_substeps, 2);
    advance(&mut positive, 1);
    advance(&mut reversed, 1);

    let expected_l1 = star_phase_sample(18.0, 10_000.0);
    let expected_l2 = star_phase_sample(-102.0, 10_000.0);
    let expected_l3 = star_phase_sample(138.0, 10_000.0);
    close(
        value(
            &positive.layout,
            &positive.snapshot,
            &format!("{SOURCE_ID}/l1/voltage"),
        ),
        expected_l1,
        1e-6,
        "positive-sequence l1 voltage",
    );
    close(
        value(
            &positive.layout,
            &positive.snapshot,
            &format!("{SOURCE_ID}/l2/voltage"),
        ),
        expected_l2,
        1e-6,
        "positive-sequence l2 voltage",
    );
    close(
        value(
            &positive.layout,
            &positive.snapshot,
            &format!("{SOURCE_ID}/l3/voltage"),
        ),
        expected_l3,
        1e-6,
        "positive-sequence l3 voltage",
    );
    close(
        value(
            &reversed.layout,
            &reversed.snapshot,
            &format!("{SOURCE_ID}/l2/voltage"),
        ),
        expected_l3,
        1e-6,
        "reversed-sequence l2 voltage",
    );
    close(
        value(
            &reversed.layout,
            &reversed.snapshot,
            &format!("{SOURCE_ID}/l3/voltage"),
        ),
        expected_l2,
        1e-6,
        "reversed-sequence l3 voltage",
    );

    let mut ideal = compile(star_request([10_000.0; 3], true, 0.0, 0.0));
    advance(&mut ideal, 1);
    close(
        value(
            &ideal.layout,
            &ideal.snapshot,
            &format!("{SOURCE_ID}/l1/voltage"),
        ),
        230.0 * std::f64::consts::SQRT_2 * 18.0_f64.to_radians().sin(),
        1e-6,
        "ideal star winding voltage",
    );
    close(
        value(
            &ideal.layout,
            &ideal.snapshot,
            &format!("{SOURCE_ID}/l1/current"),
        ),
        -230.0 * std::f64::consts::SQRT_2 * 18.0_f64.to_radians().sin() / 10_000.0,
        1e-9,
        "ideal star winding branch current",
    );
}

#[test]
fn star_source_matches_unbalanced_loads_with_and_without_neutral() {
    let loads = [100.0, 220.0, 470.0];
    let phase_degrees: [f64; 3] = [18.0, -102.0, 138.0];
    let source_voltages =
        phase_degrees.map(|phase| 230.0 * std::f64::consts::SQRT_2 * phase.to_radians().sin());

    let mut connected = compile(star_request(loads, true, 0.0, 0.1));
    advance(&mut connected, 1);
    let mut expected_connected_currents = [0.0; 3];
    for index in 0..3 {
        expected_connected_currents[index] = -source_voltages[index] / (loads[index] + 0.1);
        close(
            value(
                &connected.layout,
                &connected.snapshot,
                &format!("{SOURCE_ID}/l{}/current", index + 1),
            ),
            expected_connected_currents[index],
            1e-7,
            "connected-neutral line current",
        );
    }
    close(
        value(
            &connected.layout,
            &connected.snapshot,
            &format!("{SOURCE_ID}/n/current"),
        ),
        -expected_connected_currents.iter().sum::<f64>(),
        1e-7,
        "connected-neutral current",
    );
    close(
        (0..3)
            .map(|index| {
                value(
                    &connected.layout,
                    &connected.snapshot,
                    &format!("{SOURCE_ID}/l{}/current", index + 1),
                )
            })
            .sum::<f64>()
            + value(
                &connected.layout,
                &connected.snapshot,
                &format!("{SOURCE_ID}/n/current"),
            ),
        0.0,
        1e-9,
        "star source terminal-current sum",
    );

    let mut floating = compile(star_request(loads, false, 0.0, 0.1));
    advance(&mut floating, 1);
    let total_conductance = loads.iter().map(|load| 1.0 / (load + 0.1)).sum::<f64>();
    let neutral_voltage = source_voltages
        .iter()
        .zip(loads)
        .map(|(voltage, load)| voltage / (load + 0.1))
        .sum::<f64>()
        / total_conductance;
    let mut expected_floating_currents = [0.0; 3];
    for index in 0..3 {
        expected_floating_currents[index] =
            (neutral_voltage - source_voltages[index]) / (loads[index] + 0.1);
        close(
            value(
                &floating.layout,
                &floating.snapshot,
                &format!("{SOURCE_ID}/l{}/current", index + 1),
            ),
            expected_floating_currents[index],
            1e-7,
            "floating-neutral line current",
        );
    }
    close(
        value(
            &floating.layout,
            &floating.snapshot,
            &format!("{SOURCE_ID}/n/current"),
        ),
        0.0,
        1e-7,
        "disconnected source neutral current",
    );
    close(
        expected_floating_currents.iter().sum(),
        0.0,
        1e-9,
        "floating load neutral current balance",
    );
}

#[test]
fn delta_source_reports_line_voltages_and_balanced_terminal_currents() {
    let load_resistance_ohms = 10_000.0;
    let mut compiled = compile(delta_request(load_resistance_ohms));
    advance(&mut compiled, 5);
    let voltage_peak =
        400.0 * std::f64::consts::SQRT_2 * load_resistance_ohms / (load_resistance_ohms + 0.1);
    close(
        value(
            &compiled.layout,
            &compiled.snapshot,
            &format!("{SOURCE_ID}/l12/voltage"),
        ),
        voltage_peak,
        1e-5,
        "delta l12 voltage",
    );
    close(
        value(
            &compiled.layout,
            &compiled.snapshot,
            &format!("{SOURCE_ID}/l23/voltage"),
        ),
        -0.5 * voltage_peak,
        1e-5,
        "delta l23 voltage",
    );
    close(
        value(
            &compiled.layout,
            &compiled.snapshot,
            &format!("{SOURCE_ID}/l31/voltage"),
        ),
        -0.5 * voltage_peak,
        1e-5,
        "delta l31 voltage",
    );
    let terminal_current_sum = (1..=3)
        .map(|line| {
            value(
                &compiled.layout,
                &compiled.snapshot,
                &format!("{SOURCE_ID}/l{line}/current"),
            )
        })
        .sum::<f64>();
    close(
        terminal_current_sum,
        0.0,
        1e-9,
        "delta source terminal-current sum",
    );
    close(
        value(
            &compiled.layout,
            &compiled.snapshot,
            &format!("{SOURCE_ID}/l1/current"),
        ),
        -1.5 * voltage_peak / load_resistance_ohms,
        1e-7,
        "delta l1 terminal current",
    );

    let mut reversed_request = delta_request(load_resistance_ohms);
    let ComponentSpec::ThreePhaseSourceDelta { parameters, .. } =
        &mut reversed_request.schematic.components[0]
    else {
        unreachable!();
    };
    parameters.reversed_sequence = 1.0;
    let mut reversed = compile(reversed_request);
    advance(&mut reversed, 1);
    let line_voltage_peak =
        400.0 * std::f64::consts::SQRT_2 * load_resistance_ohms / (load_resistance_ohms + 0.1);
    close(
        value(
            &reversed.layout,
            &reversed.snapshot,
            &format!("{SOURCE_ID}/l23/voltage"),
        ),
        line_voltage_peak * 138.0_f64.to_radians().sin(),
        1e-5,
        "reversed delta phase sequence",
    );
}

#[test]
fn invalid_delta_resistance_and_ideal_phase_short_are_diagnosed() {
    let mut delta = delta_request(10_000.0);
    let ComponentSpec::ThreePhaseSourceDelta { parameters, .. } =
        &mut delta.schematic.components[0]
    else {
        unreachable!();
    };
    parameters.internal_resistance_ohms = 0.0;
    let mut engine = Engine::default();
    let CompileResult::Failure { diagnostics } = engine.compile(delta) else {
        panic!("a zero-resistance delta source must be rejected");
    };
    assert!(diagnostics.iter().any(|item| {
        item.code == DiagnosticCode::InvalidParameter
            && item.references.iter().any(|reference| {
                matches!(reference, open_circuit_engine::DiagnosticReference::Component { component_id } if component_id.as_str() == SOURCE_ID)
            })
    }));

    let mut fractional_sequence = star_request([10_000.0; 3], true, 0.0, 0.1);
    let ComponentSpec::ThreePhaseSourceStar { parameters, .. } =
        &mut fractional_sequence.schematic.components[0]
    else {
        unreachable!();
    };
    parameters.reversed_sequence = 0.5;
    let mut engine = Engine::default();
    let CompileResult::Failure { diagnostics } = engine.compile(fractional_sequence) else {
        panic!("a non-binary phase sequence must be rejected");
    };
    assert!(diagnostics.iter().any(|item| {
        item.code == DiagnosticCode::InvalidParameter
            && item.references.iter().any(|reference| {
                matches!(reference, open_circuit_engine::DiagnosticReference::Component { component_id } if component_id.as_str() == SOURCE_ID)
            })
    }));

    let mut star = star_request([10_000.0; 3], true, 0.0, 0.0);
    let ComponentSpec::ThreePhaseSourceStar { parameters, .. } = &mut star.schematic.components[0]
    else {
        unreachable!();
    };
    parameters.internal_resistance_ohms = 0.0;
    let source_geometry = star
        .geometry
        .components
        .iter_mut()
        .find(|component| component.component_id.as_str() == SOURCE_ID)
        .unwrap();
    let l1 = source_geometry
        .pins
        .iter()
        .find(|pin| pin.pin_id == "l1")
        .unwrap()
        .position;
    source_geometry
        .pins
        .iter_mut()
        .find(|pin| pin.pin_id == "l2")
        .unwrap()
        .position = l1;
    let mut engine = Engine::default();
    let CompileResult::Failure { diagnostics } = engine.compile(star) else {
        panic!("shorting two ideal star-source phases must be rejected");
    };
    assert!(diagnostics.iter().any(|item| {
        item.code == DiagnosticCode::ShortCircuit
            && item.references.iter().any(|reference| {
                matches!(reference, open_circuit_engine::DiagnosticReference::Component { component_id } if component_id.as_str() == SOURCE_ID)
            })
    }));
}

#[test]
fn star_source_reversed_sequence_updates_live() {
    let mut compiled = compile(star_request([10_000.0; 3], true, 0.0, 0.1));
    advance(&mut compiled, 1);
    let before_l2 = value(
        &compiled.layout,
        &compiled.snapshot,
        &format!("{SOURCE_ID}/l2/voltage"),
    );
    let before_l3 = value(
        &compiled.layout,
        &compiled.snapshot,
        &format!("{SOURCE_ID}/l3/voltage"),
    );
    let SetParameterResult::Success { snapshot, .. } =
        compiled.engine.set_parameter(SetParameterRequest {
            run_id: compiled.run_id.clone(),
            expected_document_revision: compiled.snapshot.metadata.document_revision.clone(),
            document_revision: "three-phase-sequence-update".to_owned(),
            component_id: StableId::parse(SOURCE_ID).unwrap(),
            parameter_id: ParameterId::ReversedSequence,
            value: 1.0,
        })
    else {
        panic!("changing the phase sequence should succeed");
    };
    close(
        value(
            &compiled.layout,
            &snapshot,
            &format!("{SOURCE_ID}/l2/voltage"),
        ),
        before_l3,
        1e-6,
        "live l2/l3 phase swap",
    );
    close(
        value(
            &compiled.layout,
            &snapshot,
            &format!("{SOURCE_ID}/l3/voltage"),
        ),
        before_l2,
        1e-6,
        "live l3/l2 phase swap",
    );
}
