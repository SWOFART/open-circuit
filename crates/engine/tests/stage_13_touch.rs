//! Stage 13 touch voltage and the hazard flag (issue #201, docs/contracts/
//! stage-13-protection-and-safety.md section 7): an exposed part measures the voltage of its net
//! against the earth node over one period and reports it hazardous above its limit.

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, Engine, ResultLayout,
    ResultSnapshot, StableId,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const ELECTRODE: &str = "00000000-0000-4000-8000-000000000002";
const FRAME: &str = "00000000-0000-4000-8000-000000000003";
const LEAK: &str = "00000000-0000-4000-8000-0000000000a1";

const DOCUMENTATION: &str = r#"{"designators":[],"wireNumbers":[],"terminals":[],"notes":[],
    "titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},
    "sheets":[]}"#;

fn part(id: &str, type_id: &str, parameters: Value, pins: &[(&str, (i32, i32))]) -> (Value, Value) {
    (
        json!({"typeId": type_id, "id": id, "position": {"x": 0, "y": 0},
            "rotationDeg": 0, "parameters": parameters}),
        json!({"componentId": id, "pins": pins.iter().map(|(pin, (x, y))| json!({
            "pinId": pin, "position": {"x": x, "y": y}})).collect::<Vec<_>>()}),
    )
}

fn dc_source(volts: f64, rs: f64) -> (Value, Value) {
    part(
        SOURCE,
        "dc_voltage_source",
        json!({"voltageVolts": volts, "internalResistanceOhms": rs}),
        &[("pin1", (0, 0)), ("pin2", (2, 2))],
    )
}

fn ac_source(rms: f64, rs: f64) -> (Value, Value) {
    part(
        SOURCE,
        "ac_voltage_source",
        json!({"voltageRmsVolts": rms, "frequencyHz": 50.0, "phaseOffsetDegrees": 0.0,
            "internalResistanceOhms": rs}),
        &[("pin1", (0, 0)), ("pin2", (2, 2))],
    )
}

fn electrode(re: f64, at: (i32, i32)) -> (Value, Value) {
    part(
        ELECTRODE,
        "earth_electrode",
        json!({"earthResistanceOhms": re}),
        &[("pe", at)],
    )
}

fn frame(limit: f64) -> (Value, Value) {
    part(
        FRAME,
        "exposed_part",
        json!({"frequencyHz": 50.0, "touchVoltageLimitVolts": limit}),
        &[("ep", (10, 0))],
    )
}

fn request(parts: Vec<(Value, Value)>, marker: bool, rf: f64) -> CompileRequest {
    let markers = if marker {
        json!([{"id": "00000000-0000-4000-8000-0000000000f0", "position": {"x": 2, "y": 2}}])
    } else {
        json!([])
    };
    serde_json::from_value(json!({
        "schematic": {
            "schemaVersion": 3,
            "components": parts.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
            "wires": [], "junctions": [], "rails": [], "referenceMarkers": markers,
            "documentation": serde_json::from_str::<Value>(DOCUMENTATION).unwrap(),
        },
        "geometry": {"components": parts.iter().map(|(_, g)| g.clone()).collect::<Vec<_>>()},
        "documentRevision": "rev-1",
        "faults": [{"id": LEAK, "kind": "leak", "componentId": SOURCE,
            "pinId": "pin1", "frameId": FRAME, "value": rf}],
    }))
    .expect("a compile request")
}

struct Run {
    engine: Engine,
    run_id: StableId,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
    warnings: Vec<DiagnosticCode>,
}

fn start(request: CompileRequest) -> Run {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        diagnostics,
        ..
    } = engine.compile(request)
    else {
        panic!("the circuit should compile");
    };
    Run {
        engine,
        run_id,
        layout,
        snapshot: initial_snapshot,
        warnings: diagnostics.into_iter().map(|d| d.code).collect(),
    }
}

impl Run {
    fn value(&self, suffix: &str) -> f64 {
        let key = format!("{FRAME}/{suffix}");
        let slot = self
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == key)
            .unwrap_or_else(|| panic!("no slot {key}"));
        self.snapshot.values[slot.offset as usize]
    }

    fn advance(&mut self, steps: u32) {
        self.snapshot = self
            .engine
            .advance(AdvanceRequest {
                run_id: self.run_id.clone(),
                step_count: steps,
            })
            .expect("the run advances");
    }

    fn valid(&self) -> bool {
        self.value("valid") == 1.0
    }

    fn hazard(&self) -> bool {
        self.value("hazard") == 1.0
    }
}

fn close(actual: f64, expected: f64, relative: f64, absolute: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= absolute + relative * expected.abs(),
        "{what}: {actual} is not within {absolute} + {relative}*|x| of {expected}"
    );
}

/// 100 V with 1 ohm internal resistance leaking through `rf` to a frame bonded (the electrode
/// shares the frame's point) to an electrode `re`: the frame sits at `100 * re / (1 + rf + re)`.
fn bonded(re: f64, rf: f64, limit: f64) -> CompileRequest {
    request(
        vec![dc_source(100.0, 1.0), electrode(re, (10, 0)), frame(limit)],
        true,
        rf,
    )
}

#[test]
fn the_touch_voltage_is_read_after_one_full_window_and_is_hazardous_above_the_limit() {
    // Re = 9, Rf = 10: 45 V on the frame.
    for (limit, hazardous) in [(44.5, true), (45.5, false), (50.0, false), (10.0, true)] {
        let mut run = start(bonded(9.0, 10.0, limit));
        run.advance(25);

        assert!(run.valid(), "limit {limit}");
        close(run.value("touch_voltage_rms"), 45.0, 1e-6, 1e-6, "rms");
        close(run.value("touch_voltage_peak"), 45.0, 1e-6, 1e-6, "peak");
        assert_eq!(run.hazard(), hazardous, "45 V against a limit of {limit} V");
    }
}

#[test]
fn nothing_is_valid_or_hazardous_before_the_first_full_window() {
    let mut run = start(bonded(20.0, 10.0, 50.0));
    // The frame is at about 64.5 V, well above the limit, but the window is one period (20 ms).
    for _ in 0..19 {
        assert!(!run.valid());
        assert!(!run.hazard());
        assert_eq!(run.value("touch_voltage_rms"), 0.0);
        run.advance(1);
    }
    run.advance(2);
    assert!(run.valid());
    assert!(run.hazard());
    close(
        run.value("touch_voltage_rms"),
        100.0 * 20.0 / 31.0,
        1e-6,
        1e-6,
        "rms",
    );
}

#[test]
fn an_unbonded_frame_reads_the_leaking_voltage() {
    // The electrode is somewhere else: the bonding is missing and the frame floats to about E.
    let mut run = start(request(
        vec![dc_source(100.0, 1.0), electrode(9.0, (20, 0)), frame(50.0)],
        true,
        10.0,
    ));
    run.advance(25);

    assert!(run.valid());
    close(run.value("touch_voltage_rms"), 100.0, 1e-6, 1e-6, "rms");
    assert!(run.hazard());
}

#[test]
fn without_an_earth_node_there_is_no_reading() {
    // No ground marker and no electrode: the frame's island has only an automatic reference.
    let mut run = start(request(
        vec![dc_source(100.0, 1.0), frame(50.0)],
        false,
        10.0,
    ));
    run.advance(25);

    assert!(run.warnings.contains(&DiagnosticCode::NoEarthPath));
    assert!(!run.valid());
    assert!(!run.hazard());
    assert_eq!(run.value("touch_voltage_rms"), 0.0);
}

#[test]
fn a_fifty_hertz_fault_reads_the_rms_of_the_frame_voltage() {
    // 230 V RMS, Rs = 1, Rf = 10, Re = 9: the frame carries Re * I = 103.5 V RMS.
    let mut run = start(request(
        vec![ac_source(230.0, 1.0), electrode(9.0, (10, 0)), frame(50.0)],
        true,
        10.0,
    ));
    run.advance(60);

    assert!(run.valid());
    // The measured RMS of a sinusoid is held to the Stage 9 tolerance, not to the DC one.
    close(
        run.value("touch_voltage_rms"),
        230.0 * 9.0 / 20.0,
        2e-3,
        0.0,
        "rms",
    );
    close(
        run.value("touch_voltage_peak"),
        230.0 * 2.0_f64.sqrt() * 9.0 / 20.0,
        2e-3,
        0.0,
        "peak",
    );
    assert!(run.hazard());
}

#[test]
fn the_results_follow_the_contract_slot_order_with_no_instantaneous_slot() {
    let run = start(bonded(9.0, 10.0, 50.0));
    let keys: Vec<&str> = run
        .layout
        .slots
        .iter()
        .filter(|slot| slot.key.starts_with(FRAME))
        .map(|slot| slot.key.as_str())
        .collect();

    assert_eq!(
        keys,
        [
            format!("{FRAME}/touch_voltage_rms"),
            format!("{FRAME}/touch_voltage_peak"),
            format!("{FRAME}/hazard"),
            format!("{FRAME}/valid"),
        ]
    );
}

#[test]
fn the_frequency_and_the_limit_are_live_and_a_new_frequency_starts_a_new_window() {
    use open_circuit_engine::{ParameterId, SetParameterRequest, SetParameterResult};

    let mut run = start(bonded(9.0, 10.0, 50.0));
    run.advance(25);
    assert!(run.valid());
    assert!(!run.hazard());

    let set = |run: &mut Run, parameter, value| {
        let revision = run.snapshot.metadata.document_revision.clone();
        let result = run.engine.set_parameter(SetParameterRequest {
            run_id: run.run_id.clone(),
            expected_document_revision: revision.clone(),
            document_revision: format!("{revision}+"),
            component_id: StableId::parse(FRAME).unwrap(),
            parameter_id: parameter,
            value,
        });
        let SetParameterResult::Success { snapshot, .. } = result else {
            panic!("{parameter:?} should be live");
        };
        run.snapshot = snapshot;
    };

    // Lowering the limit below the reading makes the part hazardous at once.
    set(&mut run, ParameterId::TouchVoltageLimitVolts, 40.0);
    assert!(run.valid());
    assert!(run.hazard());

    // A new frequency is a new window: the reading is not valid until a full period has passed.
    set(&mut run, ParameterId::FrequencyHz, 25.0);
    assert!(!run.valid());
    assert!(!run.hazard());
    run.advance(45);
    assert!(run.valid());
    assert!(run.hazard());
}
