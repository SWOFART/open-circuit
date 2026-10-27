use open_circuit_engine::schematic::CapacitorParameters;
use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentGeometry, ComponentSpec,
    DcVoltageSourceParameters, DeviceState, Engine, GeometryEnvelope, GridPoint, ParameterId,
    ReferenceMarkerSpec, RelayContactState, RelayParameters, ResistorParameters,
    ResolvedPinPosition, ResultLayout, ResultSnapshot, SCHEMA_VERSION, Schematic,
    SetParameterRequest, SetParameterResult, StableId,
};
use serde_json::Value;

const EXPECTED: &str =
    include_str!("../../../tests/golden/stage-4-relay-self-latching.expected.json");

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const START_RESISTOR: &str = "00000000-0000-4000-8000-000000000002";
const RELAY: &str = "00000000-0000-4000-8000-000000000003";

fn id(value: &str) -> StableId {
    StableId::parse(value).unwrap()
}

fn fixture() -> (Schematic, GeometryEnvelope) {
    let point = |x, y| GridPoint { x, y };
    let schematic = Schematic {
        schema_version: SCHEMA_VERSION,
        components: vec![
            ComponentSpec::DcVoltageSource {
                id: id(SOURCE),
                position: point(0, 0),
                rotation_deg: 0,
                parameters: DcVoltageSourceParameters {
                    voltage_volts: 5.0,
                    internal_resistance_ohms: 0.0,
                },
            },
            ComponentSpec::Resistor {
                id: id(START_RESISTOR),
                position: point(4, 0),
                rotation_deg: 0,
                parameters: ResistorParameters {
                    resistance_ohms: 1.0,
                },
            },
            ComponentSpec::RelaySpdt {
                id: id(RELAY),
                position: point(8, 0),
                rotation_deg: 0,
                parameters: RelayParameters {
                    coil_resistance_ohms: 100.0,
                    coil_inductance_henries: 0.01,
                    pickup_current_amps: 0.04,
                    dropout_current_amps: 0.02,
                    closed_contact_resistance_ohms: 0.1,
                },
            },
        ],
        wires: Vec::new(),
        junctions: Vec::new(),
        reference_markers: vec![ReferenceMarkerSpec {
            id: id("00000000-0000-4000-8000-000000000004"),
            position: point(2, 0),
        }],
    };
    let geometry = GeometryEnvelope {
        components: vec![
            ComponentGeometry {
                component_id: id(SOURCE),
                pins: vec![pin("pin1", point(0, 0)), pin("pin2", point(2, 0))],
            },
            ComponentGeometry {
                component_id: id(START_RESISTOR),
                pins: vec![pin("pin1", point(4, 0)), pin("pin2", point(2, 0))],
            },
            ComponentGeometry {
                component_id: id(RELAY),
                pins: vec![
                    pin("coil_positive", point(0, 0)),
                    pin("coil_negative", point(4, 0)),
                    pin("common", point(4, 0)),
                    pin("normally_open", point(2, 0)),
                    pin("normally_closed", point(6, 0)),
                ],
            },
        ],
    };
    (schematic, geometry)
}

fn pin(pin_id: &str, position: GridPoint) -> ResolvedPinPosition {
    ResolvedPinPosition {
        pin_id: pin_id.to_owned(),
        position,
    }
}

fn compile(engine: &mut Engine) -> (StableId, ResultLayout, ResultSnapshot) {
    let (schematic, geometry) = fixture();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = engine.compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "relay-test-1".to_owned(),
        faults: None,
    })
    else {
        panic!("relay fixture should compile");
    };
    (run_id, layout, initial_snapshot)
}

fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, key: &str) -> f64 {
    let slot = layout.slots.iter().find(|slot| slot.key == key).unwrap();
    snapshot.values[slot.offset as usize]
}

fn relay_state(snapshot: &ResultSnapshot) -> (RelayContactState, Option<f64>) {
    let [
        DeviceState::Relay {
            state,
            last_transition_time_seconds,
            ..
        },
    ] = snapshot.states.as_slice()
    else {
        panic!("the snapshot contains exactly one relay state");
    };
    (*state, *last_transition_time_seconds)
}

#[test]
fn relay_picks_at_initialization_holds_on_the_no_contact_and_drops_out() {
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let mut engine = Engine::default();
    let (run_id, layout, mut snapshot) = compile(&mut engine);
    assert_eq!(expected["expected"][0]["contactState"], "picked");
    assert_eq!(
        relay_state(&snapshot),
        (RelayContactState::Picked, Some(0.0))
    );

    let coil_current = value(&layout, &snapshot, &format!("{RELAY}/coil/current"));
    assert!((coil_current - 5.0 / 101.0).abs() < 1e-8);

    snapshot = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 2,
        })
        .unwrap();
    assert_eq!(snapshot.metadata.simulation_time_seconds, 0.002);
    assert_eq!(relay_state(&snapshot).0, RelayContactState::Picked);

    let update = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "relay-test-1".to_owned(),
        document_revision: "relay-test-2".to_owned(),
        component_id: id(START_RESISTOR),
        parameter_id: ParameterId::ResistanceOhms,
        value: 1e12,
    });
    let SetParameterResult::Success { .. } = update else {
        panic!("raising the trigger resistance should be accepted");
    };
    snapshot = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 3,
        })
        .unwrap();
    assert_eq!(snapshot.metadata.simulation_time_seconds, 0.005);
    assert_eq!(relay_state(&snapshot).0, RelayContactState::Picked);
    let current = value(&layout, &snapshot, &format!("{RELAY}/coil/current"));
    let expected_current = expected["expected"][1]["coilCurrentAmps"].as_f64().unwrap();
    let tolerance = expected["expected"][1]["coilCurrentToleranceAmps"]
        .as_f64()
        .unwrap();
    assert!((current - expected_current).abs() <= tolerance);
    assert!(
        value(
            &layout,
            &snapshot,
            &format!("{RELAY}/normally_open/current")
        ) > 0.049
    );
    assert!(
        value(
            &layout,
            &snapshot,
            &format!("{RELAY}/normally_closed/current")
        )
        .abs()
            < 1e-12
    );

    let total_power = [SOURCE, START_RESISTOR]
        .iter()
        .map(|component| {
            value(&layout, &snapshot, &format!("{component}:voltage"))
                * value(&layout, &snapshot, &format!("{component}:current"))
        })
        .sum::<f64>()
        + value(&layout, &snapshot, &format!("{RELAY}/coil/voltage")) * current
        + value(
            &layout,
            &snapshot,
            &format!("{RELAY}/normally_open/voltage"),
        ) * value(
            &layout,
            &snapshot,
            &format!("{RELAY}/normally_open/current"),
        );
    assert!(
        total_power.abs() < 1e-5,
        "signed device power sums to {total_power} W"
    );

    snapshot = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 1,
        })
        .unwrap();
    assert_eq!(snapshot.metadata.simulation_time_seconds, 0.006);

    let update = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "relay-test-2".to_owned(),
        document_revision: "relay-test-3".to_owned(),
        component_id: id(SOURCE),
        parameter_id: ParameterId::VoltageVolts,
        value: 0.0,
    });
    let SetParameterResult::Success { .. } = update else {
        panic!("removing the supply should be accepted");
    };
    snapshot = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1,
        })
        .unwrap();
    assert_eq!(snapshot.metadata.simulation_time_seconds, 0.007);
    let (state, transition_time) = relay_state(&snapshot);
    assert_eq!(state, RelayContactState::Released);
    assert_eq!(expected["expected"][2]["contactState"], "released");
    assert!(transition_time.unwrap() >= 0.005);
    assert!(transition_time.unwrap() <= 0.006);
    assert!(
        value(
            &layout,
            &snapshot,
            &format!("{RELAY}/normally_open/current")
        )
        .abs()
            < 1e-9
    );
}

#[test]
fn relay_remains_released_below_pickup_and_uses_the_nc_contact() {
    let (mut schematic, mut geometry) = fixture();
    let ComponentSpec::DcVoltageSource { parameters, .. } = &mut schematic.components[0] else {
        unreachable!();
    };
    parameters.voltage_volts = 1.0;
    let relay_geometry = geometry
        .components
        .iter_mut()
        .find(|entry| entry.component_id.as_str() == RELAY)
        .unwrap();
    relay_geometry
        .pins
        .iter_mut()
        .find(|pin| pin.pin_id == "normally_closed")
        .unwrap()
        .position = GridPoint { x: 2, y: 0 };
    let CompileResult::Success {
        initial_snapshot,
        layout,
        ..
    } = Engine::default().compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "relay-nc".to_owned(),
        faults: None,
    })
    else {
        panic!("below-pickup relay fixture should compile");
    };
    assert_eq!(
        relay_state(&initial_snapshot),
        (RelayContactState::Released, None)
    );
    assert!(
        value(
            &layout,
            &initial_snapshot,
            &format!("{RELAY}/normally_closed/current")
        ) > 0.0
    );
    assert!(
        value(
            &layout,
            &initial_snapshot,
            &format!("{RELAY}/normally_open/current")
        )
        .abs()
            < 1e-12
    );
}

#[test]
fn disconnected_coil_and_contact_ports_compile_as_separate_matrix_blocks() {
    let (schematic, mut geometry) = fixture();
    let relay_geometry = geometry
        .components
        .iter_mut()
        .find(|entry| entry.component_id.as_str() == RELAY)
        .unwrap();
    for (pin_id, position) in [
        ("common", GridPoint { x: 8, y: 8 }),
        ("normally_open", GridPoint { x: 10, y: 8 }),
        ("normally_closed", GridPoint { x: 12, y: 8 }),
    ] {
        relay_geometry
            .pins
            .iter_mut()
            .find(|pin| pin.pin_id == pin_id)
            .unwrap()
            .position = position;
    }

    let CompileResult::Success {
        initial_snapshot,
        layout,
        ..
    } = Engine::default().compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "isolated-relay-ports".to_owned(),
        faults: None,
    })
    else {
        panic!("galvanically separate relay ports should compile independently");
    };
    assert_eq!(relay_state(&initial_snapshot).0, RelayContactState::Picked);
    assert!(
        initial_snapshot
            .values
            .iter()
            .all(|value| value.is_finite())
    );
    assert!(
        value(
            &layout,
            &initial_snapshot,
            &format!("{RELAY}/normally_closed/voltage")
        )
        .is_finite()
    );
}

#[test]
fn explicit_capacitor_initial_state_does_not_ground_an_unconnected_relay_coil_terminal() {
    let (mut schematic, mut geometry) = fixture();
    schematic
        .components
        .retain(|component| component.id().as_str() != START_RESISTOR);
    geometry
        .components
        .retain(|component| component.component_id.as_str() != START_RESISTOR);
    let ComponentSpec::RelaySpdt { .. } = &schematic.components[1] else {
        unreachable!();
    };
    schematic.components.push(ComponentSpec::Capacitor {
        id: id("00000000-0000-4000-8000-000000000005"),
        position: GridPoint { x: 20, y: 20 },
        rotation_deg: 0,
        parameters: CapacitorParameters {
            capacitance_farads: 1e-5,
            initial_voltage_volts: Some(1.0),
        },
    });
    geometry.components.push(ComponentGeometry {
        component_id: id("00000000-0000-4000-8000-000000000005"),
        pins: vec![
            pin("pin1", GridPoint { x: 20, y: 20 }),
            pin("pin2", GridPoint { x: 22, y: 20 }),
        ],
    });
    let relay_geometry = geometry
        .components
        .iter_mut()
        .find(|entry| entry.component_id.as_str() == RELAY)
        .unwrap();
    for (pin_id, position) in [
        ("common", GridPoint { x: 8, y: 8 }),
        ("normally_open", GridPoint { x: 10, y: 8 }),
        ("normally_closed", GridPoint { x: 12, y: 8 }),
    ] {
        relay_geometry
            .pins
            .iter_mut()
            .find(|pin| pin.pin_id == pin_id)
            .unwrap()
            .position = position;
    }

    let CompileResult::Success {
        initial_snapshot,
        layout,
        ..
    } = Engine::default().compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "explicit-capacitor-state".to_owned(),
        faults: None,
    })
    else {
        panic!("the explicit-capacitor initial-state circuit should compile");
    };
    assert_eq!(
        relay_state(&initial_snapshot),
        (RelayContactState::Released, None)
    );
    assert!(value(&layout, &initial_snapshot, &format!("{RELAY}/coil/current")).abs() < 1e-12);
}

#[test]
fn an_open_relay_coil_terminal_stays_floating_during_transient_steps() {
    let (mut schematic, mut geometry) = fixture();
    schematic
        .components
        .retain(|component| component.id().as_str() != START_RESISTOR);
    geometry
        .components
        .retain(|component| component.component_id.as_str() != START_RESISTOR);
    let relay_geometry = geometry
        .components
        .iter_mut()
        .find(|entry| entry.component_id.as_str() == RELAY)
        .unwrap();
    for (pin_id, position) in [
        ("common", GridPoint { x: 8, y: 8 }),
        ("normally_open", GridPoint { x: 10, y: 8 }),
        ("normally_closed", GridPoint { x: 12, y: 8 }),
    ] {
        relay_geometry
            .pins
            .iter_mut()
            .find(|pin| pin.pin_id == pin_id)
            .unwrap()
            .position = position;
    }

    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = engine.compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "open-coil-transient".to_owned(),
        faults: None,
    })
    else {
        panic!("an open relay coil should compile without a false return path");
    };
    assert_eq!(
        relay_state(&initial_snapshot).0,
        RelayContactState::Released
    );
    assert!(value(&layout, &initial_snapshot, &format!("{RELAY}/coil/current")).abs() < 1e-12);

    let snapshot = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1,
        })
        .unwrap();
    assert_eq!(relay_state(&snapshot), (RelayContactState::Released, None));
    assert!(value(&layout, &snapshot, &format!("{RELAY}/coil/current")).abs() < 1e-12);
}

#[test]
fn relay_coil_rises_to_the_analytical_rl_response_and_can_switch_at_the_public_boundary() {
    const PUBLIC_STEP_SECONDS: f64 = 0.001;
    const INTERNAL_SUBSTEPS: u32 = 64;
    const COIL_RESISTANCE_OHMS: f64 = 100.0;
    const SERIES_RESISTANCE_OHMS: f64 = 1.0;
    const COIL_INDUCTANCE_HENRIES: f64 = 0.01;
    const SOURCE_VOLTS: f64 = 5.0;

    let substep_seconds = PUBLIC_STEP_SECONDS / f64::from(INTERNAL_SUBSTEPS);
    let inductive_resistance = 2.0 * COIL_INDUCTANCE_HENRIES / substep_seconds;
    let denominator = COIL_RESISTANCE_OHMS + SERIES_RESISTANCE_OHMS + inductive_resistance;
    let mut current = 0.0;
    let mut coil_voltage = 0.0;
    let mut before_last_substep = 0.0;
    for substep in 0..INTERNAL_SUBSTEPS {
        if substep + 1 == INTERNAL_SUBSTEPS {
            before_last_substep = current;
        }
        current =
            (SOURCE_VOLTS + coil_voltage + (inductive_resistance - COIL_RESISTANCE_OHMS) * current)
                / denominator;
        coil_voltage = SOURCE_VOLTS - SERIES_RESISTANCE_OHMS * current;
    }
    let pickup_current_amps = (before_last_substep + current) / 2.0;

    let (mut schematic, mut geometry) = fixture();
    let ComponentSpec::DcVoltageSource { parameters, .. } = &mut schematic.components[0] else {
        unreachable!();
    };
    parameters.voltage_volts = 0.0;
    let ComponentSpec::RelaySpdt { parameters, .. } = &mut schematic.components[2] else {
        unreachable!();
    };
    parameters.coil_resistance_ohms = COIL_RESISTANCE_OHMS;
    parameters.coil_inductance_henries = COIL_INDUCTANCE_HENRIES;
    parameters.pickup_current_amps = pickup_current_amps;
    parameters.dropout_current_amps = pickup_current_amps / 2.0;
    let relay_geometry = geometry
        .components
        .iter_mut()
        .find(|entry| entry.component_id.as_str() == RELAY)
        .unwrap();
    for (pin_id, position) in [
        ("common", GridPoint { x: 8, y: 8 }),
        ("normally_open", GridPoint { x: 10, y: 8 }),
        ("normally_closed", GridPoint { x: 12, y: 8 }),
    ] {
        relay_geometry
            .pins
            .iter_mut()
            .find(|pin| pin.pin_id == pin_id)
            .unwrap()
            .position = position;
    }

    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        internal_substeps,
        initial_snapshot,
        ..
    } = engine.compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "relay-response-1".to_owned(),
        faults: None,
    })
    else {
        panic!("the unpowered relay response fixture should compile");
    };
    assert_eq!(internal_substeps, INTERNAL_SUBSTEPS);
    assert_eq!(
        relay_state(&initial_snapshot).0,
        RelayContactState::Released
    );
    let _ = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 1,
        })
        .unwrap();
    let SetParameterResult::Success { snapshot, .. } = engine.set_parameter(SetParameterRequest {
        run_id,
        expected_document_revision: "relay-response-1".to_owned(),
        document_revision: "relay-response-2".to_owned(),
        component_id: id(SOURCE),
        parameter_id: ParameterId::VoltageVolts,
        value: SOURCE_VOLTS,
    }) else {
        panic!("the relay source step should be accepted");
    };

    let analytical_current = SOURCE_VOLTS / (COIL_RESISTANCE_OHMS + SERIES_RESISTANCE_OHMS)
        * (1.0
            - (-(COIL_RESISTANCE_OHMS + SERIES_RESISTANCE_OHMS) * PUBLIC_STEP_SECONDS
                / COIL_INDUCTANCE_HENRIES)
                .exp());
    let simulated_current = value(&layout, &snapshot, &format!("{RELAY}/coil/current"));
    assert!((simulated_current - analytical_current).abs() < 5e-5);
    assert_eq!(
        relay_state(&snapshot),
        (RelayContactState::Picked, Some(PUBLIC_STEP_SECONDS))
    );
    assert_eq!(
        snapshot.metadata.simulation_time_seconds,
        PUBLIC_STEP_SECONDS
    );
}

#[test]
fn relay_rejects_missing_hysteresis_gap() {
    let (mut schematic, geometry) = fixture();
    let ComponentSpec::RelaySpdt { parameters, .. } = &mut schematic.components[2] else {
        unreachable!();
    };
    parameters.dropout_current_amps = parameters.pickup_current_amps;
    let result = Engine::default().compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "invalid-relay".to_owned(),
        faults: None,
    });
    let CompileResult::Failure { diagnostics } = result else {
        panic!("the relay with no hysteresis gap must be refused");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code
                == open_circuit_engine::DiagnosticCode::InvalidParameter)
    );
}
