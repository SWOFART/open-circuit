use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentGeometry, ComponentSpec,
    DcVoltageSourceParameters, DiagnosticCode, Engine, GeometryEnvelope, GridPoint,
    PmdcMotorParameters, ReferenceMarkerSpec, ResolvedPinPosition, ResultLayout, ResultQuantity,
    ResultSnapshot, SCHEMA_VERSION, Schematic, StableId,
};
use serde_json::{Value, json};

const EXPECTED: &str = include_str!("../../../tests/golden/stage-4-pmdc-motor.expected.json");
const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const MOTOR: &str = "00000000-0000-4000-8000-000000000002";
const REFERENCE: &str = "00000000-0000-4000-8000-000000000003";

fn id(value: &str) -> StableId {
    StableId::parse(value).unwrap()
}

fn fixture(
    load_torque_newton_meters: f64,
    initial_angular_velocity_radians_per_second: Option<f64>,
    initial_armature_current_amps: Option<f64>,
) -> (Schematic, GeometryEnvelope) {
    let point = |x, y| GridPoint { x, y };
    let schematic = Schematic {
        schema_version: SCHEMA_VERSION,
        components: vec![
            ComponentSpec::DcVoltageSource {
                id: id(SOURCE),
                position: point(0, 0),
                rotation_deg: 0,
                parameters: DcVoltageSourceParameters {
                    voltage_volts: 10.0,
                    internal_resistance_ohms: 0.0,
                },
            },
            ComponentSpec::PmdcMotor {
                id: id(MOTOR),
                position: point(2, 0),
                rotation_deg: 0,
                parameters: PmdcMotorParameters {
                    armature_resistance_ohms: 2.0,
                    armature_inductance_henries: 0.1,
                    motor_constant_newton_meters_per_amp: 0.1,
                    rotor_inertia_kilogram_meters_squared: 0.01,
                    viscous_friction_newton_meter_seconds_per_radian: 0.01,
                    load_torque_newton_meters,
                    initial_angular_velocity_radians_per_second,
                    initial_armature_current_amps,
                },
            },
        ],
        wires: Vec::new(),
        junctions: Vec::new(),
        reference_markers: vec![ReferenceMarkerSpec {
            id: id(REFERENCE),
            position: point(0, 0),
        }],
    };
    let geometry = GeometryEnvelope {
        components: vec![
            ComponentGeometry {
                component_id: id(SOURCE),
                pins: vec![pin("pin1", point(2, 0)), pin("pin2", point(0, 0))],
            },
            ComponentGeometry {
                component_id: id(MOTOR),
                pins: vec![pin("pin1", point(2, 0)), pin("pin2", point(0, 0))],
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

fn compile_request(
    load_torque_newton_meters: f64,
    initial_angular_velocity_radians_per_second: Option<f64>,
    initial_armature_current_amps: Option<f64>,
) -> CompileRequest {
    let (schematic, geometry) = fixture(
        load_torque_newton_meters,
        initial_angular_velocity_radians_per_second,
        initial_armature_current_amps,
    );
    CompileRequest {
        schematic,
        geometry,
        document_revision: "motor-test-1".to_owned(),
    }
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
        .expect("the requested result slot exists");
    snapshot.values[slot.offset as usize]
}

#[test]
fn pmdc_startup_matches_the_independent_coupled_ode_reference() {
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    let reference = &expected["reference"];
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        internal_substeps,
        ..
    } = engine.compile(compile_request(0.0, Some(0.0), Some(0.0)))
    else {
        panic!("the canonical PMDC startup fixture should compile");
    };

    assert_eq!(internal_substeps, 1);
    assert_eq!(
        value(&layout, &initial_snapshot, MOTOR, ResultQuantity::Current),
        0.0
    );
    assert_eq!(
        value(
            &layout,
            &initial_snapshot,
            MOTOR,
            ResultQuantity::AngularVelocity
        ),
        0.0
    );

    let snapshot = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: (reference["timeSeconds"].as_f64().unwrap() / 0.001) as u32,
        })
        .unwrap();
    let speed = value(&layout, &snapshot, MOTOR, ResultQuantity::AngularVelocity);
    let current = value(&layout, &snapshot, MOTOR, ResultQuantity::Current);
    assert!(
        (speed
            - reference["angularVelocityRadiansPerSecond"]
                .as_f64()
                .unwrap())
        .abs()
            <= reference["angularVelocityToleranceRadiansPerSecond"]
                .as_f64()
                .unwrap(),
        "speed is {speed} rad/s"
    );
    assert!(
        (current - reference["armatureCurrentAmps"].as_f64().unwrap()).abs()
            <= reference["armatureCurrentToleranceAmps"].as_f64().unwrap(),
        "current is {current} A"
    );
    assert!(speed > 0.0 && current > 0.0);

    // The ideal source and motor use opposite passive sign conventions.
    let source_power = value(&layout, &snapshot, SOURCE, ResultQuantity::Voltage)
        * value(&layout, &snapshot, SOURCE, ResultQuantity::Current);
    let motor_power = value(&layout, &snapshot, MOTOR, ResultQuantity::Voltage) * current;
    assert!((source_power + motor_power).abs() < 1e-10);
}

#[test]
fn coupled_trapezoidal_step_balances_electrical_and_mechanical_energy() {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = engine.compile(compile_request(0.0, Some(0.0), Some(0.0)))
    else {
        panic!("the PMDC fixture should compile");
    };
    let next = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 1,
        })
        .unwrap();

    let i0 = value(&layout, &initial_snapshot, MOTOR, ResultQuantity::Current);
    let w0 = value(
        &layout,
        &initial_snapshot,
        MOTOR,
        ResultQuantity::AngularVelocity,
    );
    let i1 = value(&layout, &next, MOTOR, ResultQuantity::Current);
    let w1 = value(&layout, &next, MOTOR, ResultQuantity::AngularVelocity);
    let v0 = value(&layout, &initial_snapshot, MOTOR, ResultQuantity::Voltage);
    let v1 = value(&layout, &next, MOTOR, ResultQuantity::Voltage);
    let average_i = (i0 + i1) / 2.0;
    let average_w = (w0 + w1) / 2.0;
    let average_v = (v0 + v1) / 2.0;
    let h = 0.001;
    let stored_energy_rate =
        (0.5 * 0.1 * (i1 * i1 - i0 * i0) + 0.5 * 0.01 * (w1 * w1 - w0 * w0)) / h;
    let dissipated_power = 2.0 * average_i * average_i + 0.01 * average_w * average_w;
    let load_power = 0.0 * average_w;
    assert!(
        (average_v * average_i - stored_energy_rate - dissipated_power + load_power).abs() < 1e-8,
        "discrete power balance residual is {} W",
        average_v * average_i - stored_energy_rate - dissipated_power + load_power
    );
}

#[test]
fn constant_opposing_shaft_torque_reaches_the_expected_loaded_equilibrium() {
    let mut engine = Engine::default();
    let CompileResult::Success { run_id, layout, .. } =
        engine.compile(compile_request(-0.5, Some(0.0), Some(0.0)))
    else {
        panic!("the loaded PMDC fixture should compile");
    };
    let snapshot = engine
        .advance(AdvanceRequest {
            run_id,
            step_count: 8_000,
        })
        .unwrap();
    let current = value(&layout, &snapshot, MOTOR, ResultQuantity::Current);
    let speed = value(&layout, &snapshot, MOTOR, ResultQuantity::AngularVelocity);
    assert!((current - 5.0).abs() < 0.001, "current is {current} A");
    assert!(speed.abs() < 0.01, "speed is {speed} rad/s");
    assert!((10.0 * current - 2.0 * current * current).abs() < 0.01);
}

#[test]
fn missing_initial_motor_current_uses_the_operating_point_at_configured_speed() {
    let mut engine = Engine::default();
    let CompileResult::Success {
        layout,
        initial_snapshot,
        ..
    } = engine.compile(compile_request(0.0, Some(10.0), None))
    else {
        panic!("the configured-speed motor fixture should compile");
    };
    assert!(
        (value(&layout, &initial_snapshot, MOTOR, ResultQuantity::Current) - 4.5).abs() < 1e-12
    );
    assert_eq!(
        value(
            &layout,
            &initial_snapshot,
            MOTOR,
            ResultQuantity::AngularVelocity
        ),
        10.0
    );
}

#[test]
fn motor_domains_and_nullable_initial_states_are_validated() {
    let (mut schematic, _) = fixture(0.0, None, None);
    let ComponentSpec::PmdcMotor { parameters, .. } = schematic
        .components
        .iter_mut()
        .find(|component| component.id() == &id(MOTOR))
        .unwrap()
    else {
        unreachable!();
    };
    parameters.rotor_inertia_kilogram_meters_squared = 0.0;
    assert!(
        schematic
            .validate()
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidParameter)
    );

    let (schematic, _) = fixture(0.0, None, None);
    let mut serialized = serde_json::to_value(schematic).unwrap();
    serialized["components"][1]["parameters"]["initialAngularVelocityRadiansPerSecond"] =
        json!(null);
    assert!(serde_json::from_value::<Schematic>(serialized).is_err());
}

#[test]
fn motor_coupling_contributes_to_the_deterministic_substep_rate_bound() {
    let (mut schematic, geometry) = fixture(0.0, Some(0.0), Some(0.0));
    let ComponentSpec::PmdcMotor { parameters, .. } = schematic
        .components
        .iter_mut()
        .find(|component| component.id() == &id(MOTOR))
        .unwrap()
    else {
        unreachable!();
    };
    parameters.motor_constant_newton_meters_per_amp = 2.0;
    parameters.rotor_inertia_kilogram_meters_squared = 0.001;
    let mut engine = Engine::default();
    let CompileResult::Success {
        internal_substeps, ..
    } = engine.compile(CompileRequest {
        schematic,
        geometry,
        document_revision: "motor-rate-bound".to_owned(),
    })
    else {
        panic!("the high-coupling PMDC fixture should compile");
    };
    assert_eq!(internal_substeps, 2);
}
