//! Stage 13 acceptance (issue #206): one drawn earth-fault circuit, run by the real Engine behind
//! `SimulationCore`, held to the independent reference of `tests/golden/stage-13/` and carried
//! through a fault-finding assignment. Nothing here adds a model.
//!
//! The circuit is a 230 V, 50 Hz single-phase supply with its return pole earthed, a 23 Ohm load
//! behind a two-pole residual-current device of 1 A rated residual current, and an insulation
//! failure of 65 Ohm from the load's live pin to a frame. With the frame bonded to an electrode of
//! 50 Ohm, the leak current is 230 / (65 + 50) = 2 A, which is a residual current of twice the
//! rated one: the reference `rcd-trip-ac-2` gives the trip time, and the frame stands at 100 V
//! RMS until it does.

mod common;

use common::Run;
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosisEntryVerdict, DiagnosisOutcome, DiagnosticCode,
    OperatorPosition, ResultSnapshot, SetOperatorStateResult, check_diagnosis,
    decode_assignment_json,
};
use serde_json::{Value, json};

const REFERENCES: &str = include_str!("../../../tests/golden/stage-13/analytical.json");

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const ELECTRODE: &str = "00000000-0000-4000-8000-000000000002";
const FRAME: &str = "00000000-0000-4000-8000-000000000003";
const LOAD: &str = "00000000-0000-4000-8000-000000000004";
const DEVICE: &str = "00000000-0000-4000-8000-000000000005";
const MARKER: &str = "00000000-0000-4000-8000-0000000000f0";
const LEAK: &str = "00000000-0000-4000-8000-0000000000a1";

fn part(id: &str, type_id: &str, parameters: Value, pins: &[(&str, (i32, i32))]) -> (Value, Value) {
    (
        json!({"typeId": type_id, "id": id, "position": {"x": 0, "y": 0},
            "rotationDeg": 0, "parameters": parameters}),
        json!({"componentId": id, "pins": pins.iter().map(|(pin, (x, y))| json!({
            "pinId": pin, "position": {"x": x, "y": y}})).collect::<Vec<_>>()}),
    )
}

/// The drawn parts. `bonded` puts the electrode on the frame's net, as a wire would.
fn parts(bonded: bool) -> Vec<(Value, Value)> {
    vec![
        part(
            SOURCE,
            "ac_voltage_source",
            json!({"voltageRmsVolts": 230.0, "frequencyHz": 50.0,
                "phaseOffsetDegrees": 0.0, "internalResistanceOhms": 0.001}),
            &[("pin1", (0, 0)), ("pin2", (2, 2))],
        ),
        part(
            LOAD,
            "resistor",
            json!({"resistanceOhms": 23.0}),
            &[("pin1", (10, 0)), ("pin2", (20, 0))],
        ),
        part(
            DEVICE,
            "rcd_two_pole",
            json!({"ratedResidualCurrentAmps": 1.0, "tripTimeAtRatedSeconds": 0.1,
                "residualSenseSeconds": 0.02, "closedContactResistanceOhms": 0.0001,
                "initialClosed": 1.0}),
            &[
                ("l1", (0, 0)),
                ("t1", (10, 0)),
                ("l2", (2, 2)),
                ("t2", (20, 0)),
            ],
        ),
        part(
            FRAME,
            "exposed_part",
            json!({"frequencyHz": 50.0, "touchVoltageLimitVolts": 50.0}),
            &[("ep", (30, 0))],
        ),
        part(
            ELECTRODE,
            "earth_electrode",
            json!({"earthResistanceOhms": 50.0}),
            &[("pe", if bonded { (30, 0) } else { (50, 50) })],
        ),
    ]
}

fn leak() -> Value {
    json!([{"id": LEAK, "kind": "leak", "componentId": LOAD, "pinId": "pin1",
        "frameId": FRAME, "value": 65.0}])
}

fn request(bonded: bool, faults: Value) -> CompileRequest {
    let parts = parts(bonded);
    serde_json::from_value(json!({
        "schematic": {
            "schemaVersion": 3,
            "components": parts.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
            "wires": [], "junctions": [], "rails": [],
            "referenceMarkers": [{"id": MARKER, "position": {"x": 2, "y": 2}}],
            "documentation": {"designators": [], "wireNumbers": [], "terminals": [], "notes": [],
                "titleBlock": {"title": "", "drawingNumber": "", "revision": "", "author": "",
                    "company": "", "date": ""},
                "sheets": []},
        },
        "geometry": {"components": parts.iter().map(|(_, g)| g.clone()).collect::<Vec<_>>()},
        "documentRevision": "rev-1",
        "faults": faults,
    }))
    .expect("a compile request")
}

fn value(run: &Run, snapshot: &ResultSnapshot, id: &str, suffix: &str) -> f64 {
    let key = format!("{id}/{suffix}");
    let slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot {key}"));
    snapshot.values[slot.offset as usize]
}

/// The newest delivered frame at or before `step`.
fn at(run: &Run, step: u64) -> ResultSnapshot {
    run.delivered
        .iter()
        .rev()
        .find(|(snapshot, _)| snapshot.metadata.simulation_step <= step)
        .map(|(snapshot, _)| snapshot.clone())
        .unwrap_or_else(|| panic!("no frame at or before step {step}"))
}

/// The first public step at which the device reports `tripped`, from the delivered frames.
fn first_trip(run: &Run) -> Option<u64> {
    run.delivered
        .iter()
        .find(|(snapshot, _)| value(run, snapshot, DEVICE, "tripped") == 1.0)
        .map(|(snapshot, _)| snapshot.metadata.simulation_step)
}

/// What the independent reference gives for a residual of twice the rated current at 50 Hz.
fn reference_trip() -> (f64, f64) {
    let all: Value = serde_json::from_str(REFERENCES).unwrap();
    let fixture = all["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"] == "rcd-trip-ac-2")
        .expect("the reference fixture");
    let check = &fixture["checks"][0];
    (
        check["expectedSeconds"].as_f64().unwrap(),
        check["toleranceSeconds"].as_f64().unwrap(),
    )
}

#[test]
fn an_earth_fault_trips_the_rcd_when_the_reference_says_and_the_frame_is_safe_afterwards() {
    let (expected, tolerance) = reference_trip();
    let mut run = Run::start_every_step(request(true, leak()));
    run.advance(400);

    let step = first_trip(&run).expect("the earth fault trips the device");
    let seconds = step as f64 * 1.0e-3;
    assert!(
        (seconds - expected).abs() <= tolerance.max(0.02 * expected),
        "tripped at {seconds} s, the reference says {expected} s"
    );

    // Before the trip the leak current is the residual (the detector's level), the frame is above the 50 V limit and
    // the Engine says so: 50 Ohm times 2 A is 100 V.
    let before = at(&run, step.saturating_sub(5));
    assert_eq!(value(&run, &before, DEVICE, "tripped"), 0.0);
    // The detector is still rising towards the 2 A of the leak, and is above the rated 1 A.
    let detector = value(&run, &before, DEVICE, "residual_rms");
    assert!(detector > 1.0 && detector < 2.01, "detector {detector} A");
    assert_eq!(value(&run, &before, FRAME, "valid"), 1.0);
    let rms = value(&run, &before, FRAME, "touch_voltage_rms");
    assert!((rms - 100.0).abs() < 0.5, "touch voltage {rms} V");
    assert_eq!(value(&run, &before, FRAME, "hazard"), 1.0);

    // After the trip nothing flows and the frame returns to zero: the hazard clears once the
    // measuring window has passed.
    let after = at(&run, step + 100);
    assert_eq!(value(&run, &after, DEVICE, "tripped"), 1.0);
    assert_eq!(value(&run, &after, DEVICE, "pole1/current"), 0.0);
    assert_eq!(value(&run, &after, DEVICE, "pole2/current"), 0.0);
    assert!(value(&run, &after, FRAME, "touch_voltage_rms") < 1.0e-6);
    assert_eq!(value(&run, &after, FRAME, "hazard"), 0.0);
    assert_eq!(value(&run, &after, FRAME, "valid"), 1.0);
}

#[test]
fn the_latched_device_refuses_to_close_and_trips_again_while_the_fault_is_there() {
    let mut run = Run::start_every_step(request(true, leak()));
    run.advance(200);
    let first = first_trip(&run).expect("tripped");

    let (_, refused) = run.set_operator_state(DEVICE, OperatorPosition::Closed);
    assert!(matches!(refused, SetOperatorStateResult::Failure { .. }));

    let (_, opened) = run.set_operator_state(DEVICE, OperatorPosition::Open);
    assert!(matches!(opened, SetOperatorStateResult::Success { .. }));
    let (_, closed) = run.set_operator_state(DEVICE, OperatorPosition::Closed);
    let SetOperatorStateResult::Success { snapshot, .. } = closed else {
        panic!("a close after an open is accepted");
    };
    // It never trips in the reply of its own close, and the fault trips it again afterwards.
    assert_eq!(value(&run, &snapshot, DEVICE, "tripped"), 0.0);
    run.advance(300);
    let latest = run.delivered.last().unwrap().0.clone();
    assert_eq!(value(&run, &latest, DEVICE, "tripped"), 1.0);
    assert!(latest.metadata.simulation_step > first);
}

#[test]
fn an_unbonded_frame_is_not_repaired_it_floats_dangerous_and_the_rcd_stays_closed() {
    let mut run = Run::start_every_step(request(false, leak()));
    let CompileResult::Success { diagnostics, .. } = &run.compile_result else {
        panic!("compiles");
    };
    let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagnosticCode::ExposedPartNotBonded),
        "{codes:?}"
    );

    run.advance(400);
    assert_eq!(
        first_trip(&run),
        None,
        "a floating frame carries no residual"
    );
    let last = run.delivered.last().unwrap().0.clone();
    // The frame floats up to the live voltage, which is the danger of a missing bond.
    let rms = value(&run, &last, FRAME, "touch_voltage_rms");
    assert!(rms > 220.0 && rms < 231.0, "frame at {rms} V");
    assert_eq!(value(&run, &last, FRAME, "hazard"), 1.0);
    assert!(value(&run, &last, DEVICE, "residual_rms") < 1.0e-6);
}

/// The assignment of this fault: the same drawing as a fault-finding task, in the file format.
fn assignment_file() -> Vec<u8> {
    let parts = parts(true);
    let locked: Vec<&str> = vec![SOURCE, LOAD, DEVICE, FRAME, ELECTRODE];
    serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "id": "00000000-0000-4000-8000-0000000005a4",
        "title": "Find the earth fault",
        "instructions": "A person who touches the frame gets a shock. Find the insulation fault.",
        "task": {"kind": "findFault", "reveal": "afterCheck"},
        "startingCircuit": {
            "schemaVersion": 1,
            "components": parts.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
            "wires": [], "junctions": [],
            "referenceMarkers": [{"id": MARKER, "position": {"x": 2, "y": 2}}],
        },
        "restrictions": {"allowedComponentTypes": ["ac_voltmeter", "ac_ammeter"],
            "lockedComponents": locked},
        "faults": leak(),
    }))
    .unwrap()
}

#[test]
fn a_fault_finding_assignment_runs_the_fault_and_judges_the_diagnosis() {
    let assignment = decode_assignment_json(&assignment_file()).expect("the file loads");

    // The task's faults are what the student's runs are compiled with: the same trip.
    let faults = serde_json::to_value(assignment.faults.as_ref().unwrap()).unwrap();
    let (expected, tolerance) = reference_trip();
    let mut run = Run::start_every_step(request(true, faults));
    run.advance(400);
    let seconds = first_trip(&run).expect("the assigned fault trips the device") as f64 * 1.0e-3;
    assert!((seconds - expected).abs() <= tolerance.max(0.02 * expected));

    let judge = |entries: Value| check_diagnosis(&assignment, &entries).expect("a valid diagnosis");

    let right = judge(json!([{"componentId": LOAD, "kind": "leak", "pinId": "pin1"}]));
    assert_eq!(right.outcome, DiagnosisOutcome::Passed);
    assert_eq!(right.entries[0].verdict, DiagnosisEntryVerdict::Correct);

    let other_pin = judge(json!([{"componentId": LOAD, "kind": "leak", "pinId": "pin2"}]));
    assert_eq!(other_pin.outcome, DiagnosisOutcome::Partial);
    assert_eq!(
        other_pin.entries[0].verdict,
        DiagnosisEntryVerdict::WrongKind
    );

    let other_part = judge(json!([{"componentId": SOURCE, "kind": "open"}]));
    assert_eq!(other_part.outcome, DiagnosisOutcome::Failed);
    assert_eq!(
        other_part.entries[0].verdict,
        DiagnosisEntryVerdict::Incorrect
    );

    // The frame is not something a student can name.
    assert!(
        check_diagnosis(
            &assignment,
            &json!([{"componentId": LOAD, "kind": "leak", "pinId": "pin1", "frameId": FRAME}])
        )
        .is_err()
    );
}
