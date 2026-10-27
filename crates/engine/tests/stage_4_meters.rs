use open_circuit_engine::{
    CompileRequest, CompileResult, ComponentGeometry, ComponentSpec, DcVoltageSourceParameters,
    DiagnosticCode, Engine, GeometryEnvelope, GridPoint, ReferenceMarkerSpec, ResistorParameters,
    ResolvedPinPosition, ResultLayout, ResultQuantity, ResultSignConvention, ResultSnapshot,
    SCHEMA_VERSION, Schematic, StableId, Unit,
};
use serde_json::Value;

const EXPECTED: &str = include_str!("../../../tests/golden/stage-4-meters-loading.expected.json");
const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const METER: &str = "00000000-0000-4000-8000-000000000002";
const FIRST_LOAD: &str = "00000000-0000-4000-8000-000000000003";
const SECOND_LOAD: &str = "00000000-0000-4000-8000-000000000004";
const REFERENCE: &str = "00000000-0000-4000-8000-000000000005";

#[derive(Clone, Copy)]
enum MeterKind {
    Voltmeter,
    Ammeter,
}

fn id(value: &str) -> StableId {
    StableId::parse(value).unwrap()
}

fn point(x: i32, y: i32) -> GridPoint {
    GridPoint { x, y }
}

fn pin(pin_id: &str, position: GridPoint) -> ResolvedPinPosition {
    ResolvedPinPosition {
        pin_id: pin_id.to_owned(),
        position,
    }
}

fn fixture(
    kind: MeterKind,
    source_internal_resistance_ohms: f64,
    load_resistances_ohms: &[f64],
    reverse_meter: bool,
) -> (Schematic, GeometryEnvelope) {
    let high = point(2, 0);
    let after_meter = point(4, 0);
    let ground = point(0, 0);
    let mut components = vec![ComponentSpec::DcVoltageSource {
        id: id(SOURCE),
        position: point(0, 2),
        rotation_deg: 0,
        parameters: DcVoltageSourceParameters {
            voltage_volts: 10.0,
            internal_resistance_ohms: source_internal_resistance_ohms,
        },
    }];
    components.push(match kind {
        MeterKind::Voltmeter => ComponentSpec::Voltmeter {
            id: id(METER),
            position: point(2, 2),
            rotation_deg: 0,
            parameters: open_circuit_engine::VoltmeterParameters {
                input_resistance_ohms: 10_000_000.0,
            },
        },
        MeterKind::Ammeter => ComponentSpec::Ammeter {
            id: id(METER),
            position: point(4, 2),
            rotation_deg: 0,
            parameters: open_circuit_engine::AmmeterParameters {
                burden_resistance_ohms: 1.0,
            },
        },
    });
    for (index, resistance_ohms) in load_resistances_ohms.iter().copied().enumerate() {
        components.push(ComponentSpec::Resistor {
            id: id(if index == 0 { FIRST_LOAD } else { SECOND_LOAD }),
            position: point(6 + index as i32 * 2, 2),
            rotation_deg: 0,
            parameters: ResistorParameters { resistance_ohms },
        });
    }

    let mut geometries = vec![
        ComponentGeometry {
            component_id: id(SOURCE),
            pins: vec![pin("pin1", high), pin("pin2", ground)],
        },
        ComponentGeometry {
            component_id: id(METER),
            pins: match (kind, reverse_meter) {
                (MeterKind::Voltmeter, false) => {
                    vec![pin("pin1", high), pin("pin2", ground)]
                }
                (MeterKind::Voltmeter, true) => {
                    vec![pin("pin1", ground), pin("pin2", high)]
                }
                (MeterKind::Ammeter, false) => {
                    vec![pin("pin1", high), pin("pin2", after_meter)]
                }
                (MeterKind::Ammeter, true) => {
                    vec![pin("pin1", after_meter), pin("pin2", high)]
                }
            },
        },
    ];
    for (index, _) in load_resistances_ohms.iter().enumerate() {
        let (load_pin1, load_pin2) = match kind {
            MeterKind::Voltmeter => (high, ground),
            MeterKind::Ammeter => (after_meter, ground),
        };
        geometries.push(ComponentGeometry {
            component_id: id(if index == 0 { FIRST_LOAD } else { SECOND_LOAD }),
            pins: vec![pin("pin1", load_pin1), pin("pin2", load_pin2)],
        });
    }

    (
        Schematic {
            schema_version: SCHEMA_VERSION,
            components,
            wires: Vec::new(),
            junctions: Vec::new(),
            reference_markers: vec![ReferenceMarkerSpec {
                id: id(REFERENCE),
                position: ground,
            }],
        },
        GeometryEnvelope {
            components: geometries,
        },
    )
}

fn compile(
    kind: MeterKind,
    source_internal_resistance_ohms: f64,
    load_resistances_ohms: &[f64],
    reverse_meter: bool,
) -> (Engine, ResultLayout, ResultSnapshot) {
    let (schematic, geometry) = fixture(
        kind,
        source_internal_resistance_ohms,
        load_resistances_ohms,
        reverse_meter,
    );
    let mut engine = Engine::default();
    let CompileResult::Success {
        layout,
        initial_snapshot,
        diagnostics,
        ..
    } = engine.compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "meters-test-1".to_owned(),
        faults: None,
    })
    else {
        panic!("the meter circuit should compile");
    };
    assert!(!diagnostics.iter().any(|item| {
        item.severity == open_circuit_engine::DiagnosticSeverity::Error
            || item.code == DiagnosticCode::NoGroundReference
    }));
    (engine, layout, initial_snapshot)
}

fn value(
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
    component: &str,
    quantity: ResultQuantity,
) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.component_id.as_str() == component && slot.quantity == quantity)
        .expect("the requested meter result slot exists");
    snapshot.values[slot.offset as usize]
}

#[test]
fn voltmeter_loading_matches_the_canonical_divider_reference_and_layout() {
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let reference = &expected["voltmeterCircuit"];
    let (_, layout, snapshot) = compile(
        MeterKind::Voltmeter,
        reference["sourceInternalResistanceOhms"].as_f64().unwrap(),
        &[],
        false,
    );
    let slots = layout
        .slots
        .iter()
        .filter(|slot| slot.component_id.as_str() == METER)
        .collect::<Vec<_>>();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].quantity, ResultQuantity::Voltage);
    assert_eq!(slots[0].unit, Unit::Volt);
    assert_eq!(
        slots[0].sign_convention,
        ResultSignConvention::VoltagePin1MinusPin2
    );
    assert_eq!(slots[1].quantity, ResultQuantity::Current);
    assert_eq!(slots[1].unit, Unit::Amp);
    assert_eq!(
        slots[1].sign_convention,
        ResultSignConvention::CurrentIntoPin1
    );

    let reading = value(&layout, &snapshot, METER, ResultQuantity::Voltage);
    let current = value(&layout, &snapshot, METER, ResultQuantity::Current);
    assert!(
        (reading - reference["expectedReadingVolts"].as_f64().unwrap()).abs()
            <= reference["readingToleranceVolts"].as_f64().unwrap()
    );
    assert!(
        (current - reference["expectedInputCurrentAmps"].as_f64().unwrap()).abs()
            <= reference["currentToleranceAmps"].as_f64().unwrap()
    );
    assert!(current > 0.0);
}

#[test]
fn ammeter_loading_matches_the_canonical_burden_reference_and_sign() {
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let reference = &expected["ammeterCircuit"];
    let (_, layout, snapshot) = compile(
        MeterKind::Ammeter,
        0.0,
        &[reference["loadResistanceOhms"].as_f64().unwrap()],
        false,
    );
    let reading = value(&layout, &snapshot, METER, ResultQuantity::Current);
    let burden = value(&layout, &snapshot, METER, ResultQuantity::Voltage);
    assert!(
        (reading - reference["expectedReadingAmps"].as_f64().unwrap()).abs()
            <= reference["readingToleranceAmps"].as_f64().unwrap()
    );
    assert!(
        (burden - reference["expectedBurdenVoltageVolts"].as_f64().unwrap()).abs()
            <= reference["voltageToleranceVolts"].as_f64().unwrap()
    );
    assert!(reading > 0.0 && burden > 0.0);
}

#[test]
fn reversed_meter_pins_reverse_the_standard_voltage_and_current_signs() {
    let (_, voltmeter_layout, voltmeter_snapshot) =
        compile(MeterKind::Voltmeter, 1_000_000.0, &[], true);
    assert!(
        value(
            &voltmeter_layout,
            &voltmeter_snapshot,
            METER,
            ResultQuantity::Voltage
        ) < 0.0
    );
    assert!(
        value(
            &voltmeter_layout,
            &voltmeter_snapshot,
            METER,
            ResultQuantity::Current
        ) < 0.0
    );

    let (_, ammeter_layout, ammeter_snapshot) = compile(MeterKind::Ammeter, 0.0, &[10.0], true);
    assert!(
        value(
            &ammeter_layout,
            &ammeter_snapshot,
            METER,
            ResultQuantity::Current
        ) < 0.0
    );
    assert!(
        value(
            &ammeter_layout,
            &ammeter_snapshot,
            METER,
            ResultQuantity::Voltage
        ) < 0.0
    );
}

#[test]
fn meters_load_parallel_branches_without_changing_the_reference_net() {
    let (_, voltmeter_layout, voltmeter_snapshot) =
        compile(MeterKind::Voltmeter, 1_000_000.0, &[10_000_000.0], false);
    assert!(
        (value(
            &voltmeter_layout,
            &voltmeter_snapshot,
            METER,
            ResultQuantity::Voltage
        ) - (10.0 * 5.0 / 6.0))
            .abs()
            < 1e-9
    );
    assert_eq!(
        value(
            &voltmeter_layout,
            &voltmeter_snapshot,
            FIRST_LOAD,
            ResultQuantity::Voltage
        ),
        value(
            &voltmeter_layout,
            &voltmeter_snapshot,
            METER,
            ResultQuantity::Voltage
        )
    );

    let (_, ammeter_layout, ammeter_snapshot) =
        compile(MeterKind::Ammeter, 0.0, &[20.0, 20.0], false);
    let total_current = value(
        &ammeter_layout,
        &ammeter_snapshot,
        METER,
        ResultQuantity::Current,
    );
    assert!((total_current - 10.0 / 11.0).abs() < 1e-9);
    assert!(
        (value(
            &ammeter_layout,
            &ammeter_snapshot,
            FIRST_LOAD,
            ResultQuantity::Current
        ) - total_current / 2.0)
            .abs()
            < 1e-9
    );
    assert!(
        (value(
            &ammeter_layout,
            &ammeter_snapshot,
            SECOND_LOAD,
            ResultQuantity::Current
        ) - total_current / 2.0)
            .abs()
            < 1e-9
    );
}

#[test]
fn meter_resistances_must_be_strictly_positive() {
    let (mut schematic, _) = fixture(MeterKind::Voltmeter, 1_000_000.0, &[], false);
    let ComponentSpec::Voltmeter { parameters, .. } = &mut schematic.components[1] else {
        unreachable!();
    };
    parameters.input_resistance_ohms = 0.0;
    assert!(
        schematic
            .validate()
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidParameter)
    );

    let (mut schematic, _) = fixture(MeterKind::Ammeter, 0.0, &[10.0], false);
    let ComponentSpec::Ammeter { parameters, .. } = &mut schematic.components[1] else {
        unreachable!();
    };
    parameters.burden_resistance_ohms = 0.0;
    assert!(
        schematic
            .validate()
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidParameter)
    );
}
