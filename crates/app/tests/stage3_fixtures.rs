//! The documented Stage 3 fixtures (docs/physics/models.md, "Stage 3 acceptance"), run on
//! the real Engine and read back through the bridge's trace path: the Engine's ring of
//! samples, the `OCT1` encoder, and bounded history reads. The Engine's own tests in
//! `crates/engine/tests` check the same fixtures against ngspice; this file checks that
//! what the oscilloscope receives is what the Engine computed, in every public step.

mod common;

use common::{Run, assert_close, slot_key, value_of};
use open_circuit_engine::{CompileRequest, ResultQuantity};
use serde_json::Value;

const RC_CHARGE: (&str, &str, &str) = (
    include_str!("../../../tests/golden/stage-3-rc-charge.schematic.json"),
    include_str!("../../../tests/golden/stage-3-rc-charge.geometry.json"),
    include_str!("../../../tests/golden/stage-3-rc-charge.expected.json"),
);
const RC_DISCHARGE: (&str, &str, &str) = (
    include_str!("../../../tests/golden/stage-3-rc-discharge.schematic.json"),
    include_str!("../../../tests/golden/stage-3-rc-discharge.geometry.json"),
    include_str!("../../../tests/golden/stage-3-rc-discharge.expected.json"),
);
const RL_RISE: (&str, &str, &str) = (
    include_str!("../../../tests/golden/stage-3-rl-rise.schematic.json"),
    include_str!("../../../tests/golden/stage-3-rl-rise.geometry.json"),
    include_str!("../../../tests/golden/stage-3-rl-rise.expected.json"),
);
const RL_DECAY: (&str, &str, &str) = (
    include_str!("../../../tests/golden/stage-3-rl-decay.schematic.json"),
    include_str!("../../../tests/golden/stage-3-rl-decay.geometry.json"),
    include_str!("../../../tests/golden/stage-3-rl-decay.expected.json"),
);
const DC_INDUCTOR: (&str, &str) = (
    include_str!("../../../tests/golden/stage-3-dc-inductor.schematic.json"),
    include_str!("../../../tests/golden/stage-3-dc-inductor.geometry.json"),
);
const LOSSLESS_LC: (&str, &str, &str) = (
    include_str!("../../../tests/golden/stage-3-lossless-lc.schematic.json"),
    include_str!("../../../tests/golden/stage-3-lossless-lc.geometry.json"),
    include_str!("../../../tests/golden/stage-3-lossless-lc.expected.json"),
);

const RC_CHARGE_CAPACITOR: &str = "00000000-0000-4000-8000-000000000003";
const RC_DISCHARGE_CAPACITOR: &str = "00000000-0000-4000-8000-000000000002";
const RL_RISE_INDUCTOR: &str = "00000000-0000-4000-8000-000000000003";
const RL_DECAY_INDUCTOR: &str = "00000000-0000-4000-8000-000000000002";
const DC_INDUCTOR_ID: &str = "00000000-0000-4000-8000-000000000003";
const LC_CAPACITOR: &str = "00000000-0000-4000-8000-000000000001";
const LC_INDUCTOR: &str = "00000000-0000-4000-8000-000000000002";

fn request(schematic: &str, geometry: &str) -> CompileRequest {
    CompileRequest {
        schematic: serde_json::from_str(schematic).expect("a schematic fixture"),
        geometry: serde_json::from_str(geometry).expect("a geometry fixture"),
        document_revision: "rev-1".to_owned(),
        faults: None,
    }
}

/// Runs a first-order fixture for 50 ms and compares its state quantity with the
/// analytical curve at every public step, and with the table at 10, 20, and 30 ms.
fn assert_first_order(
    fixture: (&str, &str, &str),
    component: &str,
    quantity: ResultQuantity,
    value_key: &str,
    tolerance_key: &str,
    curve: impl Fn(f64) -> f64,
    what: &str,
) {
    let expected: Value = serde_json::from_str(fixture.2).expect("an expected-value file");
    let tolerance = expected[tolerance_key].as_f64().unwrap();
    let mut run = Run::start(request(fixture.0, fixture.1));

    assert!(run.advance(60) >= 50, "{what}: the run reaches 50 ms");

    let key = slot_key(&run.layout, component, quantity);
    let samples = run.samples(std::slice::from_ref(&key), 0, 50);

    assert_eq!(samples.len(), 51, "{what}: every public step is stored");
    for (step, values) in &samples {
        let seconds = *step as f64 * 0.001;

        assert_close(
            values[0],
            curve(seconds),
            tolerance,
            &format!("{what} at {step} ms"),
        );
    }
    for reference in expected["analytical"].as_array().unwrap() {
        let step = (reference["timeSeconds"].as_f64().unwrap() / 0.001).round() as usize;

        assert_close(
            samples[step].1[0],
            reference[value_key].as_f64().unwrap(),
            tolerance,
            &format!("{what} at {step} ms (table)"),
        );
    }
}

#[test]
fn rc_charge_and_discharge_follow_the_analytical_curves_through_the_trace_path() {
    assert_first_order(
        RC_CHARGE,
        RC_CHARGE_CAPACITOR,
        ResultQuantity::Voltage,
        "voltageVolts",
        "analyticalToleranceVolts",
        |t| 10.0 * (1.0 - (-t / 0.01).exp()),
        "RC charge",
    );
    assert_first_order(
        RC_DISCHARGE,
        RC_DISCHARGE_CAPACITOR,
        ResultQuantity::Voltage,
        "voltageVolts",
        "analyticalToleranceVolts",
        |t| 10.0 * (-t / 0.01).exp(),
        "RC discharge",
    );
}

#[test]
fn rl_rise_and_decay_follow_the_analytical_curves_through_the_trace_path() {
    assert_first_order(
        RL_RISE,
        RL_RISE_INDUCTOR,
        ResultQuantity::Current,
        "currentAmps",
        "analyticalToleranceAmps",
        |t| 1.0 - (-t / 0.01).exp(),
        "RL rise",
    );
    assert_first_order(
        RL_DECAY,
        RL_DECAY_INDUCTOR,
        ResultQuantity::Current,
        "currentAmps",
        "analyticalToleranceAmps",
        |t| (-t / 0.01).exp(),
        "RL decay",
    );
}

#[test]
fn an_unspecified_state_starts_from_the_dc_operating_point_and_holds_it() {
    // The inductor fixture has no initial current: 5 V across 1 kOhm, the inductor a short.
    let mut inductor = Run::start(request(DC_INDUCTOR.0, DC_INDUCTOR.1));
    inductor.advance(1000);
    let keys = [
        slot_key(&inductor.layout, DC_INDUCTOR_ID, ResultQuantity::Current),
        slot_key(&inductor.layout, DC_INDUCTOR_ID, ResultQuantity::Voltage),
    ];

    for (step, values) in inductor.samples(&keys, 0, 1000) {
        assert_close(values[0], 0.005, 1e-9, &format!("L1 current at {step} ms"));
        assert_close(values[1], 0.0, 1e-6, &format!("L1 voltage at {step} ms"));
    }

    // The RC charge fixture with its explicit 0 V taken away: the capacitor starts at the
    // 10 V the source holds across it, and no current flows.
    let mut schematic: Value = serde_json::from_str(RC_CHARGE.0).unwrap();
    schematic["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == "capacitor")
        .unwrap()["parameters"]
        .as_object_mut()
        .unwrap()
        .remove("initialVoltageVolts");
    let mut capacitor = Run::start(request(&schematic.to_string(), RC_CHARGE.1));
    capacitor.advance(1000);
    let keys = [
        slot_key(
            &capacitor.layout,
            RC_CHARGE_CAPACITOR,
            ResultQuantity::Voltage,
        ),
        slot_key(
            &capacitor.layout,
            RC_CHARGE_CAPACITOR,
            ResultQuantity::Current,
        ),
    ];

    for (step, values) in capacitor.samples(&keys, 0, 1000) {
        assert_close(values[0], 10.0, 1e-6, &format!("C1 voltage at {step} ms"));
        assert_close(values[1], 0.0, 1e-9, &format!("C1 current at {step} ms"));
    }
}

#[test]
fn the_lossless_lc_circuit_keeps_its_period_and_energy_through_the_trace_path() {
    let expected: Value = serde_json::from_str(LOSSLESS_LC.2).unwrap();
    let mut run = Run::start(request(LOSSLESS_LC.0, LOSSLESS_LC.1));

    // Up to 10 s of history is kept for this circuit, so step 0 is still stored at 9.5 s.
    assert!(run.advance(9_500) >= 9_000, "the run reaches about 9.5 s");

    let keys = [
        slot_key(&run.layout, LC_CAPACITOR, ResultQuantity::Voltage),
        slot_key(&run.layout, LC_INDUCTOR, ResultQuantity::Current),
    ];
    let samples = run.samples(&keys, 0, 9_000);
    let energy = expected["storedEnergyJoules"].as_f64().unwrap();
    let energy_tolerance = expected["energyRelativeTolerance"].as_f64().unwrap();

    assert_eq!(samples.len(), 9_001);
    for (step, values) in &samples {
        let stored = 0.5 * 1e-4 * values[0] * values[0] + 0.5 * 0.1 * values[1] * values[1];

        assert!(
            (stored - energy).abs() / energy <= energy_tolerance,
            "step {step}: stored energy {stored} J is outside the relative tolerance"
        );
    }

    // The period, from the first 200 steps with the least-squares estimator the Engine's
    // own tests use for the same fixture.
    let voltages: Vec<f64> = samples.iter().take(200).map(|(_, v)| v[0]).collect();
    let numerator: f64 = (1..=198)
        .map(|index| voltages[index] * (voltages[index + 1] + voltages[index - 1]))
        .sum();
    let denominator: f64 = 2.0
        * voltages[1..=198]
            .iter()
            .map(|sample| sample * sample)
            .sum::<f64>();
    let period =
        2.0 * std::f64::consts::PI * 0.001 / (numerator / denominator).clamp(-1.0, 1.0).acos();
    let expected_period = expected["periodSeconds"].as_f64().unwrap();

    assert!(
        (period - expected_period).abs() / expected_period
            <= expected["periodRelativeTolerance"].as_f64().unwrap(),
        "the period {period} s is outside the tolerance of {expected_period} s"
    );

    // The step-0 values of the fixture: 10 V across C1, no inductor current.
    assert_close(
        value_of(
            &run.layout,
            &run.initial_snapshot,
            LC_CAPACITOR,
            ResultQuantity::Voltage,
        ),
        10.0,
        1e-9,
        "C1 voltage at step 0",
    );
}
