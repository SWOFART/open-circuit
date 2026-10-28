//! Assignment checks (docs/contracts/stage-5-assignments.md section 4). The known answers are
//! direct circuit equations, in `tests/golden/stage-5-divider-build.expected.json`,
//! `tests/golden/stage-5-fuse-state.expected.json`, and below.

use open_circuit_engine::{
    CheckOutcome, CheckRequest, CheckResult, CheckVerdict, CriterionResult, DeviceStateName,
    DiagnosticCode, DiagnosticReference, Engine, evaluate_checks,
};
use serde_json::{Value, json};

const DIVIDER: &str = include_str!("../../../tests/golden/stage-5-divider-build.expected.json");
const FUSE: &str = include_str!("../../../tests/golden/stage-5-fuse-state.expected.json");

const V1: &str = "00000000-0000-4000-8000-000000000001";
const VM1: &str = "00000000-0000-4000-8000-000000000002";
const F1: &str = "00000000-0000-4000-8000-000000000003";
const R_TOP: &str = "00000000-0000-4000-8000-000000000011";
const R_BOTTOM: &str = "00000000-0000-4000-8000-000000000012";
const R_MIDDLE: &str = "00000000-0000-4000-8000-000000000013";
const CAPACITOR: &str = "00000000-0000-4000-8000-000000000014";
const RELAY: &str = "00000000-0000-4000-8000-000000000015";

const READING: &str = "00000000-0000-4000-8000-0000000000c1";
const STABLE: &str = "00000000-0000-4000-8000-0000000000c2";
const TWO_RESISTORS: &str = "00000000-0000-4000-8000-0000000000c3";
const TRIPS: &str = "00000000-0000-4000-8000-0000000000c4";

type Part = (Value, Value);

fn part(id: &str, type_id: &str, parameters: Value, pins: &[(&str, (i32, i32))]) -> Part {
    (
        json!({
            "typeId": type_id,
            "id": id,
            "position": { "x": 0, "y": 0 },
            "rotationDeg": 0,
            "parameters": parameters,
        }),
        json!({
            "componentId": id,
            "pins": pins
                .iter()
                .map(|(pin, (x, y))| json!({ "pinId": pin, "position": { "x": x, "y": y } }))
                .collect::<Vec<_>>(),
        }),
    )
}

fn two_pin(id: &str, type_id: &str, parameters: Value, pin1: (i32, i32), pin2: (i32, i32)) -> Part {
    part(id, type_id, parameters, &[("pin1", pin1), ("pin2", pin2)])
}

fn resistor(id: &str, ohms: f64, pin1: (i32, i32), pin2: (i32, i32)) -> Part {
    two_pin(
        id,
        "resistor",
        json!({ "resistanceOhms": ohms }),
        pin1,
        pin2,
    )
}

fn source(volts: f64) -> Part {
    two_pin(
        V1,
        "dc_voltage_source",
        json!({ "voltageVolts": volts, "internalResistanceOhms": 0.0 }),
        (0, 0),
        (2, 2),
    )
}

fn voltmeter() -> Part {
    two_pin(
        VM1,
        "voltmeter",
        json!({ "inputResistanceOhms": 1e7 }),
        (4, 0),
        (2, 2),
    )
}

fn fuse() -> Part {
    two_pin(
        F1,
        "fuse",
        json!({ "intactResistanceOhms": 0.01, "tripI2tAmpereSquaredSeconds": 0.004 }),
        (0, 0),
        (4, 0),
    )
}

/// A schematic and the geometry the editor would derive, with a reference marker at (2, 2).
fn schematic(parts: &[Part]) -> (Value, Value) {
    (
        json!({
            "schemaVersion": 1,
            "components": parts.iter().map(|(component, _)| component.clone()).collect::<Vec<_>>(),
            "wires": [],
            "junctions": [],
            "referenceMarkers": [{
                "id": "00000000-0000-4000-8000-0000000000f0",
                "position": { "x": 2, "y": 2 },
            }],
        }),
        json!({ "components": parts.iter().map(|(_, pins)| pins.clone()).collect::<Vec<_>>() }),
    )
}

fn value_criterion(
    id: &str,
    component: &str,
    at_seconds: f64,
    expected: f64,
    tolerance_abs: f64,
) -> Value {
    json!({
        "kind": "value", "id": id,
        "signal": { "key": format!("{component}:voltage"), "unit": "volt" },
        "atSeconds": at_seconds, "expected": expected,
        "toleranceAbs": tolerance_abs, "toleranceRel": 0.0,
    })
}

fn range_criterion(
    id: &str,
    key: &str,
    from_seconds: f64,
    to_seconds: f64,
    min: f64,
    max: f64,
) -> Value {
    json!({
        "kind": "range", "id": id,
        "signal": { "key": key, "unit": "volt" },
        "fromSeconds": from_seconds, "toSeconds": to_seconds, "min": min, "max": max,
    })
}

fn state_criterion(id: &str, component: &str, state: &str, at_seconds: f64) -> Value {
    json!({
        "kind": "deviceState", "id": id, "componentId": component,
        "state": state, "atSeconds": at_seconds,
    })
}

fn count_criterion(id: &str, type_id: &str, min: u32, max: Option<u32>) -> Value {
    let mut criterion =
        json!({ "kind": "componentCount", "id": id, "typeId": type_id, "min": min });
    if let Some(max) = max {
        criterion["max"] = json!(max);
    }
    criterion
}

/// The request of a check: `student` is the attempt, `starting` the assignment's circuit.
fn request(checks: Value, locked: &[&str], starting: &[Part], student: &[Part]) -> CheckRequest {
    let (starting_circuit, _) = schematic(starting);
    let (student_schematic, geometry) = schematic(student);
    serde_json::from_value(json!({
        "checks": checks,
        "lockedComponents": locked,
        "startingCircuit": starting_circuit,
        "schematic": student_schematic,
        "geometry": geometry,
        "documentRevision": "rev-7",
    }))
    .unwrap()
}

fn divider_criteria() -> Value {
    json!([
        value_criterion(READING, VM1, 0.0, 2.5, 0.025),
        range_criterion(STABLE, &format!("{VM1}:voltage"), 0.0, 0.01, 2.4, 2.6),
        count_criterion(TWO_RESISTORS, "resistor", 2, Some(2)),
    ])
}

fn divider_request(student: &[Part]) -> CheckRequest {
    request(
        divider_criteria(),
        &[V1, VM1],
        &[source(10.0), voltmeter()],
        student,
    )
}

fn with_added(parts: Vec<Part>) -> Vec<Part> {
    let mut all = vec![source(10.0), voltmeter()];
    all.extend(parts);
    all
}

fn verdict_of(result: &CheckResult, id: &str) -> CheckVerdict {
    result
        .criteria
        .iter()
        .find_map(|criterion| match criterion {
            CriterionResult::Value {
                id: found, verdict, ..
            }
            | CriterionResult::Range {
                id: found, verdict, ..
            }
            | CriterionResult::DeviceState {
                id: found, verdict, ..
            }
            | CriterionResult::ComponentCount {
                id: found, verdict, ..
            } if found.as_str() == id => Some(*verdict),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no criterion {id}"))
}

fn measured_value(result: &CheckResult, id: &str) -> Option<f64> {
    result
        .criteria
        .iter()
        .find_map(|criterion| match criterion {
            CriterionResult::Value {
                id: found,
                measured,
                ..
            }
            | CriterionResult::Range {
                id: found,
                measured,
                ..
            } if found.as_str() == id => *measured,
            _ => None,
        })
}

fn diagnostic_codes(result: &CheckResult) -> Vec<DiagnosticCode> {
    result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} is not within {tolerance} of {expected}"
    );
}

fn verdict_name(verdict: CheckVerdict) -> &'static str {
    match verdict {
        CheckVerdict::Pass => "pass",
        CheckVerdict::Fail => "fail",
        CheckVerdict::NotEvaluated => "notEvaluated",
    }
}

fn outcome_name(outcome: CheckOutcome) -> &'static str {
    match outcome {
        CheckOutcome::Passed => "passed",
        CheckOutcome::Partial => "partial",
        CheckOutcome::Failed => "failed",
        CheckOutcome::NotEvaluated => "notEvaluated",
    }
}

fn criterion_id(name: &str) -> &'static str {
    match name {
        "reading" => READING,
        "stable" => STABLE,
        "two-resistors" => TWO_RESISTORS,
        "trips" => TRIPS,
        other => panic!("no criterion named {other}"),
    }
}

/// The student's parts of an attempt of the divider fixture.
fn divider_attempt(name: &str) -> Vec<Part> {
    let correct = || {
        vec![
            resistor(R_TOP, 30_000.0, (0, 0), (4, 0)),
            resistor(R_BOTTOM, 10_000.0, (4, 0), (2, 2)),
        ]
    };
    match name {
        "correct" => with_added(correct()),
        "three-resistors" => with_added(vec![
            resistor(R_TOP, 20_000.0, (0, 0), (2, 0)),
            resistor(R_MIDDLE, 10_000.0, (2, 0), (4, 0)),
            resistor(R_BOTTOM, 10_000.0, (4, 0), (2, 2)),
        ]),
        "equal-resistors" => with_added(vec![
            resistor(R_TOP, 10_000.0, (0, 0), (4, 0)),
            resistor(R_BOTTOM, 10_000.0, (4, 0), (2, 2)),
        ]),
        "no-resistors" => with_added(Vec::new()),
        // A modified file changed the locked source from 10 V to 20 V.
        "locked-part-changed" => {
            let mut all = vec![source(20.0), voltmeter()];
            all.extend(correct());
            all
        }
        // Both pins of the ideal source on the same grid point: a short across it.
        "source-shorted" => {
            let mut shorted = source(10.0);
            shorted.1["pins"][0]["position"] = json!({ "x": 2, "y": 2 });
            let mut all = vec![shorted, voltmeter()];
            all.extend(correct());
            all
        }
        other => panic!("no attempt named {other}"),
    }
}

#[test]
fn the_divider_attempts_get_the_verdicts_of_the_golden_fixture() {
    let golden: Value = serde_json::from_str(DIVIDER).unwrap();
    let attempts = golden["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 6);

    for attempt in attempts {
        let name = attempt["name"].as_str().unwrap();
        let expected = &attempt["expected"];
        let result = evaluate_checks(divider_request(&divider_attempt(name)));

        assert_eq!(
            outcome_name(result.outcome),
            expected["outcome"].as_str().unwrap(),
            "{name}: outcome"
        );
        assert_eq!(
            result.outcome_key,
            format!("check.outcome.{}", expected["outcome"].as_str().unwrap()),
            "{name}: outcome key"
        );
        for (criterion, verdict) in expected["verdicts"].as_object().unwrap() {
            let actual = if criterion == "locked" {
                result.locked.verdict
            } else {
                verdict_of(&result, criterion_id(criterion))
            };
            assert_eq!(
                verdict_name(actual),
                verdict.as_str().unwrap(),
                "{name}: {criterion}"
            );
        }
        if let Some(reading) = attempt["voltmeterReadingVolts"].as_f64() {
            let tolerance = attempt["readingToleranceVolts"].as_f64().unwrap();
            assert_close(
                measured_value(&result, READING).unwrap(),
                reading,
                tolerance,
                &format!("{name}: the reading"),
            );
        }
        if let Some(codes) = expected["diagnostics"].as_array() {
            assert_eq!(
                diagnostic_codes(&result),
                codes
                    .iter()
                    .map(|code| serde_json::from_value(code.clone()).unwrap())
                    .collect::<Vec<DiagnosticCode>>(),
                "{name}: diagnostics"
            );
        }
        assert_eq!(result.document_revision, "rev-7");
    }
}

#[test]
fn a_circuit_that_does_not_compile_leaves_the_run_criteria_unevaluated_but_reads_the_schematic() {
    let result = evaluate_checks(divider_request(&divider_attempt("source-shorted")));

    assert_eq!(result.outcome, CheckOutcome::NotEvaluated);
    assert_eq!(result.outcome_key, "check.outcome.notEvaluated");
    assert_eq!(
        diagnostic_codes(&result),
        vec![DiagnosticCode::ShortCircuit]
    );
    assert_eq!(verdict_of(&result, READING), CheckVerdict::NotEvaluated);
    assert_eq!(verdict_of(&result, STABLE), CheckVerdict::NotEvaluated);
    assert_eq!(verdict_of(&result, TWO_RESISTORS), CheckVerdict::Pass);
    assert_eq!(result.locked.verdict, CheckVerdict::Pass);
    assert_eq!(
        (
            result.counts.pass,
            result.counts.fail,
            result.counts.not_evaluated
        ),
        (1, 0, 2)
    );
    // No measured value exists for a criterion that was not evaluated.
    assert_eq!(measured_value(&result, READING), None);
    assert!(result.criteria.iter().any(|criterion| matches!(
        criterion,
        CriterionResult::Value { message_key, .. } if message_key == "check.value.notEvaluated"
    )));
}

#[test]
fn a_changed_locked_part_fails_the_check_even_when_the_circuit_does_not_compile() {
    let mut shorted_and_changed = divider_attempt("source-shorted");
    shorted_and_changed[0].0["parameters"]["voltageVolts"] = json!(20.0);
    let result = evaluate_checks(divider_request(&shorted_and_changed));

    assert_eq!(result.locked.verdict, CheckVerdict::Fail);
    assert_eq!(result.locked.message_key, "check.locked.fail");
    assert_eq!(result.outcome, CheckOutcome::Failed);
    assert_eq!(
        diagnostic_codes(&result),
        vec![DiagnosticCode::ShortCircuit]
    );
}

#[test]
fn the_fuse_attempts_get_the_verdicts_of_the_golden_fixture() {
    let golden: Value = serde_json::from_str(FUSE).unwrap();
    let attempts = golden["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 3);
    let checks = json!([state_criterion(TRIPS, F1, "tripped", 0.002)]);
    let starting = [source(2.02), fuse()];

    for attempt in attempts {
        let name = attempt["name"].as_str().unwrap();
        let mut student = vec![source(2.02), fuse()];
        match name {
            "one-ohm-load" => student.push(resistor(R_TOP, 1.0, (4, 0), (2, 2))),
            "ten-ohm-load" => student.push(resistor(R_TOP, 10.0, (4, 0), (2, 2))),
            "no-load" => {}
            other => panic!("no attempt named {other}"),
        }
        let result = evaluate_checks(request(checks.clone(), &[V1, F1], &starting, &student));
        let expected = &attempt["expected"];

        assert_eq!(
            outcome_name(result.outcome),
            expected["outcome"].as_str().unwrap(),
            "{name}: outcome"
        );
        assert_eq!(result.locked.verdict, CheckVerdict::Pass, "{name}: locked");
        assert_eq!(
            verdict_name(verdict_of(&result, TRIPS)),
            expected["verdicts"]["trips"].as_str().unwrap(),
            "{name}: trips"
        );
        let CriterionResult::DeviceState {
            measured,
            expected: wanted,
            references,
            ..
        } = &result.criteria[0]
        else {
            panic!("a device state result");
        };
        let state = expected["fuseState"].as_str().unwrap();
        assert_eq!(
            measured.map(|state| format!("{state:?}").to_lowercase()),
            Some(state.to_owned()),
            "{name}: the fuse state"
        );
        assert_eq!(*wanted, DeviceStateName::Tripped);
        assert_eq!(
            references,
            &vec![DiagnosticReference::Component {
                component_id: serde_json::from_value(json!(F1)).unwrap()
            }]
        );
    }
}

/// The reading of the divider with a 30 kOhm / 10 kOhm pair and the 10 MOhm meter.
fn closed_form_reading() -> f64 {
    let lower = 10_000.0 * 1e7 / (10_000.0 + 1e7);
    10.0 * lower / (30_000.0 + lower)
}

#[test]
fn a_tolerance_is_inclusive_and_just_outside_it_fails() {
    let reading = closed_form_reading();
    let student = divider_attempt("correct");
    let verdict_with = |tolerance_abs: f64, tolerance_rel: f64, expected: f64| {
        let mut criterion = value_criterion(READING, VM1, 0.0, expected, tolerance_abs);
        criterion["toleranceRel"] = json!(tolerance_rel);
        let result = evaluate_checks(request(
            json!([criterion]),
            &[V1, VM1],
            &[source(10.0), voltmeter()],
            &student,
        ));
        verdict_of(&result, READING)
    };
    let off = 5.0 - reading;

    // Absolute tolerance: a nanovolt inside the distance to 5 V passes, a nanovolt short fails.
    assert_eq!(verdict_with(off + 1e-9, 0.0, 5.0), CheckVerdict::Pass);
    assert_eq!(verdict_with(off - 1e-9, 0.0, 5.0), CheckVerdict::Fail);
    // Relative tolerance: `toleranceRel * |expected|` adds to the absolute one.
    assert_eq!(verdict_with(0.0, off / 5.0 + 1e-9, 5.0), CheckVerdict::Pass);
    assert_eq!(verdict_with(0.0, off / 5.0 - 1e-9, 5.0), CheckVerdict::Fail);
    assert_eq!(
        verdict_with(off / 2.0, off / 2.0 / 5.0 + 1e-9, 5.0),
        CheckVerdict::Pass
    );
    assert_eq!(
        verdict_with(off / 2.0, off / 2.0 / 5.0 - 1e-9, 5.0),
        CheckVerdict::Fail
    );
    // The same distance below the expected value.
    assert_eq!(
        verdict_with(off + 1e-9, 0.0, reading - off),
        CheckVerdict::Pass
    );
    assert_eq!(
        verdict_with(off - 1e-9, 0.0, reading - off),
        CheckVerdict::Fail
    );
}

#[test]
fn a_value_exactly_on_its_tolerance_passes_and_the_next_float_outside_fails() {
    // The ideal 10 V source reads exactly 10 V, so the distances below are exact in binary.
    let parts = [source(10.0), resistor(R_TOP, 1000.0, (0, 0), (2, 2))];
    let verdict_with = |expected: f64, tolerance_abs: f64, tolerance_rel: f64| {
        let mut criterion = value_criterion(READING, V1, 0.0, expected, tolerance_abs);
        criterion["toleranceRel"] = json!(tolerance_rel);
        let result = evaluate_checks(request(json!([criterion]), &[V1], &parts, &parts));
        assert_eq!(measured_value(&result, READING), Some(10.0));
        verdict_of(&result, READING)
    };
    let below = |value: f64| f64::from_bits(value.to_bits() - 1);

    assert_eq!(verdict_with(9.5, 0.5, 0.0), CheckVerdict::Pass);
    assert_eq!(verdict_with(9.5, below(0.5), 0.0), CheckVerdict::Fail);
    assert_eq!(verdict_with(10.5, 0.5, 0.0), CheckVerdict::Pass);
    assert_eq!(verdict_with(10.5, below(0.5), 0.0), CheckVerdict::Fail);
    // 0.25 * |8| = 2 exactly, and the two tolerances add: 1.5 + 0.0625 * 8 = 2.
    assert_eq!(verdict_with(8.0, 0.0, 0.25), CheckVerdict::Pass);
    assert_eq!(verdict_with(8.0, 0.0, below(0.25)), CheckVerdict::Fail);
    assert_eq!(verdict_with(8.0, 1.5, 0.0625), CheckVerdict::Pass);
    assert_eq!(verdict_with(8.0, below(1.5), 0.0625), CheckVerdict::Fail);
    assert_eq!(verdict_with(10.0, 0.0, 0.0), CheckVerdict::Pass);
}

/// 10 V ideal source, 1 kOhm, and a 100 uF capacitor (tau = 100 ms) from rest.
fn rc_parts() -> Vec<Part> {
    vec![
        source(10.0),
        resistor(R_TOP, 1000.0, (0, 0), (4, 0)),
        two_pin(
            CAPACITOR,
            "capacitor",
            json!({ "capacitanceFarads": 1e-4, "initialVoltageVolts": 0.0 }),
            (4, 0),
            (2, 2),
        ),
    ]
}

fn rc_request(checks: Value) -> CheckRequest {
    request(checks, &[V1], &rc_parts(), &rc_parts())
}

fn rc_voltage(seconds: f64) -> f64 {
    10.0 * (1.0 - (-seconds / 0.1).exp())
}

fn range_fields(result: &CheckResult, id: &str) -> (CheckVerdict, Option<f64>, Option<f64>) {
    result
        .criteria
        .iter()
        .find_map(|criterion| match criterion {
            CriterionResult::Range {
                id: found,
                verdict,
                measured,
                at_seconds,
                ..
            } if found.as_str() == id => Some((*verdict, *measured, *at_seconds)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no range criterion {id}"))
}

#[test]
fn a_value_is_read_at_the_named_public_step() {
    let checks = json!([
        value_criterion(READING, CAPACITOR, 0.0, 0.0, 1e-9),
        value_criterion(STABLE, CAPACITOR, 0.05, rc_voltage(0.05), 0.01),
        value_criterion(TWO_RESISTORS, CAPACITOR, 0.3, rc_voltage(0.3), 0.01),
    ]);
    let result = evaluate_checks(rc_request(checks));

    assert_eq!(result.outcome, CheckOutcome::Passed);
    assert_close(
        measured_value(&result, READING).unwrap(),
        0.0,
        1e-9,
        "step 0",
    );
    assert_close(
        measured_value(&result, STABLE).unwrap(),
        rc_voltage(0.05),
        0.01,
        "50 ms",
    );
    assert_close(
        measured_value(&result, TWO_RESISTORS).unwrap(),
        rc_voltage(0.3),
        0.01,
        "300 ms",
    );
}

#[test]
fn a_range_reports_the_step_with_the_smallest_margin_and_fails_outside_a_bound() {
    let key = format!("{CAPACITOR}:voltage");
    let window = (0.02, 0.03);
    let low = rc_voltage(0.02);
    let high = rc_voltage(0.03);
    let checks = json!([
        // Inside with room: the closest bound is the lower one, at the start of the window.
        range_criterion(READING, &key, window.0, window.1, 0.0, 100.0),
        // The lower bound is above the value at the start: fails there.
        range_criterion(STABLE, &key, window.0, window.1, low + 0.1, 100.0),
        // The upper bound is below the value at the end: fails there.
        range_criterion(TWO_RESISTORS, &key, window.0, window.1, 0.0, high - 0.1),
        // The bounds are inclusive and the window too: it ends before the value leaves them.
        range_criterion(TRIPS, &key, window.0, window.1, low - 0.01, high + 0.01),
    ]);
    let result = evaluate_checks(rc_request(checks));

    let (verdict, measured, at) = range_fields(&result, READING);
    assert_eq!(verdict, CheckVerdict::Pass);
    assert_close(measured.unwrap(), low, 0.01, "lowest value");
    assert_close(at.unwrap(), 0.02, 1e-12, "its time");

    let (verdict, measured, at) = range_fields(&result, STABLE);
    assert_eq!(verdict, CheckVerdict::Fail);
    assert_close(measured.unwrap(), low, 0.01, "value below the bound");
    assert_close(at.unwrap(), 0.02, 1e-12, "the step below the bound");

    let (verdict, measured, at) = range_fields(&result, TWO_RESISTORS);
    assert_eq!(verdict, CheckVerdict::Fail);
    assert_close(measured.unwrap(), high, 0.01, "value above the bound");
    assert_close(at.unwrap(), 0.03, 1e-12, "the step above the bound");

    assert_eq!(range_fields(&result, TRIPS).0, CheckVerdict::Pass);
    assert_eq!(result.outcome, CheckOutcome::Partial);
}

#[test]
fn a_range_with_equal_margins_reports_the_earliest_step() {
    // A constant 10 mA through 1 kOhm: every step has the same margin.
    let parts = vec![source(10.0), resistor(R_TOP, 1000.0, (0, 0), (2, 2))];
    let key = format!("{R_TOP}:current");
    let mut criterion = range_criterion(READING, &key, 0.0, 0.005, 0.0, 1.0);
    criterion["signal"]["unit"] = json!("amp");
    let result = evaluate_checks(request(json!([criterion]), &[V1], &parts, &parts));

    let (verdict, measured, at) = range_fields(&result, READING);
    assert_eq!(verdict, CheckVerdict::Pass);
    assert_close(measured.unwrap(), 0.01, 1e-12, "the current");
    assert_eq!(at, Some(0.0));
}

/// 5 V, a 10 Ohm resistor, and a relay whose 100 Ohm coil carries 45.5 mA against 40 mA pickup.
fn relay_parts() -> Vec<Part> {
    vec![
        source(5.0),
        resistor(R_TOP, 10.0, (0, 0), (4, 0)),
        part(
            RELAY,
            "relay_spdt",
            json!({
                "coilResistanceOhms": 100.0, "coilInductanceHenries": 0.01,
                "pickupCurrentAmps": 0.04, "dropoutCurrentAmps": 0.02,
                "closedContactResistanceOhms": 0.1,
            }),
            &[
                ("coil_positive", (4, 0)),
                ("coil_negative", (2, 2)),
                ("common", (20, 0)),
                ("normally_open", (22, 0)),
                ("normally_closed", (24, 0)),
            ],
        ),
    ]
}

fn state_result(result: &CheckResult, id: &str) -> (CheckVerdict, Option<DeviceStateName>) {
    result
        .criteria
        .iter()
        .find_map(|criterion| match criterion {
            CriterionResult::DeviceState {
                id: found,
                verdict,
                measured,
                ..
            } if found.as_str() == id => Some((*verdict, *measured)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no device state criterion {id}"))
}

#[test]
fn each_device_state_criterion_reads_the_engine_state_of_a_relay_or_a_fuse() {
    let parts = relay_parts();
    let checks = json!([
        state_criterion(READING, RELAY, "picked", 0.005),
        state_criterion(STABLE, RELAY, "released", 0.005),
        // A fuse state for a relay, a part that is not there, and a part of another type.
        state_criterion(TWO_RESISTORS, RELAY, "tripped", 0.005),
        state_criterion(
            TRIPS,
            "00000000-0000-4000-8000-0000000000ee",
            "intact",
            0.005
        ),
        state_criterion(
            "00000000-0000-4000-8000-0000000000c5",
            R_TOP,
            "released",
            0.005
        ),
    ]);
    let result = evaluate_checks(request(checks, &[V1], &parts, &parts));

    assert_eq!(
        state_result(&result, READING),
        (CheckVerdict::Pass, Some(DeviceStateName::Picked))
    );
    assert_eq!(
        state_result(&result, STABLE),
        (CheckVerdict::Fail, Some(DeviceStateName::Picked))
    );
    for id in [TWO_RESISTORS, TRIPS, "00000000-0000-4000-8000-0000000000c5"] {
        assert_eq!(
            state_result(&result, id),
            (CheckVerdict::NotEvaluated, None),
            "{id}"
        );
    }
    assert_eq!(result.outcome, CheckOutcome::Partial);
}

#[test]
fn a_part_the_student_removed_leaves_its_criteria_unevaluated() {
    let missing = "00000000-0000-4000-8000-0000000000ee";
    let checks = json!([
        value_criterion(READING, missing, 0.0, 1.0, 0.1),
        range_criterion(STABLE, &format!("{missing}:current"), 0.0, 0.01, 0.0, 1.0),
        count_criterion(TWO_RESISTORS, "resistor", 0, None),
    ]);
    let result = evaluate_checks(divider_with(checks, &divider_attempt("correct")));

    assert_eq!(verdict_of(&result, READING), CheckVerdict::NotEvaluated);
    assert_eq!(verdict_of(&result, STABLE), CheckVerdict::NotEvaluated);
    assert_eq!(verdict_of(&result, TWO_RESISTORS), CheckVerdict::Pass);
    assert_eq!(result.outcome, CheckOutcome::Partial);
    assert!(result.diagnostics.is_empty());
}

fn divider_with(checks: Value, student: &[Part]) -> CheckRequest {
    request(checks, &[V1, VM1], &[source(10.0), voltmeter()], student)
}

#[test]
fn a_signal_with_the_wrong_unit_cannot_be_evaluated() {
    let mut criterion = value_criterion(READING, VM1, 0.0, 2.5, 0.025);
    criterion["signal"]["unit"] = json!("amp");
    let result = evaluate_checks(divider_with(
        json!([criterion]),
        &divider_attempt("correct"),
    ));

    assert_eq!(verdict_of(&result, READING), CheckVerdict::NotEvaluated);
    assert_eq!(result.outcome, CheckOutcome::Failed);
}

#[test]
fn a_criterion_that_cannot_be_evaluated_is_a_structured_verdict_not_an_error() {
    let key = format!("{VM1}:voltage");
    let checks = json!([
        // Not a multiple of the public step, past 60 s, and negative.
        value_criterion(
            "00000000-0000-4000-8000-0000000000d1",
            VM1,
            0.0005,
            2.5,
            0.025
        ),
        value_criterion(
            "00000000-0000-4000-8000-0000000000d2",
            VM1,
            60.001,
            2.5,
            0.025
        ),
        value_criterion(
            "00000000-0000-4000-8000-0000000000d3",
            VM1,
            -0.001,
            2.5,
            0.025
        ),
        // A negative tolerance.
        value_criterion("00000000-0000-4000-8000-0000000000d4", VM1, 0.0, 2.5, -0.1),
        // A window that ends before it starts, and bounds that are reversed.
        range_criterion(
            "00000000-0000-4000-8000-0000000000d5",
            &key,
            0.01,
            0.0,
            2.4,
            2.6
        ),
        range_criterion(
            "00000000-0000-4000-8000-0000000000d6",
            &key,
            0.0,
            0.01,
            2.6,
            2.4
        ),
        // A count bound that is reversed, and one past the limit.
        count_criterion(
            "00000000-0000-4000-8000-0000000000d7",
            "resistor",
            3,
            Some(2)
        ),
        count_criterion(
            "00000000-0000-4000-8000-0000000000d8",
            "resistor",
            1001,
            None
        ),
        // The criterion that is fine.
        value_criterion("00000000-0000-4000-8000-0000000000d9", VM1, 0.0, 2.5, 0.025),
    ]);
    let result = evaluate_checks(divider_with(checks, &divider_attempt("correct")));

    for suffix in 1..=8 {
        let id = format!("00000000-0000-4000-8000-0000000000d{suffix}");
        assert_eq!(
            verdict_of(&result, &id),
            CheckVerdict::NotEvaluated,
            "criterion d{suffix}"
        );
    }
    assert_eq!(
        verdict_of(&result, "00000000-0000-4000-8000-0000000000d9"),
        CheckVerdict::Pass
    );
    assert_eq!((result.counts.pass, result.counts.not_evaluated), (1, 8));
    assert_eq!(result.outcome, CheckOutcome::Partial);
}

#[test]
fn a_result_never_carries_a_number_that_is_not_finite() {
    let mut request = divider_with(
        json!([
            value_criterion(READING, VM1, 0.0, 2.5, 0.025),
            range_criterion(STABLE, &format!("{VM1}:voltage"), 0.0, 0.01, 2.4, 2.6),
        ]),
        &divider_attempt("correct"),
    );
    // Rust callers can build what JSON cannot spell.
    if let open_circuit_engine::CheckCriterion::Value {
        at_seconds,
        expected,
        tolerance_abs,
        tolerance_rel,
        ..
    } = &mut request.checks[0]
    {
        *at_seconds = f64::NAN;
        *expected = f64::INFINITY;
        *tolerance_abs = f64::NEG_INFINITY;
        *tolerance_rel = f64::NAN;
    }
    if let open_circuit_engine::CheckCriterion::Range { min, max, .. } = &mut request.checks[1] {
        *min = f64::NAN;
        *max = f64::INFINITY;
    }
    let result = evaluate_checks(request);

    assert_eq!(verdict_of(&result, READING), CheckVerdict::NotEvaluated);
    assert_eq!(verdict_of(&result, STABLE), CheckVerdict::NotEvaluated);
    // `serde_json` writes a non-finite number as `null`; none may be in the result.
    let text = serde_json::to_string(&result).unwrap();
    assert!(!text.contains("null"), "{text}");
}

#[test]
fn only_the_first_64_criteria_are_evaluated() {
    let checks = (0..65)
        .map(|index| {
            count_criterion(
                &format!("00000000-0000-4000-8000-{index:012x}"),
                "resistor",
                0,
                None,
            )
        })
        .collect::<Vec<_>>();
    let result = evaluate_checks(divider_with(json!(checks), &divider_attempt("correct")));

    assert_eq!(result.criteria.len(), 65);
    assert_eq!((result.counts.pass, result.counts.not_evaluated), (64, 1));
    assert_eq!(
        verdict_of(&result, &format!("00000000-0000-4000-8000-{:012x}", 64)),
        CheckVerdict::NotEvaluated
    );
}

#[test]
fn component_counts_are_read_from_the_schematic_and_name_the_counted_parts() {
    let checks = json!([
        count_criterion(READING, "resistor", 2, None),
        count_criterion(STABLE, "resistor", 0, Some(1)),
        count_criterion(TWO_RESISTORS, "capacitor", 0, Some(0)),
        count_criterion(TRIPS, "voltmeter", 1, Some(1)),
    ]);
    let result = evaluate_checks(divider_with(checks, &divider_attempt("correct")));

    let counted = |id: &str| {
        result
            .criteria
            .iter()
            .find_map(|criterion| match criterion {
                CriterionResult::ComponentCount {
                    id: found,
                    measured,
                    references,
                    ..
                } if found.as_str() == id => Some((*measured, references.clone())),
                _ => None,
            })
            .unwrap()
    };
    let (measured, references) = counted(READING);
    assert_eq!(measured, Some(2));
    assert_eq!(
        references,
        [R_TOP, R_BOTTOM]
            .iter()
            .map(|id| DiagnosticReference::Component {
                component_id: serde_json::from_value(json!(id)).unwrap()
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(verdict_of(&result, READING), CheckVerdict::Pass);
    assert_eq!(verdict_of(&result, STABLE), CheckVerdict::Fail);
    assert_eq!(counted(TWO_RESISTORS), (Some(0), Vec::new()));
    assert_eq!(verdict_of(&result, TWO_RESISTORS), CheckVerdict::Pass);
    assert_eq!(verdict_of(&result, TRIPS), CheckVerdict::Pass);
}

#[test]
fn the_outcome_follows_the_precedence_of_the_contract() {
    let outcome_of =
        |checks: Value, student: &[Part]| evaluate_checks(divider_with(checks, student));
    let pass = count_criterion(READING, "resistor", 2, Some(2));
    let fail = count_criterion(STABLE, "resistor", 5, None);
    let unevaluable = value_criterion(TWO_RESISTORS, VM1, 0.0005, 2.5, 0.025);
    let correct = divider_attempt("correct");

    assert_eq!(
        outcome_of(json!([]), &correct).outcome,
        CheckOutcome::Passed
    );
    assert_eq!(
        outcome_of(json!([pass.clone()]), &correct).outcome,
        CheckOutcome::Passed
    );
    assert_eq!(
        outcome_of(json!([pass.clone(), fail.clone()]), &correct).outcome,
        CheckOutcome::Partial
    );
    assert_eq!(
        outcome_of(json!([pass.clone(), unevaluable.clone()]), &correct).outcome,
        CheckOutcome::Partial
    );
    assert_eq!(
        outcome_of(json!([fail.clone()]), &correct).outcome,
        CheckOutcome::Failed
    );
    assert_eq!(
        outcome_of(json!([unevaluable]), &correct).outcome,
        CheckOutcome::Failed
    );
    // No criteria at all: a circuit that does not compile is still `notEvaluated`.
    assert_eq!(
        outcome_of(json!([]), &divider_attempt("source-shorted")).outcome,
        CheckOutcome::NotEvaluated
    );
    assert_eq!(
        outcome_of(json!([pass, fail]), &divider_attempt("source-shorted")).outcome,
        CheckOutcome::NotEvaluated
    );
}

fn locked_result(student: &[Part], locked: &[&str]) -> (CheckVerdict, Vec<String>) {
    let result = evaluate_checks(request(
        json!([]),
        locked,
        &[source(10.0), voltmeter()],
        student,
    ));
    let references = result
        .locked
        .references
        .iter()
        .map(|reference| match reference {
            DiagnosticReference::Component { component_id } => component_id.as_str().to_owned(),
            other => panic!("a component reference expected, found {other:?}"),
        })
        .collect();
    (result.locked.verdict, references)
}

#[test]
fn the_locked_criterion_checks_existence_type_and_parameters_but_not_position() {
    let intact = [source(10.0), voltmeter()];
    assert_eq!(
        locked_result(&intact, &[V1, VM1]),
        (CheckVerdict::Pass, vec![])
    );

    // Moved and rotated: allowed.
    let mut moved = intact.clone();
    moved[0].0["position"] = json!({ "x": 9, "y": 9 });
    moved[0].0["rotationDeg"] = json!(90);
    assert_eq!(
        locked_result(&moved, &[V1, VM1]),
        (CheckVerdict::Pass, vec![])
    );

    // A parameter changed.
    let mut changed = intact.clone();
    changed[1].0["parameters"]["inputResistanceOhms"] = json!(1e6);
    assert_eq!(
        locked_result(&changed, &[V1, VM1]),
        (CheckVerdict::Fail, vec![VM1.to_owned()])
    );

    // Deleted.
    assert_eq!(
        locked_result(&intact[..1], &[V1, VM1]),
        (CheckVerdict::Fail, vec![VM1.to_owned()])
    );

    // The same ID with another type.
    let swapped = [resistor(V1, 10.0, (0, 0), (2, 2)), voltmeter()];
    assert_eq!(
        locked_result(&swapped, &[V1, VM1]),
        (CheckVerdict::Fail, vec![V1.to_owned()])
    );

    // A locked part that is not in the starting circuit cannot be compared; a repeated entry
    // is reported once.
    let unknown = "00000000-0000-4000-8000-0000000000ee";
    assert_eq!(
        locked_result(&intact, &[unknown, V1, unknown]),
        (CheckVerdict::Fail, vec![unknown.to_owned()])
    );

    // Only the locked parts matter: other parts may change freely.
    assert_eq!(
        locked_result(&[source(20.0), voltmeter()], &[VM1]),
        (CheckVerdict::Pass, vec![])
    );
}

#[test]
fn a_check_is_deterministic_and_independent_of_other_checks_and_of_a_live_run() {
    let student = divider_attempt("correct");
    let first = evaluate_checks(divider_request(&student));

    // A live run of the same circuit, advanced meanwhile, does not matter.
    let (schematic_value, geometry_value) = schematic(&student);
    let mut live = Engine::default();
    let compile = serde_json::from_value(json!({
        "schematic": schematic_value, "geometry": geometry_value, "documentRevision": "live",
    }))
    .unwrap();
    let open_circuit_engine::CompileResult::Success { run_id, .. } = live.compile(compile) else {
        panic!("the circuit should compile");
    };
    live.advance(open_circuit_engine::AdvanceRequest {
        run_id: run_id.clone(),
        step_count: 3,
    })
    .unwrap();
    let live_before = live.latest_snapshot().cloned();

    // Another check in between, with other criteria.
    let _ = evaluate_checks(divider_with(
        json!([value_criterion(READING, VM1, 0.2, 2.5, 0.025)]),
        &student,
    ));
    let second = evaluate_checks(divider_request(&student));

    assert_eq!(first, second);
    assert_eq!(live.latest_snapshot().cloned(), live_before);
    assert_eq!(live.latest_snapshot().unwrap().metadata.simulation_step, 3);
}

#[test]
fn a_value_does_not_depend_on_how_far_the_check_runs() {
    // The value at 10 ms does not depend on how far the check runs.
    let key = format!("{CAPACITOR}:voltage");
    let short = evaluate_checks(rc_request(json!([value_criterion(
        READING, CAPACITOR, 0.01, 0.0, 1e9
    )])));
    let long = evaluate_checks(rc_request(json!([
        value_criterion(READING, CAPACITOR, 0.01, 0.0, 1e9),
        range_criterion(STABLE, &key, 0.0, 0.2, -1.0, 100.0),
    ])));

    assert_eq!(
        measured_value(&short, READING),
        measured_value(&long, READING)
    );
    assert_close(
        measured_value(&short, READING).unwrap(),
        rc_voltage(0.01),
        0.01,
        "10 ms",
    );
}

#[test]
fn the_wire_format_is_the_contracts_and_rejects_unknown_fields() {
    let result = evaluate_checks(divider_request(&divider_attempt("correct")));
    let value = serde_json::to_value(&result).unwrap();

    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        [
            "counts",
            "criteria",
            "diagnostics",
            "documentRevision",
            "locked",
            "outcome",
            "outcomeKey"
        ]
    );
    assert_eq!(value["outcome"], "passed");
    assert_eq!(
        value["counts"],
        json!({ "pass": 3, "fail": 0, "notEvaluated": 0 })
    );
    assert_eq!(value["locked"]["verdict"], "pass");
    assert_eq!(value["locked"]["messageKey"], "check.locked.pass");

    let reading = &value["criteria"][0];
    let mut keys = reading
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        [
            "atSeconds",
            "expected",
            "id",
            "kind",
            "measured",
            "messageKey",
            "references",
            "toleranceAbs",
            "toleranceRel",
            "unit",
            "verdict"
        ]
    );
    assert_eq!(reading["kind"], "value");
    assert_eq!(reading["verdict"], "pass");
    assert_eq!(reading["unit"], "volt");
    assert_eq!(reading["messageKey"], "check.value.pass");
    assert_eq!(value["criteria"][1]["kind"], "range");
    assert_eq!(value["criteria"][2]["kind"], "componentCount");
    assert_eq!(value["criteria"][2]["measured"], 2);

    // `toleranceRel` and `label` are optional in a request; an unknown field is refused.
    let mut criterion = value_criterion(READING, VM1, 0.0, 2.5, 0.025);
    criterion.as_object_mut().unwrap().remove("toleranceRel");
    criterion["label"] = json!("The reading");
    let parsed: open_circuit_engine::CheckCriterion = serde_json::from_value(criterion).unwrap();
    assert!(matches!(
        parsed,
        open_circuit_engine::CheckCriterion::Value { tolerance_rel, .. } if tolerance_rel == 0.0
    ));
    let mut extra = value_criterion(READING, VM1, 0.0, 2.5, 0.025);
    extra["weight"] = json!(2);
    assert!(serde_json::from_value::<open_circuit_engine::CheckCriterion>(extra).is_err());
}
