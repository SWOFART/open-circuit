//! Issue #114: the semiconductor circuits that the editor commands and the real symbol geometry
//! draw from blank documents (`src/store/stage7Drawing.test.ts`) run on the real Engine behind
//! `SimulationCore`, and their frames agree with the answers of the model equations in
//! `tests/golden/stage-7-*` (never Engine output), within the manifest's tolerances.

mod common;

use common::{Run, assert_close};
use open_circuit_engine::CompileRequest;
use serde_json::Value;

const DIODE_FORWARD: &str =
    include_str!("../../../tests/golden/stage-7-drawn-diode-forward.request.json");
const DIODE_REVERSE: &str =
    include_str!("../../../tests/golden/stage-7-drawn-diode-reverse.request.json");
const NPN_ACTIVE: &str =
    include_str!("../../../tests/golden/stage-7-drawn-npn-active.request.json");
const PNP_ACTIVE: &str =
    include_str!("../../../tests/golden/stage-7-drawn-pnp-active.request.json");
const ANALYTICAL: &str = include_str!("../../../tests/golden/stage-7-analytical.expected.json");
const MANIFEST: &str = include_str!("../../../tests/golden/stage-7-manifest.json");

fn num(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

fn answer(id: &str) -> Value {
    let all: Value = serde_json::from_str(ANALYTICAL).unwrap();
    all["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))["contract"]
        .clone()
}

fn tolerance(id: &str) -> Value {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    manifest["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"] == id)
        .unwrap_or_else(|| panic!("no fixture {id}"))["engineTolerance"]
        .clone()
}

fn close(actual: f64, expected: f64, limits: &Value, volts: bool, what: &str) {
    if volts {
        assert_close(actual, expected, num(&limits["voltsAbsolute"]), what);
    } else {
        let relative = limits.get("ampsRelative").map_or(0.0, num);
        let absolute = limits
            .get("ampsAbsolute")
            .or_else(|| limits.get("ampsAbsoluteFloor"))
            .map_or(0.0, num);
        assert_close(
            actual,
            expected,
            relative * actual.abs().max(expected.abs()) + absolute,
            what,
        );
    }
}

/// The identifier of the one component of a type in a drawn request.
fn device(request: &CompileRequest, type_id: &str) -> String {
    let json = serde_json::to_value(request).unwrap();
    json["schematic"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["typeId"] == type_id)
        .unwrap_or_else(|| panic!("no {type_id}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn value(run: &Run, step: u64, key: &str) -> f64 {
    let (frame, _bytes) = run.frame_at(step);
    let slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot {key}"));
    frame.values[slot.offset as usize]
}

fn diode_case(text: &str, id: &str) {
    let request: CompileRequest = serde_json::from_str(text).unwrap();
    let diode = device(&request, "diode");
    let mut run = Run::start_every_step(request);
    let step = run.advance(5);
    let expected = answer(id);
    let limits = tolerance(id);

    close(
        value(&run, step, &format!("{diode}:voltage")),
        num(&expected["diodeVoltageVolts"]),
        &limits,
        true,
        "diode voltage",
    );
    close(
        value(&run, step, &format!("{diode}:current")),
        num(&expected["diodeCurrentAmps"]),
        &limits,
        false,
        "diode current",
    );
}

fn transistor_case(text: &str, type_id: &str, id: &str) {
    let request: CompileRequest = serde_json::from_str(text).unwrap();
    let transistor = device(&request, type_id);
    let mut run = Run::start_every_step(request);
    let step = run.advance(5);
    let expected = answer(id);
    let limits = tolerance(id);
    let read = |name: &str| value(&run, step, &format!("{transistor}/{name}"));

    close(
        read("base_emitter/voltage"),
        num(&expected["baseVoltageVolts"]),
        &limits,
        true,
        "V_BE",
    );
    close(
        read("collector_emitter/voltage"),
        num(&expected["loadNodeVoltageVolts"]),
        &limits,
        true,
        "V_CE",
    );
    close(
        read("collector/current"),
        num(&expected["collectorCurrentAmps"]),
        &limits,
        false,
        "i_C",
    );
    close(
        read("base/current"),
        num(&expected["baseCurrentAmps"]),
        &limits,
        false,
        "i_B",
    );
    close(
        read("emitter/current"),
        num(&expected["emitterCurrentAmps"]),
        &limits,
        false,
        "i_E",
    );
}

#[test]
fn the_drawn_forward_diode_agrees_with_the_model_equations() {
    diode_case(DIODE_FORWARD, "diode-forward");
}

#[test]
fn the_drawn_reverse_diode_agrees_with_the_model_equations() {
    diode_case(DIODE_REVERSE, "diode-reverse");
}

#[test]
fn the_drawn_npn_stage_agrees_with_the_model_equations() {
    transistor_case(NPN_ACTIVE, "bjt_npn", "npn-active");
}

#[test]
fn the_drawn_pnp_stage_agrees_with_the_model_equations() {
    transistor_case(PNP_ACTIVE, "bjt_pnp", "pnp-active");
}
