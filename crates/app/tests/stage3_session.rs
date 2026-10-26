//! The Stage 3 acceptance session (issue #50): the RC charge circuit drawn in the editor,
//! compiled and run by the real Engine, and read back through the bridge's trace envelope.
//!
//! `tests/golden/stage-3-drawn-rc.request.json` is the compile request that
//! `src/store/stage3Drawing.test.ts` produces by drawing the circuit from a blank document
//! with the editor's own commands. This test compiles that file with the real Engine
//! through `SimulationCore`, runs it against a fake clock, reads the stored samples with
//! the real trace encoder, and checks them against the analytical curve of
//! docs/physics/models.md. It writes the session to `<target>/tmp/stage3-session.json`,
//! which `src/store/stage3EndToEnd.test.tsx` replays through the TypeScript adapter,
//! decoder, controller, and oscilloscope panel.

mod common;

use std::fs;
use std::path::PathBuf;

use common::{Run, assert_close, hex, slot_key, value_of};
use open_circuit_app_lib::simulation::{SimulationConfig, SimulationCore};
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, DiagnosticReference, ResultQuantity,
    component_catalog,
};
use serde_json::{Value, json};

const RC_REQUEST: &str = include_str!("../../../tests/golden/stage-3-drawn-rc.request.json");
const CONFLICT_REQUEST: &str =
    include_str!("../../../tests/golden/stage-3-drawn-rc-conflict.request.json");
const EXPECTED: &str = include_str!("../../../tests/golden/stage-3-rc-charge.expected.json");

/// The drawn circuit: V1 is the first component, R1 the second, C1 the third, and the
/// conflicting request adds a second capacitor.
const CAPACITOR_ID: &str = "00000000-0000-4000-8000-000000000003";
const PARALLEL_CAPACITOR_ID: &str = "00000000-0000-4000-8000-000000000008";

const SOURCE_VOLTS: f64 = 10.0;
const RESISTANCE_OHMS: f64 = 1000.0;
const CAPACITANCE_FARADS: f64 = 1e-5;
const TAU_SECONDS: f64 = RESISTANCE_OHMS * CAPACITANCE_FARADS;
/// 0.1% of the 10 V full scale, the accepted error of the RC fixtures.
const VOLTAGE_TOLERANCE_VOLTS: f64 = 0.01;
/// 0.1% of the 10 mA full scale.
const CURRENT_TOLERANCE_AMPS: f64 = 1e-5;

/// `v_C(t)` of the charging capacitor, the analytical solution.
fn charging_voltage(seconds: f64) -> f64 {
    SOURCE_VOLTS * (1.0 - (-seconds / TAU_SECONDS).exp())
}

/// `i_C(t)`, positive entering `pin1` as the source charges it.
fn charging_current(seconds: f64) -> f64 {
    (SOURCE_VOLTS / RESISTANCE_OHMS) * (-seconds / TAU_SECONDS).exp()
}

#[test]
fn the_drawn_rc_circuit_runs_on_the_real_engine_and_its_traces_follow_the_analytical_curve() {
    let expected: Value = serde_json::from_str(EXPECTED).expect("expected fixture");
    let request: CompileRequest = serde_json::from_str(RC_REQUEST).expect("the drawn request");

    // 1. Compile what the editor drew, and start streaming.
    let mut run = Run::start(request.clone());
    let CompileResult::Success { diagnostics, .. } = &run.compile_result else {
        unreachable!("Run::start only returns a successful compile");
    };
    assert!(diagnostics.is_empty(), "no diagnostics: {diagnostics:?}");

    // Step 0 starts from the explicit 0 V: C1 empty, and the source charging it at 10 mA.
    assert_close(
        value_of(
            &run.layout,
            &run.initial_snapshot,
            CAPACITOR_ID,
            ResultQuantity::Voltage,
        ),
        0.0,
        VOLTAGE_TOLERANCE_VOLTS,
        "C1 voltage at step 0",
    );
    assert_close(
        value_of(
            &run.layout,
            &run.initial_snapshot,
            CAPACITOR_ID,
            ResultQuantity::Current,
        ),
        charging_current(0.0),
        CURRENT_TOLERANCE_AMPS,
        "C1 current at step 0",
    );

    // 2. Run 70 ms against the fake clock, acknowledging every frame.
    let newest = run.advance(70);
    assert!(newest >= 50, "the run advanced to step {newest}");

    // 3. Read the stored samples of C1's voltage and current through the trace envelope.
    let keys = vec![
        slot_key(&run.layout, CAPACITOR_ID, ResultQuantity::Voltage),
        slot_key(&run.layout, CAPACITOR_ID, ResultQuantity::Current),
    ];
    let (trace, response, trace_request) = run.read(&keys, 0, 50);

    assert_eq!(
        trace.header["metadata"]["runId"],
        json!(run.run_id.as_str())
    );
    assert_eq!(
        trace.header["metadata"]["sampleIntervalSeconds"],
        json!(0.001)
    );
    assert_eq!(trace.header["signals"][0]["key"], json!(keys[0]));
    assert_eq!(trace.header["signals"][0]["unit"], json!("volt"));
    assert_eq!(trace.header["signals"][1]["unit"], json!("amp"));
    assert_eq!(
        trace.points.len(),
        51,
        "one exact sample for each of steps 0 to 50"
    );

    // Every public step to 50 ms against the analytical curve, within 0.1% of full scale.
    for (step, values) in &trace.points {
        let seconds = *step as f64 * 0.001;
        let (voltage, voltage_max) = values[0];
        let (current, _) = values[1];

        assert_eq!(voltage, voltage_max, "an undecimated point is one sample");
        assert_close(
            voltage,
            charging_voltage(seconds),
            VOLTAGE_TOLERANCE_VOLTS,
            &format!("C1 voltage at {step} ms"),
        );
        assert_close(
            current,
            charging_current(seconds),
            CURRENT_TOLERANCE_AMPS,
            &format!("C1 current at {step} ms"),
        );
    }

    // The values the acceptance names: 10, 20, and 30 ms, from the fixture file.
    for reference in expected["analytical"].as_array().unwrap() {
        let seconds = reference["timeSeconds"].as_f64().unwrap();
        let step = (seconds / 0.001).round() as u64;
        let (_, values) = trace
            .points
            .iter()
            .find(|(candidate, _)| *candidate == step)
            .expect("a sample at every public step");

        assert_close(
            values[0].0,
            reference["voltageVolts"].as_f64().unwrap(),
            expected["analyticalToleranceVolts"].as_f64().unwrap(),
            &format!("C1 voltage at {seconds} s"),
        );
    }

    // 4. An incompatible combined initial state is a structured diagnostic, not a result.
    let conflict_request: CompileRequest =
        serde_json::from_str(CONFLICT_REQUEST).expect("the conflicting request");
    let conflict_result =
        SimulationCore::new(SimulationConfig::default()).compile(conflict_request.clone());
    let CompileResult::Failure { diagnostics } = &conflict_result else {
        panic!("parallel capacitors with 0 V and 5 V must not compile");
    };
    let state_conflict = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == DiagnosticCode::InconsistentState)
        .expect("the conflict is reported as InconsistentState");
    let referenced: Vec<&str> = state_conflict
        .references
        .iter()
        .filter_map(|reference| match reference {
            DiagnosticReference::Component { component_id } => Some(component_id.as_str()),
            _ => None,
        })
        .collect();
    assert!(referenced.contains(&CAPACITOR_ID));
    assert!(referenced.contains(&PARALLEL_CAPACITOR_ID));

    let recorded = json!({
        "format": 1,
        "description": "The Stage 3 RC charge acceptance session, written by crates/app/tests/stage3_session.rs on every cargo test. src/store/stage3EndToEnd.test.tsx replays it through the TypeScript adapter, decoder, controller, and oscilloscope panel.",
        "catalog": component_catalog(),
        "rc": {
            "request": request,
            "compileReply": run.compile_result,
            "traceRequest": trace_request,
            "traceResponseHex": hex(&response),
        },
        "conflict": {
            "request": conflict_request,
            "compileReply": conflict_result,
        },
    });
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("stage3-session.json");
    fs::create_dir_all(path.parent().expect("a parent directory")).expect("create the directory");
    fs::write(
        &path,
        serde_json::to_string_pretty(&recorded).expect("the session serializes"),
    )
    .expect("write the session");
}
