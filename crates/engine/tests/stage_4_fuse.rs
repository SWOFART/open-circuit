use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentGeometry, ComponentSpec,
    DcVoltageSourceParameters, DeviceState, Engine, FuseParameters, FuseState, GeometryEnvelope,
    GridPoint, ReferenceMarkerSpec, ResistorParameters, ResolvedPinPosition, SCHEMA_VERSION,
    Schematic, StableId,
};
use serde_json::Value;

const EXPECTED: &str = include_str!("../../../tests/golden/stage-4-fuse-i2t.expected.json");
const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const FUSE: &str = "00000000-0000-4000-8000-000000000002";
const LOAD: &str = "00000000-0000-4000-8000-000000000003";

fn id(value: &str) -> StableId {
    StableId::parse(value).unwrap()
}

fn fixture(trip_i2t_ampere_squared_seconds: f64) -> (Schematic, GeometryEnvelope) {
    let point = |x, y| GridPoint { x, y };
    let schematic = Schematic {
        schema_version: SCHEMA_VERSION,
        components: vec![
            ComponentSpec::DcVoltageSource {
                id: id(SOURCE),
                position: point(0, 0),
                rotation_deg: 0,
                parameters: DcVoltageSourceParameters {
                    voltage_volts: 2.02,
                    internal_resistance_ohms: 0.0,
                },
            },
            ComponentSpec::Fuse {
                id: id(FUSE),
                position: point(2, 0),
                rotation_deg: 0,
                parameters: FuseParameters {
                    intact_resistance_ohms: 0.01,
                    trip_i2t_ampere_squared_seconds,
                },
            },
            ComponentSpec::Resistor {
                id: id(LOAD),
                position: point(4, 0),
                rotation_deg: 0,
                parameters: ResistorParameters {
                    resistance_ohms: 1.0,
                },
            },
        ],
        wires: Vec::new(),
        junctions: Vec::new(),
        reference_markers: vec![ReferenceMarkerSpec {
            id: id("00000000-0000-4000-8000-000000000004"),
            position: point(2, 2),
        }],
    };
    let geometry = GeometryEnvelope {
        components: vec![
            ComponentGeometry {
                component_id: id(SOURCE),
                pins: vec![pin("pin1", point(0, 0)), pin("pin2", point(2, 2))],
            },
            ComponentGeometry {
                component_id: id(FUSE),
                pins: vec![pin("pin1", point(0, 0)), pin("pin2", point(2, 0))],
            },
            ComponentGeometry {
                component_id: id(LOAD),
                pins: vec![pin("pin1", point(2, 0)), pin("pin2", point(2, 2))],
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

fn compile_request(trip_i2t_ampere_squared_seconds: f64) -> CompileRequest {
    let (schematic, geometry) = fixture(trip_i2t_ampere_squared_seconds);
    CompileRequest {
        schematic,
        geometry,
        document_revision: "fuse-test-1".to_owned(),
    }
}

fn value(
    layout: &open_circuit_engine::ResultLayout,
    snapshot: &open_circuit_engine::ResultSnapshot,
    key: &str,
) -> f64 {
    let slot = layout.slots.iter().find(|slot| slot.key == key).unwrap();
    snapshot.values[slot.offset as usize]
}

fn fuse_state(snapshot: &open_circuit_engine::ResultSnapshot) -> (FuseState, Option<f64>) {
    let [
        DeviceState::Fuse {
            state,
            last_transition_time_seconds,
            ..
        },
    ] = snapshot.states.as_slice()
    else {
        panic!("the snapshot contains exactly one fuse state");
    };
    (*state, *last_transition_time_seconds)
}

#[test]
fn fuse_trips_at_the_i2t_boundary_and_remains_open_until_a_new_compile() {
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = engine.compile(compile_request(0.004))
    else {
        panic!("the known-answer fuse fixture should compile");
    };

    assert_eq!(fuse_state(&initial_snapshot), (FuseState::Intact, None));
    assert_eq!(
        value(
            &layout,
            &initial_snapshot,
            &format!("{FUSE}/accumulated_i2t")
        ),
        0.0
    );
    assert!((value(&layout, &initial_snapshot, &format!("{FUSE}:current")) - 2.0).abs() < 1e-12);
    assert!((value(&layout, &initial_snapshot, &format!("{FUSE}:voltage")) - 0.02).abs() < 1e-12);
    let initial_power = [SOURCE, FUSE, LOAD]
        .iter()
        .map(|component| {
            value(&layout, &initial_snapshot, &format!("{component}:voltage"))
                * value(&layout, &initial_snapshot, &format!("{component}:current"))
        })
        .sum::<f64>();
    assert!(
        initial_power.abs() < 1e-10,
        "signed device power sums to {initial_power} W"
    );

    let first = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 1,
        })
        .unwrap();
    assert_eq!(
        first.metadata.simulation_time_seconds,
        expected["expected"][0]["timeSeconds"]
    );
    let (state, transition_time) = fuse_state(&first);
    assert_eq!(state, FuseState::Tripped);
    let expected_trip_time = expected["expected"][0]["timeSeconds"].as_f64().unwrap();
    let trip_time_tolerance = expected["expected"][0]["tripTimeToleranceSeconds"]
        .as_f64()
        .unwrap();
    assert!((transition_time.unwrap() - expected_trip_time).abs() < trip_time_tolerance);
    let expected_i2t = expected["expected"][0]["accumulatedI2tAmpereSquaredSeconds"]
        .as_f64()
        .unwrap();
    let actual_i2t = value(&layout, &first, &format!("{FUSE}/accumulated_i2t"));
    assert!((actual_i2t - expected_i2t).abs() < 1e-12);

    let second = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1,
        })
        .unwrap();
    assert_eq!(
        second.metadata.simulation_time_seconds,
        expected["expected"][1]["timeSeconds"]
    );
    assert_eq!(fuse_state(&second), (FuseState::Tripped, transition_time));
    let expected_i2t = expected["expected"][1]["accumulatedI2tAmpereSquaredSeconds"]
        .as_f64()
        .unwrap();
    assert!(
        (value(&layout, &second, &format!("{FUSE}/accumulated_i2t")) - expected_i2t).abs() < 1e-12
    );
    let current_tolerance = expected["expected"][1]["currentToleranceAmps"]
        .as_f64()
        .unwrap();
    assert!(
        (value(&layout, &second, &format!("{FUSE}:current"))
            - expected["expected"][1]["fuseCurrentAmps"].as_f64().unwrap())
        .abs()
            < current_tolerance
    );
    let voltage_tolerance = expected["expected"][1]["voltageToleranceVolts"]
        .as_f64()
        .unwrap();
    assert!(
        (value(&layout, &second, &format!("{FUSE}:voltage"))
            - expected["expected"][1]["fuseVoltageVolts"]
                .as_f64()
                .unwrap())
        .abs()
            < voltage_tolerance
    );
    assert!(value(&layout, &second, &format!("{SOURCE}:current")).abs() < 1e-12);
    assert!(value(&layout, &second, &format!("{LOAD}:current")).abs() < 1e-12);

    let CompileResult::Success {
        run_id: new_run_id,
        layout: new_layout,
        initial_snapshot: restarted,
        ..
    } = engine.compile(compile_request(0.004))
    else {
        panic!("recompiling the known-answer fixture should succeed");
    };
    assert_ne!(new_run_id, second.metadata.run_id);
    assert_eq!(fuse_state(&restarted), (FuseState::Intact, None));
    assert_eq!(
        value(&new_layout, &restarted, &format!("{FUSE}/accumulated_i2t")),
        0.0
    );
}

#[test]
fn fuse_accumulator_is_not_clamped_when_an_interval_overshoots_the_threshold() {
    let mut engine = Engine::default();
    let CompileResult::Success { run_id, layout, .. } = engine.compile(compile_request(0.003))
    else {
        panic!("the fuse fixture should compile");
    };
    let snapshot = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1,
        })
        .unwrap();
    assert_eq!(fuse_state(&snapshot), (FuseState::Tripped, Some(0.001)));
    assert!((value(&layout, &snapshot, &format!("{FUSE}/accumulated_i2t")) - 0.004).abs() < 1e-12);
}

#[test]
fn fuse_resistance_and_trip_threshold_must_be_strictly_positive() {
    for (resistance, threshold) in [(0.0, 0.004), (0.01, 0.0)] {
        let (mut schematic, _) = fixture(threshold);
        let ComponentSpec::Fuse { parameters, .. } = schematic
            .components
            .iter_mut()
            .find(|component| component.id() == &id(FUSE))
            .unwrap()
        else {
            unreachable!();
        };
        parameters.intact_resistance_ohms = resistance;
        assert!(schematic.validate().iter().any(|diagnostic| {
            diagnostic.code == open_circuit_engine::DiagnosticCode::InvalidParameter
        }));
    }
}
