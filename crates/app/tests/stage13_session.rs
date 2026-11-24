//! The Stage 13 bridge (issue #203): protection devices and safety results run by the real Engine
//! behind `SimulationCore`, saved and opened by the application's project persistence, and carried
//! the way the application carries everything (JSON for commands, binary OCF2 frames). Nothing here
//! adds a model: the bridge carries the earth electrode, the exposed part, the residual-current
//! device and the breaker trip classes, the `leak` fault, their commands, result keys and
//! diagnostics, and the Engine alone computes them. The closed-form answers are in
//! `tests/golden/stage-13/` and are checked against the Engine by its own crate.

mod common;

use common::Run;
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, Engine, OperatorPosition,
    ParameterId, ResultSnapshot, SetOperatorStateResult, SetParameterResult,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const ELECTRODE: &str = "00000000-0000-4000-8000-000000000002";
const FRAME: &str = "00000000-0000-4000-8000-000000000003";
const LOAD: &str = "00000000-0000-4000-8000-000000000004";
const DEVICE: &str = "00000000-0000-4000-8000-000000000005";
const LEAK: &str = "00000000-0000-4000-8000-0000000000a1";

fn part(id: &str, type_id: &str, parameters: Value, pins: &[(&str, (i32, i32))]) -> (Value, Value) {
    (
        json!({"typeId": type_id, "id": id, "position": {"x": 0, "y": 0},
            "rotationDeg": 0, "parameters": parameters}),
        json!({"componentId": id, "pins": pins.iter().map(|(pin, (x, y))| json!({
            "pinId": pin, "position": {"x": x, "y": y}})).collect::<Vec<_>>()}),
    )
}

fn request(parts: Vec<(Value, Value)>, marker: bool, faults: Option<Value>) -> CompileRequest {
    let mut value = json!({
        "schematic": {
            "schemaVersion": 3,
            "components": parts.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
            "wires": [], "junctions": [], "rails": [],
            "referenceMarkers": if marker {
                json!([{"id": "00000000-0000-4000-8000-0000000000f0", "position": {"x": 2, "y": 2}}])
            } else {
                json!([])
            },
            "documentation": {"designators": [], "wireNumbers": [], "terminals": [], "notes": [],
                "titleBlock": {"title": "", "drawingNumber": "", "revision": "", "author": "",
                    "company": "", "date": ""},
                "sheets": []},
        },
        "geometry": {"components": parts.iter().map(|(_, g)| g.clone()).collect::<Vec<_>>()},
        "documentRevision": "rev-1",
    });
    if let Some(faults) = faults {
        value["faults"] = faults;
    }
    serde_json::from_value(value).expect("a compile request")
}

fn source() -> (Value, Value) {
    part(
        SOURCE,
        "dc_voltage_source",
        json!({"voltageVolts": 100.0, "internalResistanceOhms": 1.0}),
        &[("pin1", (0, 0)), ("pin2", (2, 2))],
    )
}

fn electrode(at: (i32, i32)) -> (Value, Value) {
    part(
        ELECTRODE,
        "earth_electrode",
        json!({"earthResistanceOhms": 9.0}),
        &[("pe", at)],
    )
}

fn exposed() -> (Value, Value) {
    part(
        FRAME,
        "exposed_part",
        json!({"frequencyHz": 50.0, "touchVoltageLimitVolts": 40.0}),
        &[("ep", (10, 0))],
    )
}

fn leak(value: f64) -> Value {
    json!([{"id": LEAK, "kind": "leak", "componentId": SOURCE, "pinId": "pin1",
        "frameId": FRAME, "value": value}])
}

/// A leak from the source to a frame bonded to the electrode: 45 V on the frame, above the limit.
fn earth_fault() -> CompileRequest {
    request(
        vec![source(), electrode((10, 0)), exposed()],
        true,
        Some(leak(10.0)),
    )
}

/// A 100 V source through an RCD (or a breaker) into a load that returns straight to the source:
/// the whole load current is a residual current.
fn protected(device_type: &str, parameters: Value, amps: f64) -> CompileRequest {
    let mut pins = vec![
        ("l1", (0, 0)),
        ("t1", (10, 0)),
        ("l2", (30, 30)),
        ("t2", (31, 30)),
    ];
    if device_type == "breaker_three_pole" {
        pins.extend([("l3", (40, 40)), ("t3", (41, 40))]);
    }
    request(
        vec![
            part(
                SOURCE,
                "dc_voltage_source",
                json!({"voltageVolts": 100.0, "internalResistanceOhms": 0.0}),
                &[("pin1", (0, 0)), ("pin2", (2, 2))],
            ),
            part(
                LOAD,
                "resistor",
                json!({"resistanceOhms": 100.0 / amps}),
                &[("pin1", (10, 0)), ("pin2", (2, 2))],
            ),
            part(DEVICE, device_type, parameters, &pins),
        ],
        true,
        None,
    )
}

fn rcd(initially_closed: bool, amps: f64) -> CompileRequest {
    let mut parameters = json!({
        "ratedResidualCurrentAmps": 0.03,
        "tripTimeAtRatedSeconds": 0.1,
        "residualSenseSeconds": 0.02,
        "closedContactResistanceOhms": 0.0001,
    });
    if initially_closed {
        parameters["initialClosed"] = json!(1.0);
    }
    protected("rcd_two_pole", parameters, amps)
}

fn class_b_breaker(amps: f64) -> CompileRequest {
    protected(
        "breaker_three_pole",
        json!({
            "closedContactResistanceOhms": 0.0001,
            "instantaneousTripAmps": 100000.0,
            "initialClosed": 1.0,
            "curveClass": 1,
            "ratedCurrentAmps": 10.0,
            "thermalSenseSeconds": 0.02,
            "tripScaleSeconds": 2.0,
            "coolSeconds": 5.0,
        }),
        amps,
    )
}

fn key(id: &str, suffix: &str) -> String {
    format!("{id}/{suffix}")
}

fn value_at(run: &Run, snapshot: &ResultSnapshot, key: &str) -> f64 {
    let slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot {key}"));
    snapshot.values[slot.offset as usize]
}

fn latest(run: &Run) -> ResultSnapshot {
    run.delivered.last().expect("a delivered frame").0.clone()
}

fn bits(snapshots: &[ResultSnapshot]) -> Vec<Vec<u64>> {
    snapshots
        .iter()
        .map(|snapshot| snapshot.values.iter().map(|v| v.to_bits()).collect())
        .collect()
}

/// The same circuit run straight on the Engine, one public step at a time.
fn direct(request: &CompileRequest, steps: u32) -> Vec<ResultSnapshot> {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        initial_snapshot,
        ..
    } = engine.compile(request.clone())
    else {
        panic!("the circuit should compile");
    };
    let mut snapshots = vec![initial_snapshot];
    for _ in 0..steps {
        snapshots.push(
            engine
                .advance(AdvanceRequest {
                    run_id: run_id.clone(),
                    step_count: 1,
                })
                .expect("the run advances"),
        );
    }
    snapshots
}

#[test]
fn the_touch_voltage_and_hazard_results_reach_the_application_exactly_as_the_engine_computes_them()
{
    let request = earth_fault();
    let mut run = Run::start_every_step(request.clone());
    run.advance(30);

    let through_bridge: Vec<ResultSnapshot> = run
        .delivered
        .iter()
        .map(|(snapshot, _)| snapshot.clone())
        .collect();
    let straight = direct(&request, 30);
    // Step 0 is the compile reply, not a streamed frame: compare every streamed step.
    for (frame, expected) in through_bridge.iter().zip(straight.iter().skip(1)) {
        assert_eq!(
            frame.metadata.simulation_step,
            expected.metadata.simulation_step
        );
        assert_eq!(
            frame.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected
                .values
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            "step {}",
            frame.metadata.simulation_step
        );
    }
    // Every streamed step arrived: a dropped frame would shorten the zip silently.
    assert_eq!(through_bridge.len(), 30);
    assert_eq!(
        through_bridge
            .last()
            .map(|frame| frame.metadata.simulation_step),
        Some(30)
    );

    let last = latest(&run);
    assert_eq!(value_at(&run, &last, &key(FRAME, "valid")), 1.0);
    assert_eq!(value_at(&run, &last, &key(FRAME, "hazard")), 1.0);
    let rms = value_at(&run, &last, &key(FRAME, "touch_voltage_rms"));
    assert!((rms - 45.0).abs() < 1e-3, "{rms}");
    let peak = value_at(&run, &last, &key(FRAME, "touch_voltage_peak"));
    assert!((peak - 45.0).abs() < 1e-3, "{peak}");
}

#[test]
fn the_new_diagnostics_reach_the_application_with_the_compile_reply() {
    // A frame with no earth and no bond: both warnings, named by the exposed part.
    let alone = request(vec![exposed()], false, None);
    let run = Run::start(alone);
    let CompileResult::Success { diagnostics, .. } = &run.compile_result else {
        panic!("compiles");
    };
    let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
    assert!(codes.contains(&DiagnosticCode::NoEarthPath), "{codes:?}");
    assert!(
        codes.contains(&DiagnosticCode::ExposedPartNotBonded),
        "{codes:?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| d.severity != open_circuit_engine::DiagnosticSeverity::Error)
    );

    // A malformed leak is refused through the bridge with the Engine's own code.
    let mut core = open_circuit_app_lib::simulation::SimulationCore::new(Default::default());
    let refused = core.compile(request_with_bad_leak());
    let CompileResult::Failure { diagnostics } = refused else {
        panic!("a leak to a part that is not an exposed part is refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidFault);
}

fn request_with_bad_leak() -> CompileRequest {
    request(
        vec![source(), electrode((10, 0)), exposed()],
        true,
        Some(
            json!([{"id": LEAK, "kind": "leak", "componentId": SOURCE, "pinId": "pin1",
            "frameId": ELECTRODE, "value": 10.0}]),
        ),
    )
}

#[test]
fn an_rcd_is_closed_tripped_refused_and_reset_through_the_application_commands() {
    // Three times the rated residual current, from an open device.
    let mut run = Run::start_every_step(rcd(false, 0.09));
    run.advance(5);
    let tripped = |run: &Run| value_at(run, &latest(run), &key(DEVICE, "tripped"));
    assert_eq!(tripped(&run), 0.0);

    let (_, closed) = run.set_operator_state(DEVICE, OperatorPosition::Closed);
    assert!(matches!(closed, SetOperatorStateResult::Success { .. }));
    // It never trips in the reply of its own close.
    let SetOperatorStateResult::Success { snapshot, .. } = closed else {
        unreachable!()
    };
    assert_eq!(value_at(&run, &snapshot, &key(DEVICE, "tripped")), 0.0);

    run.advance(400);
    assert_eq!(
        tripped(&run),
        1.0,
        "the delayed trip reaches the application"
    );
    assert_eq!(
        value_at(&run, &latest(&run), &key(DEVICE, "trip_dose")),
        1.0
    );

    // Refused while latched, and a press is not an operation of this device.
    let (_, again) = run.set_operator_state(DEVICE, OperatorPosition::Closed);
    assert!(matches!(again, SetOperatorStateResult::Failure { .. }));
    let (_, pressed) = run.set_operator_state(DEVICE, OperatorPosition::Pressed);
    assert!(matches!(pressed, SetOperatorStateResult::Failure { .. }));

    // An open clears the latch, the dose and the detector.
    let (_, opened) = run.set_operator_state(DEVICE, OperatorPosition::Open);
    let SetOperatorStateResult::Success { snapshot, .. } = opened else {
        panic!("an open is accepted");
    };
    assert_eq!(value_at(&run, &snapshot, &key(DEVICE, "tripped")), 0.0);
    assert_eq!(value_at(&run, &snapshot, &key(DEVICE, "trip_dose")), 0.0);
    assert_eq!(value_at(&run, &snapshot, &key(DEVICE, "residual_rms")), 0.0);
}

#[test]
fn a_breaker_class_trips_thermally_and_reports_its_dose_through_the_application() {
    let request = class_b_breaker(20.0);
    let mut run = Run::start_every_step(request.clone());
    assert!(
        run.layout
            .slots
            .iter()
            .any(|slot| slot.key == key(DEVICE, "thermal_dose")),
        "a classed breaker has its dose slot"
    );
    run.advance(3000);

    let last = latest(&run);
    assert_eq!(value_at(&run, &last, &key(DEVICE, "tripped")), 1.0);
    // The dose reached 1 at the trip and has been cooling since (the breaker stays latched).
    let dose = value_at(&run, &last, &key(DEVICE, "thermal_dose"));
    assert!(dose > 0.3 && dose < 1.0, "{dose}");

    // The bridge carries the run bit for bit: the last frame is step 3000 and equals the direct run.
    assert_eq!(last.metadata.simulation_step, 3000);
    let straight = direct(&request, 3000);
    let streamed: Vec<ResultSnapshot> = run
        .delivered
        .iter()
        .map(|(snapshot, _)| snapshot.clone())
        .collect();
    assert_eq!(
        bits(&streamed[streamed.len() - 1..]),
        bits(&straight[straight.len() - 1..])
    );
}

#[test]
fn only_the_contract_live_parameters_of_the_new_devices_are_accepted_as_live_updates() {
    let accepted =
        |result: &SetParameterResult| matches!(result, SetParameterResult::Success { .. });

    // The exposed part: frequency and limit.
    let mut run = Run::start_every_step(earth_fault());
    run.advance(30);
    for (parameter, value) in [
        (ParameterId::TouchVoltageLimitVolts, 60.0),
        (ParameterId::FrequencyHz, 25.0),
    ] {
        let (_, result) = run.set_parameter(FRAME, parameter, value);
        assert!(accepted(&result), "{parameter:?} is live");
    }
    // The electrode is compile-only.
    let (_, result) = run.set_parameter(ELECTRODE, ParameterId::EarthResistanceOhms, 20.0);
    assert!(!accepted(&result));

    // The RCD: rated residual current and trip time are live, the rest compile-only.
    let mut run = Run::start_every_step(rcd(true, 0.09));
    for (parameter, live) in [
        (ParameterId::RatedResidualCurrentAmps, true),
        (ParameterId::TripTimeAtRatedSeconds, true),
        (ParameterId::ResidualSenseSeconds, false),
        (ParameterId::ClosedContactResistanceOhms, false),
    ] {
        let (_, result) = run.set_parameter(DEVICE, parameter, 0.05);
        assert_eq!(accepted(&result), live, "{parameter:?}");
    }

    // The breaker keeps `instantaneousTripAmps` as its only live parameter; the class inputs are
    // compile-only.
    let mut run = Run::start_every_step(class_b_breaker(20.0));
    let (_, result) = run.set_parameter(DEVICE, ParameterId::InstantaneousTripAmps, 500.0);
    assert!(accepted(&result));
    for parameter in [
        ParameterId::CurveClass,
        ParameterId::RatedCurrentAmps,
        ParameterId::ThermalSenseSeconds,
        ParameterId::TripScaleSeconds,
        ParameterId::CoolSeconds,
    ] {
        let (_, result) = run.set_parameter(DEVICE, parameter, 2.0);
        assert!(!accepted(&result), "{parameter:?} is compile-only");
    }
}

#[test]
fn a_saved_and_opened_protection_project_keeps_its_inputs_and_stores_nothing_derived() {
    let directory = tempfile::tempdir().unwrap();
    let requests = [
        ("earth-fault", earth_fault()),
        ("rcd", rcd(true, 0.09)),
        ("breaker-class", class_b_breaker(20.0)),
    ];

    for (name, request) in requests {
        let path = directory.path().join(format!("{name}.ocircuit"));
        let mut run = Run::start_every_step(request.clone());
        run.advance(30);

        write_project_file(&path, &request.schematic).expect("saved after the run");
        let file = std::fs::read_to_string(&path).unwrap();
        for derived in [
            "snapshot",
            "runId",
            "layout",
            "tripped",
            "tripDose",
            "trip_dose",
            "residualRms",
            "thermalDose",
            "touchVoltageRms",
            "touchVoltagePeak",
            "hazard",
            "frameSequence",
            // A fault is an overlay of the compile request, never part of a project.
            "faults",
            "\"leak\"",
            "frameId",
        ] {
            assert!(
                !file.contains(derived),
                "{name}: the file holds `{derived}`"
            );
        }
        let reopened = read_project_file(&path).expect("reopens");
        assert_eq!(
            reopened, request.schematic,
            "{name}: every drawn parameter round-trips"
        );

        // A run of the reopened document starts at step 0 and recomputes the same numbers.
        let mut second = Run::start_every_step(CompileRequest {
            schematic: reopened,
            ..request.clone()
        });
        assert_eq!(second.initial_snapshot.metadata.simulation_step, 0);
        second.advance(30);
        let delivered = |run: &Run| -> Vec<ResultSnapshot> {
            run.delivered
                .iter()
                .map(|(frame, _)| frame.clone())
                .collect()
        };
        assert_eq!(bits(&delivered(&run)), bits(&delivered(&second)), "{name}");
    }
}

#[test]
fn a_breaker_without_a_class_is_saved_without_the_class_parameters() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plain.ocircuit");
    let plain = protected(
        "breaker_three_pole",
        json!({"closedContactResistanceOhms": 0.01, "instantaneousTripAmps": 100.0}),
        1.0,
    );

    write_project_file(&path, &plain.schematic).unwrap();
    let file = std::fs::read_to_string(&path).unwrap();
    for absent in [
        "curveClass",
        "ratedCurrentAmps",
        "thermalSenseSeconds",
        "tripScaleSeconds",
        "coolSeconds",
    ] {
        assert!(
            !file.contains(absent),
            "`{absent}` was written for a class 0 breaker"
        );
    }
    assert_eq!(read_project_file(&path).unwrap(), plain.schematic);
}
