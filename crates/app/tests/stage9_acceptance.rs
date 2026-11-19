//! Replays the editor-pinned Stage 9 requests through the real application bridge.

mod common;

use common::{Capture, Run, assert_close, value_of};
use open_circuit_app_lib::project::ProjectSession;
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, ParameterId, ResultQuantity, SetParameterResult,
    StableId,
};
use serde_json::Value;
use std::time::Instant;

const ANALYTICAL: &str = include_str!("../../../tests/golden/stage-9-analytical.expected.json");
const NGSPICE: &str = include_str!("../../../tests/golden/stage-9-ngspice.expected.json");

const CASES: [(&str, &str, u32); 6] = [
    (
        "rc-50-phase-0",
        include_str!("../../../tests/golden/stage-9-drawn-rc-50-phase-0.request.json"),
        220,
    ),
    (
        "wattmeter-rl",
        include_str!("../../../tests/golden/stage-9-drawn-wattmeter-rl.request.json"),
        300,
    ),
    (
        "star-balanced-resistive-neutral",
        include_str!(
            "../../../tests/golden/stage-9-drawn-star-balanced-resistive-neutral.request.json"
        ),
        120,
    ),
    (
        "star-unbalanced-neutral",
        include_str!("../../../tests/golden/stage-9-drawn-star-unbalanced-neutral.request.json"),
        120,
    ),
    (
        "star-unbalanced-no-neutral",
        include_str!("../../../tests/golden/stage-9-drawn-star-unbalanced-no-neutral.request.json"),
        120,
    ),
    (
        "delta-load-on-delta-source-balanced",
        include_str!(
            "../../../tests/golden/stage-9-drawn-delta-load-on-delta-source-balanced.request.json"
        ),
        120,
    ),
];

fn oracle(document: &Value, name: &str) -> Value {
    document["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == name)
        .unwrap()
        .clone()
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a numeric reference value")
}

fn value(run: &Run, step: u64, key: &str) -> f64 {
    let slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap();
    run.frame_at(step).0.values[slot.offset as usize]
}

fn component(run: &Run, step: u64, id: &StableId, quantity: ResultQuantity) -> f64 {
    value_of(&run.layout, &run.frame_at(step).0, id.as_str(), quantity)
}

fn sample_rows(reference: &Value) -> impl Iterator<Item = (u64, &Value)> {
    reference["observations"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| {
            let millis = number(&row["timeSeconds"]) * 1000.0;
            ((millis - millis.round()).abs() < 1e-8 && millis >= 1.0)
                .then_some((millis.round() as u64, row))
        })
}

fn check_rc(run: &Run, ids: &[StableId], analytical: &Value, ngspice: &Value) {
    let mut max_error: f64 = 0.0;
    let tolerance = number(&analytical["summary"]["engineTolerance"]["startupSignalAbsolute"]);
    for reference in [analytical, ngspice] {
        for (step, row) in sample_rows(reference) {
            let expected = number(&row["nodeVoltagesVolts"]["cap"]);
            let actual = component(run, step, &ids[2], ResultQuantity::Voltage);
            assert_close(actual, expected, tolerance, "RC capacitor voltage");
            max_error = max_error.max((actual - expected).abs());
        }
    }
    let actual = component(run, 5, &ids[2], ResultQuantity::Voltage);
    println!("rc: cap(5 ms)={actual:.12} V; max sampled deviation={max_error:.12} V");
}

fn check_wattmeter(run: &Run, ids: &[StableId], analytical: &Value, ngspice: &Value) {
    let meter = ids[1].as_str();
    assert_eq!(value(run, 1, &format!("{meter}/valid")), 0.0);
    assert_eq!(value(run, 300, &format!("{meter}/valid")), 1.0);
    for (slot, field) in [
        ("rms_voltage", "rmsVoltageVolts"),
        ("rms_current", "rmsCurrentAmps"),
        ("real_power", "realPowerWatts"),
        ("reactive_power", "reactivePowerVars"),
        ("apparent_power", "apparentPowerVoltAmps"),
        ("power_factor", "powerFactor"),
        ("phase_angle", "phaseAngleDegrees"),
    ] {
        let actual = value(run, 300, &format!("{meter}/{slot}"));
        let expected = number(&analytical["summary"][field]);
        let tolerance = match slot {
            "power_factor" => 0.002,
            "phase_angle" => 0.3,
            _ => expected.abs() * 0.002,
        };
        for reference in [analytical, ngspice] {
            assert_close(
                actual,
                number(&reference["summary"][field]),
                tolerance,
                slot,
            );
        }
        println!(
            "wattmeter: {slot}={actual:.12}; analytical={expected:.12}; tolerance={tolerance:.12}"
        );
    }
}

fn check_three_phase(run: &Run, ids: &[StableId], name: &str, analytical: &Value, ngspice: &Value) {
    let delta = name.starts_with("delta-");
    let currents = if delta {
        ["Rload12", "Rload23", "Rload31"]
    } else {
        ["Rphase1", "Rphase2", "Rphase3"]
    };
    let mut max_error: f64 = 0.0;
    let current_limit =
        0.005 * 2.0_f64.sqrt() * number(&analytical["summary"]["sourceRmsVolts"]) / 100.0;
    for reference in [analytical, ngspice] {
        for (step, row) in sample_rows(reference) {
            let expected_currents =
                currents.map(|branch| number(&row["branchCurrentsAmps"][branch]));
            let actual_currents =
                [1, 2, 3].map(|index| component(run, step, &ids[index], ResultQuantity::Current));
            for (actual, expected) in actual_currents.into_iter().zip(expected_currents) {
                assert_close(actual, expected, current_limit, name);
                max_error = max_error.max((actual - expected).abs());
            }
            if delta {
                let node = &row["nodeVoltagesVolts"];
                let l1 = number(&node["l1"]);
                let l2 = number(&node["l2"]);
                let source = ids[0].as_str();
                for (line, expected) in [("l12", l1 - l2), ("l23", l2), ("l31", -l1)] {
                    let actual = value(run, step, &format!("{source}/{line}/voltage"));
                    assert_close(actual, expected, 0.005 * 2.0_f64.sqrt() * 400.0, line);
                }
            } else {
                let neutral_voltage = row["nodeVoltagesVolts"]["nload"].as_f64().unwrap_or(0.0);
                let source = ids[0].as_str();
                let l1 = value(run, step, &format!("{source}/l1/voltage"));
                let load_voltage = component(run, step, &ids[1], ResultQuantity::Voltage);
                assert_close(
                    l1 - load_voltage,
                    neutral_voltage,
                    0.005 * 2.0_f64.sqrt() * 230.0,
                    "neutral voltage",
                );
                let neutral = value(run, step, &format!("{source}/n/current"));
                assert_close(neutral, actual_currents.iter().sum(), 1e-9, "neutral KCL");
            }
        }
    }
    let actual = [1, 2, 3].map(|index| component(run, 105, &ids[index], ResultQuantity::Current));
    let expected = analytical["observations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| number(&row["timeSeconds"]) == 0.105)
        .unwrap();
    let expected = currents.map(|branch| number(&expected["branchCurrentsAmps"][branch]));
    println!(
        "{name}: I(105 ms)={actual:?} A; analytical={expected:?} A; max sampled deviation={max_error:.12} A; tolerance={current_limit:.12} A"
    );
}

fn verify_reopen_and_replay(request: CompileRequest, steps: u32) -> Run {
    let mut first = Run::start_every_step(request.clone());
    assert_eq!(first.advance(steps), u64::from(steps));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stage-9.ocircuit");
    let project = ProjectSession::default();
    project.save_to(&path, &request.schematic).unwrap();
    let reopened = project.load(&path).unwrap();
    let drawn_input = serde_json::to_value(&request.schematic).unwrap();
    assert_eq!(serde_json::to_value(&reopened).unwrap(), drawn_input);
    let stored: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(stored, drawn_input);

    let mut second = Run::start_every_step(CompileRequest {
        schematic: reopened,
        ..request
    });
    assert_ne!(first.run_id, second.run_id);
    assert_eq!(second.advance(steps), u64::from(steps));
    assert_eq!(
        first.initial_snapshot.values,
        second.initial_snapshot.values
    );
    assert_eq!(first.delivered.len(), steps as usize);
    assert_eq!(second.delivered.len(), steps as usize);
    for ((a, _), (b, _)) in first.delivered.iter().zip(&second.delivered) {
        assert_eq!(a.metadata.simulation_step, b.metadata.simulation_step);
        assert!(
            a.values
                .iter()
                .map(|n| n.to_bits())
                .eq(b.values.iter().map(|n| n.to_bits()))
        );
    }
    let keys = [first.layout.slots[0].key.clone()];
    assert_eq!(
        first.samples(&keys, 1, u64::from(steps)),
        second.samples(&keys, 1, u64::from(steps))
    );
    first
}

#[test]
fn drawn_circuits_match_both_references_and_repeat_after_reopening() {
    let analytical: Value = serde_json::from_str(ANALYTICAL).unwrap();
    let ngspice: Value = serde_json::from_str(NGSPICE).unwrap();
    for (name, input, steps) in CASES {
        println!("replaying {name}");
        let request: CompileRequest = serde_json::from_str(input).unwrap();
        let ids: Vec<_> = request
            .schematic
            .components
            .iter()
            .map(|part| part.id().clone())
            .collect();
        let run = verify_reopen_and_replay(request, steps);
        let analytical = oracle(&analytical, name);
        let ngspice = oracle(&ngspice, name);
        match name {
            "rc-50-phase-0" => check_rc(&run, &ids, &analytical, &ngspice),
            "wattmeter-rl" => check_wattmeter(&run, &ids, &analytical, &ngspice),
            _ => check_three_phase(&run, &ids, name, &analytical, &ngspice),
        }
    }
}

#[test]
fn the_drawn_wattmeter_preserves_history_on_refusal_and_invalidates_on_frequency_change() {
    let request: CompileRequest = serde_json::from_str(CASES[1].1).unwrap();
    let mut run = Run::start_every_step(request.clone());
    run.advance(60);
    let flag = format!("{}/valid", request.schematic.components[1].id().as_str());
    let keys = [flag];
    let (before, _, stale) = run.read(&keys, 59, 60);
    let source = request.schematic.components[0].id().as_str();
    let (_, SetParameterResult::Failure { diagnostics }) =
        run.set_parameter(source, ParameterId::FrequencyHz, 500.0)
    else {
        panic!("a 500 Hz edit must be refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    assert_eq!(run.read(&keys, 59, 60).0.points, before.points);
    let (_, updated) = run.set_parameter(source, ParameterId::FrequencyHz, 60.0);
    assert!(matches!(updated, SetParameterResult::Success { .. }));
    assert_eq!(
        run.read(&keys, 59, 60).0.points,
        vec![(59, vec![(1.0, 1.0)]), (60, vec![(0.0, 0.0)])]
    );
    run.advance(20);
    assert_eq!(
        run.read(&keys, 80, 80).0.points,
        vec![(80, vec![(1.0, 1.0)])]
    );

    let CompileResult::Success { run_id, .. } = run.core.compile(request) else {
        panic!("the original drawing must recompile");
    };
    assert_ne!(run_id, run.run_id);
    let diagnostics = run
        .core
        .read_traces(stale, Box::new(Capture::default()), Instant::now())
        .unwrap_err();
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);
}
