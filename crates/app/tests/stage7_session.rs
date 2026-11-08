//! The Stage 7 bridge (issue #113): the diode and the NPN and PNP transistors run by the real
//! Engine behind `SimulationCore` and `SimulationService`, carried the way the application
//! carries everything: JSON for compile and update results, binary OCF2 frames, OCT1 traces,
//! and the stop notice of docs/contracts/stage-7-nonlinear-circuits.md section 6.3. The circuits
//! and the answers are those of `tests/golden/stage-7-*` (the answers of the equations, never
//! Engine output).

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{Run, assert_close};
use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_app_lib::simulation::{
    FrameSink, SimulationConfig, SimulationCore, SimulationService, SinkClosed,
};
use open_circuit_engine::{
    CompileRequest, CompileResult, ComponentCatalog, DiagnosticCode, DiagnosticReference,
    OperatorPosition, ParameterId, ResultLayout, ResultSnapshot, SetOperatorStateRequest,
    SetOperatorStateResult, SetParameterResult, StableId, StreamNotice, component_catalog,
    decode_project_json,
};
use serde_json::{Value, json};

const ANALYTICAL: &str = include_str!("../../../tests/golden/stage-7-analytical.expected.json");
const MANIFEST: &str = include_str!("../../../tests/golden/stage-7-manifest.json");

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const DEVICE: &str = "00000000-0000-4000-8000-000000000002";
const LOAD: &str = "00000000-0000-4000-8000-000000000003";
const BASE_RESISTOR: &str = "00000000-0000-4000-8000-000000000004";
const BASE_SOURCE: &str = "00000000-0000-4000-8000-000000000005";
const MARKER: &str = "00000000-0000-4000-8000-000000000009";

fn point(x: i32, y: i32) -> Value {
    json!({ "x": x, "y": y })
}

fn component(type_id: &str, id: &str, at: Value, parameters: Value) -> Value {
    json!({ "typeId": type_id, "id": id, "position": at, "rotationDeg": 0, "parameters": parameters })
}

fn pins(id: &str, pins: &[(&str, Value)]) -> Value {
    json!({
        "componentId": id,
        "pins": pins.iter().map(|(pin, at)| json!({ "pinId": pin, "position": at })).collect::<Vec<_>>(),
    })
}

fn request(components: Vec<Value>, geometry: Vec<Value>) -> CompileRequest {
    serde_json::from_value(json!({
        "schematic": {
            "schemaVersion": 1,
            "components": components,
            "wires": [],
            "junctions": [],
            "referenceMarkers": [{ "id": MARKER, "position": point(0, 10) }],
        },
        "geometry": { "components": geometry },
        "documentRevision": "rev-1",
    }))
    .expect("a compile request")
}

fn source(id: &str, volts: f64, positive: Value, negative: Value) -> (Value, Value) {
    (
        component(
            "dc_voltage_source",
            id,
            positive.clone(),
            json!({ "voltageVolts": volts, "internalResistanceOhms": 0.0 }),
        ),
        pins(id, &[("pin1", positive), ("pin2", negative)]),
    )
}

fn resistor(id: &str, ohms: f64, a: Value, b: Value) -> (Value, Value) {
    (
        component("resistor", id, a.clone(), json!({ "resistanceOhms": ohms })),
        pins(id, &[("pin1", a), ("pin2", b)]),
    )
}

/// `diode-forward` (or a variant): a source, a resistor, and the default diode in series.
fn diode_circuit(source_volts: f64, resistance: f64) -> CompileRequest {
    let (ground, top, anode) = (point(0, 10), point(0, 0), point(4, 0));
    let (s, sg) = source(SOURCE, source_volts, top.clone(), ground.clone());
    let (r, rg) = resistor(LOAD, resistance, top, anode.clone());
    let d = component(
        "diode",
        DEVICE,
        anode.clone(),
        json!({ "saturationCurrentAmps": 1e-14, "emissionCoefficient": 1.0 }),
    );
    let dg = pins(DEVICE, &[("anode", anode), ("cathode", ground)]);
    request(vec![s, r, d], vec![sg, rg, dg])
}

/// The common-emitter stage of the transistor fixtures: the emitter at ground, 1 kOhm from the
/// supply to the collector, and a base source through a base resistor. `pnp` negates every
/// source, as the PNP fixtures do.
fn bjt_circuit(pnp: bool, base_source: f64, base_resistor: f64) -> CompileRequest {
    let sign = if pnp { -1.0 } else { 1.0 };
    let (ground, vcc, vbb) = (point(0, 10), point(0, 0), point(0, 20));
    let (collector, base) = (point(8, 0), point(8, 20));
    let (supply, supply_pins) = source(SOURCE, 10.0 * sign, vcc.clone(), ground.clone());
    let (bias, bias_pins) = source(BASE_SOURCE, base_source * sign, vbb.clone(), ground.clone());
    let (rc, rc_pins) = resistor(LOAD, 1000.0, vcc, collector.clone());
    let (rb, rb_pins) = resistor(BASE_RESISTOR, base_resistor, vbb, base.clone());
    let q = component(
        if pnp { "bjt_pnp" } else { "bjt_npn" },
        DEVICE,
        collector.clone(),
        json!({ "saturationCurrentAmps": 1e-14, "forwardBeta": 100.0, "reverseBeta": 1.0 }),
    );
    let q_pins = pins(
        DEVICE,
        &[
            ("collector", collector),
            ("base", base),
            ("emitter", ground),
        ],
    );
    request(
        vec![supply, bias, rc, rb, q],
        vec![supply_pins, bias_pins, rc_pins, rb_pins, q_pins],
    )
}

fn key(layout: &ResultLayout, wanted: &str) -> usize {
    layout
        .slots
        .iter()
        .find(|slot| slot.key == wanted)
        .unwrap_or_else(|| panic!("no slot {wanted}"))
        .offset as usize
}

fn at(layout: &ResultLayout, snapshot: &ResultSnapshot, wanted: &str) -> f64 {
    snapshot.values[key(layout, wanted)]
}

fn analytic(id: &str) -> Value {
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

fn num(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

/// An Engine value against the answer of the equations, with the fixture's tolerance: an
/// absolute tolerance plus a relative one.
fn close_to_answer(actual: f64, answer: f64, tolerance: &Value, volts: bool, what: &str) {
    if volts {
        assert_close(actual, answer, num(&tolerance["voltsAbsolute"]), what);
    } else {
        let relative = tolerance.get("ampsRelative").map_or(0.0, num);
        let absolute = tolerance
            .get("ampsAbsolute")
            .or_else(|| tolerance.get("ampsAbsoluteFloor"))
            .map_or(0.0, num);
        assert_close(
            actual,
            answer,
            relative * actual.abs().max(answer.abs()) + absolute,
            what,
        );
    }
}

#[test]
fn the_catalog_carries_the_stage_7_devices_as_the_engine_defines_them() {
    let catalog: ComponentCatalog = component_catalog();
    let json = serde_json::to_value(&catalog).unwrap();
    let entry = |name: &str| {
        json["componentTypes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["typeId"] == name)
            .unwrap_or_else(|| panic!("{name} is in the catalog"))
            .clone()
    };
    let ids = |entry: &Value, list: &str, field: &str| {
        entry[list]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item[field].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };

    let diode = entry("diode");
    assert_eq!(ids(&diode, "pins", "id"), ["anode", "cathode"]);
    assert_eq!(
        ids(&diode, "parameters", "id"),
        ["saturationCurrentAmps", "emissionCoefficient"]
    );
    for name in ["bjt_npn", "bjt_pnp"] {
        let bjt = entry(name);
        assert_eq!(
            ids(&bjt, "pins", "id"),
            ["collector", "base", "emitter"],
            "{name}"
        );
        assert_eq!(
            ids(&bjt, "parameters", "id"),
            ["saturationCurrentAmps", "forwardBeta", "reverseBeta"],
            "{name}"
        );
        assert_eq!(bjt["faultKinds"], json!([]), "{name}");
    }

    // The catalog's ranges reach the interface as optional bounds, camelCase, and only for the
    // parameters that have them.
    let saturation = &diode["parameters"][0];
    assert_eq!(saturation["minimumValue"], json!(1e-18));
    assert_eq!(saturation["maximumValue"], json!(1e-9));
    assert_eq!(saturation["defaultValue"], json!(1e-14));
    let resistor = entry("resistor");
    assert!(resistor["parameters"][0].get("minimumValue").is_none());
}

#[test]
fn a_diode_and_a_transistor_reach_the_frames_and_traces_with_the_contract_keys() {
    // The diode: the usual two slots.
    let mut diode = Run::start_every_step(diode_circuit(5.0, 1000.0));
    let keys = diode
        .layout
        .slots
        .iter()
        .map(|slot| slot.key.clone())
        .collect::<Vec<_>>();
    assert!(keys.contains(&format!("{DEVICE}:voltage")));
    assert!(keys.contains(&format!("{DEVICE}:current")));
    let diode_slots = diode
        .layout
        .slots
        .iter()
        .filter(|slot| slot.component_id.as_str() == DEVICE)
        .count();
    assert_eq!(diode_slots, 2);

    diode.advance(5);
    let (frame, bytes) = diode.delivered.last().unwrap().clone();
    // The same bytes decode to the same snapshot, and every value is finite.
    assert_eq!(decode_frame(&bytes).unwrap(), frame);
    assert!(frame.values.iter().all(|value| value.is_finite()));
    let answer = analytic("diode-forward");
    let limits = tolerance("diode-forward");
    close_to_answer(
        at(&diode.layout, &frame, &format!("{DEVICE}:voltage")),
        num(&answer["diodeVoltageVolts"]),
        &limits,
        true,
        "diode voltage",
    );
    close_to_answer(
        at(&diode.layout, &frame, &format!("{DEVICE}:current")),
        num(&answer["diodeCurrentAmps"]),
        &limits,
        false,
        "diode current",
    );

    // The transistor: five slots in the contract's order, and no state record.
    let mut bjt = Run::start_every_step(bjt_circuit(false, 5.0, 430_000.0));
    let device_keys = bjt
        .layout
        .slots
        .iter()
        .filter(|slot| slot.component_id.as_str() == DEVICE)
        .map(|slot| slot.key.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        device_keys,
        [
            format!("{DEVICE}/base_emitter/voltage"),
            format!("{DEVICE}/collector_emitter/voltage"),
            format!("{DEVICE}/collector/current"),
            format!("{DEVICE}/base/current"),
            format!("{DEVICE}/emitter/current"),
        ]
    );
    bjt.advance(5);
    let (frame, bytes) = bjt.delivered.last().unwrap().clone();
    assert_eq!(decode_frame(&bytes).unwrap(), frame);
    assert!(
        frame.states.is_empty(),
        "a semiconductor has no state record"
    );

    let answer = analytic("npn-active");
    let limits = tolerance("npn-active");
    let read = |name: &str| at(&bjt.layout, &frame, &format!("{DEVICE}/{name}"));
    close_to_answer(
        read("base_emitter/voltage"),
        num(&answer["baseVoltageVolts"]),
        &limits,
        true,
        "V_BE",
    );
    close_to_answer(
        read("collector_emitter/voltage"),
        num(&answer["loadNodeVoltageVolts"]),
        &limits,
        true,
        "V_CE",
    );
    close_to_answer(
        read("collector/current"),
        num(&answer["collectorCurrentAmps"]),
        &limits,
        false,
        "i_C",
    );
    close_to_answer(
        read("base/current"),
        num(&answer["baseCurrentAmps"]),
        &limits,
        false,
        "i_B",
    );
    close_to_answer(
        read("emitter/current"),
        num(&answer["emitterCurrentAmps"]),
        &limits,
        false,
        "i_E",
    );
    // The terminal currents the Engine reports sum to zero: nothing in the bridge derives them.
    let sum = read("collector/current") + read("base/current") + read("emitter/current");
    assert!(
        sum.abs() <= 1e-12 * read("collector/current").abs(),
        "terminal sum {sum}"
    );

    // The traces carry the same keys, units, and values as the frame at the same step.
    let step = frame.metadata.simulation_step;
    let (trace, _bytes, _request) = bjt.read(&device_keys, step, step);
    let signals = trace.header["signals"].as_array().unwrap();
    let units = signals
        .iter()
        .map(|signal| {
            (
                signal["key"].as_str().unwrap().to_owned(),
                signal["unit"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        units
            .iter()
            .map(|(_, unit)| unit.as_str())
            .collect::<Vec<_>>(),
        ["volt", "volt", "amp", "amp", "amp"]
    );
    let (last_step, values) = trace.points.last().unwrap();
    assert_eq!(*last_step, step);
    for (index, key_name) in device_keys.iter().enumerate() {
        let (minimum, maximum) = values[index];
        assert_eq!(minimum, maximum);
        assert_eq!(minimum, at(&bjt.layout, &frame, key_name), "{key_name}");
    }
}

#[test]
fn the_pnp_polarity_survives_the_bridge() {
    let mut run = Run::start_every_step(bjt_circuit(true, 5.0, 4300.0));
    run.advance(3);
    let (frame, _) = run.delivered.last().unwrap().clone();
    let answer = analytic("pnp-saturation");
    let limits = tolerance("pnp-saturation");
    let read = |name: &str| at(&run.layout, &frame, &format!("{DEVICE}/{name}"));
    close_to_answer(
        read("base_emitter/voltage"),
        num(&answer["baseVoltageVolts"]),
        &limits,
        true,
        "V_BE",
    );
    close_to_answer(
        read("collector/current"),
        num(&answer["collectorCurrentAmps"]),
        &limits,
        false,
        "i_C",
    );
    // A PNP in normal bias reads a negative base-emitter voltage and a negative collector current.
    assert!(read("base_emitter/voltage") < 0.0 && read("collector/current") < 0.0);
}

#[test]
fn a_live_update_of_a_semiconductor_parameter_recomputes_the_step_and_replaces_its_sample() {
    let mut run = Run::start_every_step(diode_circuit(5.0, 1000.0));
    run.advance(4);
    let voltage = format!("{DEVICE}:voltage");
    let step_before = run.delivered.last().unwrap().0.metadata.simulation_step;
    let (before, _, _) = run.read(std::slice::from_ref(&voltage), step_before - 1, step_before);
    let old_newest = before.points.last().unwrap().1[0].0;

    let (request, result) = run.set_parameter(DEVICE, ParameterId::SaturationCurrentAmps, 1e-12);
    let SetParameterResult::Success { snapshot, .. } = result else {
        panic!("the saturation current is a live parameter");
    };
    assert_eq!(
        snapshot.metadata.document_revision,
        request.document_revision
    );
    assert_eq!(snapshot.metadata.simulation_step, step_before);
    let new_voltage = at(&run.layout, &snapshot, &voltage);
    assert!(
        new_voltage < old_newest - 0.1,
        "{new_voltage} against {old_newest}"
    );

    // The recomputed step replaced the newest trace sample; the earlier one is unchanged.
    let (after, _, _) = run.read(std::slice::from_ref(&voltage), step_before - 1, step_before);
    assert_eq!(after.points[0], before.points[0]);
    assert_eq!(after.points[1].1[0].0, new_voltage);

    // The stream goes on with the new revision.
    let before_len = run.delivered.len();
    run.advance(3);
    assert!(run.delivered.len() > before_len);
    for (frame, _) in &run.delivered[before_len..] {
        assert_eq!(frame.metadata.document_revision, request.document_revision);
        assert!((at(&run.layout, frame, &voltage) - new_voltage).abs() < 1e-6);
    }
}

#[test]
fn the_bridge_refuses_what_the_engine_refuses_and_changes_nothing() {
    let mut run = Run::start_every_step(diode_circuit(5.0, 1000.0));
    run.advance(3);
    let before = run.delivered.last().unwrap().0.clone();

    // A value outside the catalog range.
    let (_, result) = run.set_parameter(DEVICE, ParameterId::SaturationCurrentAmps, 1.0);
    let SetParameterResult::Failure { diagnostics } = result else {
        panic!("1 A is outside the range");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);

    // An edit that makes the circuit unsolvable: the resistor shrinks until an ideal source is
    // across a forward junction. The refusal names the diode.
    let (_, result) = run.set_parameter(LOAD, ParameterId::ResistanceOhms, 1e-21);
    let SetParameterResult::Failure { diagnostics } = result else {
        panic!("the model range is exceeded: {result:?}");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::ModelRangeExceeded);
    assert_eq!(
        diagnostics[0].references,
        [DiagnosticReference::Component {
            component_id: StableId::parse(DEVICE).unwrap(),
        }]
    );

    // Neither refusal took a revision or touched the stream: frames keep the old revision and
    // the values of the old circuit, and the next valid update still expects that revision.
    let length = run.delivered.len();
    run.advance(3);
    assert!(run.delivered.len() > length);
    for (frame, _) in &run.delivered[length..] {
        assert_eq!(
            frame.metadata.document_revision,
            before.metadata.document_revision
        );
        assert_close(
            at(&run.layout, frame, &format!("{DEVICE}:voltage")),
            at(&run.layout, &before, &format!("{DEVICE}:voltage")),
            1e-6,
            "diode voltage",
        );
    }
    let (_, result) = run.set_parameter(LOAD, ParameterId::ResistanceOhms, 2000.0);
    assert!(matches!(result, SetParameterResult::Success { .. }));
}

#[test]
fn a_failed_compile_carries_the_diagnostic_and_leaves_the_run_alone() {
    let mut run = Run::start_every_step(diode_circuit(5.0, 1000.0));
    run.advance(3);
    let revision = run
        .delivered
        .last()
        .unwrap()
        .0
        .metadata
        .document_revision
        .clone();

    // An ideal 5 V source directly across the diode: no resistor.
    let (ground, top) = (point(0, 10), point(0, 0));
    let (s, sg) = source(SOURCE, 5.0, top.clone(), ground.clone());
    let d = component(
        "diode",
        DEVICE,
        top.clone(),
        json!({ "saturationCurrentAmps": 1e-14, "emissionCoefficient": 1.0 }),
    );
    let dg = pins(DEVICE, &[("anode", top), ("cathode", ground)]);
    let result = run.core.compile(request(vec![s, d], vec![sg, dg]));
    let CompileResult::Failure { diagnostics } = &result else {
        panic!("the model range is exceeded");
    };
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, DiagnosticCode::ModelRangeExceeded);

    // The JSON the interface receives: the code, the localization key, and a typed reference.
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["status"], "failure");
    assert_eq!(json["diagnostics"][0]["code"], "ModelRangeExceeded");
    assert_eq!(
        json["diagnostics"][0]["localizationKey"],
        "diagnostic.modelRangeExceeded"
    );
    assert_eq!(
        json["diagnostics"][0]["references"],
        json!([{ "kind": "component", "componentId": DEVICE }])
    );

    // The earlier run is untouched: it still streams frames of its own revision.
    let length = run.delivered.len();
    run.advance(3);
    assert!(run.delivered.len() > length);
    assert_eq!(
        run.delivered.last().unwrap().0.metadata.document_revision,
        revision
    );
}

#[test]
fn stale_requests_and_replaced_runs_are_refused_against_real_nonlinear_circuits() {
    let mut run = Run::start_every_step(bjt_circuit(false, 5.0, 430_000.0));
    run.advance(3);
    let old_run = run.run_id.clone();
    let key_names = vec![format!("{DEVICE}/collector/current")];

    // A request that names an old revision, and one for another run.
    let mut stale = run.set_parameter(DEVICE, ParameterId::ForwardBeta, 80.0).0;
    stale.expected_document_revision = "rev-0".to_owned();
    stale.document_revision = "rev-9".to_owned();
    let SetParameterResult::Failure { diagnostics } = run.core.set_parameter(stale.clone()) else {
        panic!("an old revision is refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleDocumentRevision);
    stale.run_id = StableId::parse("00000000-0000-4000-8000-0000000000aa").unwrap();
    let SetParameterResult::Failure { diagnostics } = run.core.set_parameter(stale) else {
        panic!("another run is refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);

    // A new compile replaces the run: the old stream ends, and its parameter updates and trace
    // reads are stale, not answered from the new run.
    let CompileResult::Success { run_id, .. } = run.core.compile(bjt_circuit(false, 5.0, 4300.0))
    else {
        panic!("the saturated stage compiles");
    };
    assert_ne!(run_id, old_run);
    assert!(!run.core.is_streaming());
    let late = open_circuit_engine::SetParameterRequest {
        run_id: old_run.clone(),
        expected_document_revision: "rev-1".to_owned(),
        document_revision: "rev-9".to_owned(),
        component_id: StableId::parse(DEVICE).unwrap(),
        parameter_id: ParameterId::ForwardBeta,
        value: 80.0,
    };
    let SetParameterResult::Failure { diagnostics } = run.core.set_parameter(late) else {
        panic!("the replaced run is stale");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);
    let read = open_circuit_engine::TraceRequest {
        run_id: old_run,
        layout_version: run.layout.layout_version,
        signal_keys: key_names,
        query: open_circuit_engine::TraceQuery::History {
            start_step: 0,
            end_step: 3,
        },
        max_points: 16,
    };
    let refused = run
        .core
        .read_traces(read, Box::new(Capture::default()), Instant::now());
    assert_eq!(refused.unwrap_err()[0].code, DiagnosticCode::StaleRun);
}

/// A sink that keeps frames and notices, for the tests that drive `SimulationCore` directly.
#[derive(Clone, Default)]
struct Capture {
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    notices: Arc<Mutex<Vec<StreamNotice>>>,
}

impl FrameSink for Capture {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.frames.lock().unwrap().push(frame);
        Ok(())
    }

    fn notice(&mut self, notice: &StreamNotice) -> Result<(), SinkClosed> {
        self.notices.lock().unwrap().push(notice.clone());
        Ok(())
    }
}

impl open_circuit_app_lib::simulation::TraceSink for Capture {
    fn send(&mut self, _response: Vec<u8>) -> Result<(), SinkClosed> {
        Ok(())
    }
}

/// An ideal source of 1e154 V through a fuse: the compile succeeds, and the first step's
/// accumulated I^2 t is not a finite number, so the Engine stops the run. This is a real Engine
/// failure of an advancing run (a semiconductor circuit that only becomes unsolvable with time
/// has no short, deterministic example), and the notice is the same for every error code.
fn overflowing_circuit(volts: f64) -> CompileRequest {
    let (ground, top, mid) = (point(0, 10), point(0, 0), point(4, 0));
    let (s, sg) = source(SOURCE, volts, top.clone(), ground.clone());
    let fuse = component(
        "fuse",
        DEVICE,
        top.clone(),
        json!({ "intactResistanceOhms": 0.01, "tripI2tAmpereSquaredSeconds": 1e300 }),
    );
    let fg = pins(DEVICE, &[("pin1", top), ("pin2", mid.clone())]);
    let (r, rg) = resistor(LOAD, 1.0, mid, ground);
    request(vec![s, fuse, r], vec![sg, fg, rg])
}

#[test]
fn an_advancing_run_that_fails_stops_with_a_notice_and_no_partial_frame() {
    let mut core = SimulationCore::new(SimulationConfig {
        frame_interval: Duration::from_millis(1),
        ..SimulationConfig::default()
    });
    let CompileResult::Success { run_id, .. } = core.compile(overflowing_circuit(1e154)) else {
        panic!("the circuit compiles");
    };
    let capture = Capture::default();
    let now = Instant::now();
    core.start(run_id.clone(), Box::new(capture.clone()), now)
        .unwrap();

    core.tick(now + Duration::from_millis(5));

    // The run stopped: the stream ended, and not one frame of the failed step was delivered.
    assert!(!core.is_streaming());
    assert!(capture.frames.lock().unwrap().is_empty());
    let notices = capture.notices.lock().unwrap().clone();
    assert_eq!(notices.len(), 1);
    let StreamNotice::RunStopped {
        run_id: noticed_run,
        document_revision,
        simulation_step,
        diagnostics,
    } = &notices[0];
    assert_eq!(noticed_run, &run_id);
    assert_eq!(document_revision, "rev-1");
    // The last committed snapshot is the initial one.
    assert_eq!(*simulation_step, 0);
    assert_eq!(diagnostics[0].code, DiagnosticCode::NonFiniteState);

    // The JSON the webview receives is tagged and camelCase, with typed references.
    let json = serde_json::to_value(&notices[0]).unwrap();
    assert_eq!(json["kind"], "runStopped");
    assert_eq!(json["runId"], run_id.as_str());
    assert_eq!(json["documentRevision"], "rev-1");
    assert_eq!(json["simulationStep"], 0);
    assert_eq!(json["diagnostics"][0]["code"], "NonFiniteState");

    // Another tick says nothing more: a stopped run is not retried silently.
    core.tick(now + Duration::from_millis(20));
    assert_eq!(capture.notices.lock().unwrap().len(), 1);

    // A successful update lets it go on: the Engine accepts it, the stream is started again,
    // and frames of the new revision flow.
    let update = open_circuit_engine::SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "rev-1".to_owned(),
        document_revision: "rev-2".to_owned(),
        component_id: StableId::parse(SOURCE).unwrap(),
        parameter_id: ParameterId::VoltageVolts,
        value: 5.0,
    };
    assert!(matches!(
        core.set_parameter(update),
        SetParameterResult::Success { .. }
    ));
    let resumed = Capture::default();
    let later = Instant::now();
    core.start(run_id, Box::new(resumed.clone()), later)
        .unwrap();
    core.tick(later + Duration::from_millis(5));
    let frames = resumed.frames.lock().unwrap().clone();
    assert!(!frames.is_empty());
    assert!(resumed.notices.lock().unwrap().is_empty());
    assert_eq!(
        decode_frame(frames.last().unwrap())
            .unwrap()
            .metadata
            .document_revision,
        "rev-2"
    );
}

#[test]
fn a_notice_is_only_for_the_run_that_stopped() {
    // A run replaced by a compile ends its stream without a notice: the notice is for a stop on
    // an Engine error, never for a replacement.
    let mut core = SimulationCore::new(SimulationConfig::default());
    let CompileResult::Success { run_id, .. } = core.compile(diode_circuit(5.0, 1000.0)) else {
        panic!("compiles");
    };
    let capture = Capture::default();
    let now = Instant::now();
    core.start(run_id, Box::new(capture.clone()), now).unwrap();
    assert!(matches!(
        core.compile(bjt_circuit(false, 5.0, 430_000.0)),
        CompileResult::Success { .. }
    ));
    core.tick(now + Duration::from_millis(50));

    assert!(!core.is_streaming());
    assert!(capture.notices.lock().unwrap().is_empty());
    assert!(capture.frames.lock().unwrap().is_empty());
}

/// A circuit that costs the Engine real work at every step: ten transistor stages whose base
/// resistors share one node, fed from the supply through a push button.
fn heavy_circuit(button: &str, pull_down: bool) -> (CompileRequest, Vec<String>) {
    let (ground, vcc, link) = (point(0, 10), point(0, 0), point(5, 30));
    let (s, sg) = source(SOURCE, 10.0, vcc.clone(), ground.clone());
    let mut components = vec![s];
    let mut geometry = vec![sg];
    let mut stage_ids = Vec::new();
    for index in 0..10 {
        let x = 10 + 10 * index;
        let (collector, base) = (point(x, 0), point(x, 20));
        let tag = |kind: u32| {
            format!(
                "00000000-0000-4000-8000-{:012x}",
                0x100 + 16 * index as u32 + kind
            )
        };
        let (rc, rcg) = resistor(&tag(1), 1000.0, vcc.clone(), collector.clone());
        let (rb, rbg) = resistor(&tag(2), 430_000.0, link.clone(), base.clone());
        let q = component(
            "bjt_npn",
            &tag(3),
            collector.clone(),
            json!({ "saturationCurrentAmps": 1e-14, "forwardBeta": 100.0, "reverseBeta": 1.0 }),
        );
        let qg = pins(
            &tag(3),
            &[
                ("collector", collector),
                ("base", base),
                ("emitter", ground.clone()),
            ],
        );
        components.extend([rc, rb, q]);
        geometry.extend([rcg, rbg, qg]);
        stage_ids.push(tag(3));
    }
    let b = component(
        "push_button_no",
        button,
        vcc.clone(),
        json!({ "closedResistanceOhms": 0.01 }),
    );
    components.push(b);
    geometry.push(pins(button, &[("pin1", vcc), ("pin2", link.clone())]));
    if pull_down {
        let (r, rg) = resistor(
            "00000000-0000-4000-8000-0000000001f0",
            100_000.0,
            link,
            ground,
        );
        components.push(r);
        geometry.push(rg);
    }
    (request(components, geometry), stage_ids)
}

#[test]
fn commands_and_a_button_release_stay_serviceable_under_a_nonlinear_workload() {
    const BUTTON: &str = "00000000-0000-4000-8000-0000000000b1";
    // This test never acknowledges frames, so the in-flight limit that bounds delivery is lifted.
    let service = SimulationService::spawn(SimulationConfig {
        max_in_flight: 10_000,
        ..SimulationConfig::default()
    });
    let (circuit, stages) = heavy_circuit(BUTTON, true);
    let compiled = service.compile(circuit).unwrap();
    let CompileResult::Success { run_id, layout, .. } = compiled else {
        panic!("the heavy circuit compiles: {compiled:?}");
    };
    let capture = Capture::default();
    service
        .start(run_id.clone(), Box::new(capture.clone()))
        .unwrap()
        .unwrap();
    std::thread::sleep(Duration::from_millis(150));

    // Press, a parameter update, and release, each answered while the run streams.
    let press = |revision: &str, next: &str, position| SetOperatorStateRequest {
        run_id: run_id.clone(),
        expected_document_revision: revision.to_owned(),
        document_revision: next.to_owned(),
        component_id: StableId::parse(BUTTON).unwrap(),
        position,
    };
    let started = Instant::now();
    let result = service
        .set_operator_state(press("rev-1", "rev-2", OperatorPosition::Pressed))
        .unwrap();
    assert!(
        matches!(result, SetOperatorStateResult::Success { .. }),
        "press"
    );
    let result = service
        .set_parameter(open_circuit_engine::SetParameterRequest {
            run_id: run_id.clone(),
            expected_document_revision: "rev-2".to_owned(),
            document_revision: "rev-3".to_owned(),
            component_id: StableId::parse(&stages[0]).unwrap(),
            parameter_id: ParameterId::ForwardBeta,
            value: 120.0,
        })
        .unwrap();
    assert!(
        matches!(result, SetParameterResult::Success { .. }),
        "update"
    );
    std::thread::sleep(Duration::from_millis(100));
    let result = service
        .set_operator_state(press("rev-3", "rev-4", OperatorPosition::Released))
        .unwrap();
    assert!(
        matches!(result, SetOperatorStateResult::Success { .. }),
        "release"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the commands waited {:?}",
        started.elapsed()
    );
    std::thread::sleep(Duration::from_millis(100));
    service.stop(Some(run_id)).unwrap();

    // Physics timing is unchanged: every frame is a whole public step, in increasing order, and
    // the revisions advance in the order the commands were answered.
    let frames = capture
        .frames
        .lock()
        .unwrap()
        .iter()
        .map(|bytes| decode_frame(bytes).unwrap())
        .collect::<Vec<_>>();
    assert!(frames.len() > 5, "{} frames", frames.len());
    let mut last_step = 0;
    let mut last_sequence: Option<u64> = None;
    for frame in &frames {
        let step = frame.metadata.simulation_step;
        assert!(step >= last_step);
        assert!((frame.metadata.simulation_time_seconds - step as f64 * 0.001).abs() < 1e-12);
        assert!(last_sequence.is_none_or(|last| frame.metadata.frame_sequence > last));
        assert!(frame.values.iter().all(|value| value.is_finite()));
        last_step = step;
        last_sequence = Some(frame.metadata.frame_sequence);
    }
    let revisions = frames
        .iter()
        .map(|frame| frame.metadata.document_revision.as_str())
        .collect::<Vec<_>>();
    assert!(revisions.contains(&"rev-4"), "{revisions:?}");
    // The last frames show the released button: the first stage's transistor has no base drive.
    let collector = layout
        .slots
        .iter()
        .find(|slot| slot.key == format!("{}/collector/current", stages[0]))
        .unwrap()
        .offset as usize;
    assert!(frames.last().unwrap().values[collector].abs() < 1e-6);
    service.shutdown();
}

fn project_with_stage_7_devices() -> CompileRequest {
    let (ground, vcc, vbb) = (point(0, 10), point(0, 0), point(0, 20));
    let (collector, base) = (point(8, 0), point(8, 20));
    let anode = point(16, 0);
    let (s, sg) = source(SOURCE, 10.0, vcc.clone(), ground.clone());
    let (r, rg) = resistor(LOAD, 1000.0, vcc.clone(), collector.clone());
    let (rb, rbg) = resistor(BASE_RESISTOR, 430_000.0, vbb, base.clone());
    let npn = component(
        "bjt_npn",
        DEVICE,
        collector.clone(),
        json!({ "saturationCurrentAmps": 1e-15, "forwardBeta": 30.0, "reverseBeta": 5.0 }),
    );
    let npn_pins = pins(
        DEVICE,
        &[
            ("collector", collector),
            ("base", base),
            ("emitter", ground.clone()),
        ],
    );
    let pnp = component(
        "bjt_pnp",
        "00000000-0000-4000-8000-0000000000d2",
        anode.clone(),
        json!({ "saturationCurrentAmps": 1e-14, "forwardBeta": 100.0, "reverseBeta": 1.0 }),
    );
    let pnp_pins = pins(
        "00000000-0000-4000-8000-0000000000d2",
        &[
            ("collector", point(20, 0)),
            ("base", point(20, 20)),
            ("emitter", anode.clone()),
        ],
    );
    let diode = component(
        "diode",
        "00000000-0000-4000-8000-0000000000d3",
        anode.clone(),
        json!({ "saturationCurrentAmps": 1e-12, "emissionCoefficient": 1.8 }),
    );
    let diode_pins = pins(
        "00000000-0000-4000-8000-0000000000d3",
        &[("anode", anode), ("cathode", ground)],
    );
    request(
        vec![s, r, rb, npn, pnp, diode],
        vec![sg, rg, rbg, npn_pins, pnp_pins, diode_pins],
    )
}

#[test]
fn a_stage_7_project_keeps_parameters_and_geometry_only() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("nonlinear.ocircuit");
    let schematic = project_with_stage_7_devices().schematic;

    write_project_file(&path, &schematic).expect("the project is saved");
    let text = std::fs::read_to_string(&path).unwrap();
    for computed in [
        "region",
        "conducting",
        "iteration",
        "converged",
        "solver",
        "operating",
    ] {
        assert!(!text.contains(computed), "the file contains {computed}");
    }
    let saved: Value = serde_json::from_str(&text).unwrap();
    let types = saved["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["typeId"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(types.contains(&"bjt_npn") && types.contains(&"bjt_pnp") && types.contains(&"diode"));
    assert_eq!(read_project_file(&path).unwrap(), schematic);

    // What is not a parameter, a type, or a range of this release is refused, not read as
    // something else.
    let refused = |edit: &dyn Fn(&mut Value)| {
        let mut document = saved.clone();
        edit(&mut document);
        decode_project_json(&serde_json::to_vec(&document).unwrap()).is_err()
    };
    assert!(refused(
        &|d| d["components"][3]["typeId"] = json!("bjt_darlington")
    ));
    assert!(refused(
        &|d| d["components"][3]["parameters"]["earlyVoltageVolts"] = json!(75.0)
    ));
    assert!(refused(
        &|d| d["components"][5]["parameters"]["saturationCurrentAmps"] = json!(1.0)
    ));
    assert!(refused(
        &|d| d["components"][5]["parameters"]["emissionCoefficient"] = json!(5.0)
    ));
    assert!(refused(
        &|d| d["components"][3]["parameters"]["forwardBeta"] = json!(0.0)
    ));
    assert!(refused(&|d| {
        d["components"][3]["parameters"]
            .as_object_mut()
            .unwrap()
            .remove("reverseBeta");
    }));
}

#[test]
fn the_files_of_the_earlier_stages_reopen_unchanged() {
    let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden");
    let mut checked = 0;
    for entry in std::fs::read_dir(&golden).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let document: Value = if name.ends_with(".schematic.json") {
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
        } else if name.contains("-drawn-") && name.ends_with(".request.json") {
            let request: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            request["schematic"].clone()
        } else {
            continue;
        };
        // Reading and writing it again changes nothing, and nothing new appears in it.
        let schematic = decode_project_json(&serde_json::to_vec(&document).unwrap())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            same_json(&serde_json::to_value(&schematic).unwrap(), &document),
            "{name}"
        );
        checked += 1;
    }
    assert!(checked >= 15, "only {checked} files were checked");
}

/// JSON equality that reads 5 and 5.0 as the same number, as a reader of the file does.
fn same_json(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_json(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, x)| b.get(key).is_some_and(|y| same_json(x, y)))
        }
        _ => left == right,
    }
}
