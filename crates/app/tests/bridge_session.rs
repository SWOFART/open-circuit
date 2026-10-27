//! A real Engine session, carried the way the bridge carries it.
//!
//! The test drives the real Engine through `SimulationCore` and the real frame encoder with
//! a fake clock, checks every number against the golden divider values (voltage within
//! 1 µV, current within 1 nA), and writes the session to
//! `<target>/tmp/bridge-session.json`. `src/store/endToEnd.test.tsx` replays that file
//! through the TypeScript adapter, controller, and UI, so the whole path
//! Engine -> DTO JSON -> binary frame -> decoder -> adapter -> controller -> screen is
//! checked on every `pnpm test`, with the numbers produced by the Engine on the machine
//! running the tests.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::simulation::{FrameSink, SimulationConfig, SimulationCore, SinkClosed};
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, ParameterId, ResultLayout, ResultQuantity,
    ResultSnapshot, SetParameterRequest, SetParameterResult, StableId, component_catalog,
};
use serde_json::{Value, json};

const SCHEMATIC: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const EXPECTED: &str = include_str!("../../../tests/golden/stage-1-divider.expected.json");

const SOURCE_ID: &str = "00000000-0000-4000-8000-000000000001";
const RESISTOR_1_ID: &str = "00000000-0000-4000-8000-000000000002";
const RESISTOR_2_ID: &str = "00000000-0000-4000-8000-000000000003";

const VOLTAGE_TOLERANCE_VOLTS: f64 = 1e-6;
const CURRENT_TOLERANCE_AMPS: f64 = 1e-9;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Vec<u8>>>>);

impl Capture {
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut *self.0.lock().expect("lock"))
    }
}

struct CaptureSink(Capture);

impl FrameSink for CaptureSink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.0.lock().expect("lock").push(frame);
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn id(text: &str) -> StableId {
    StableId::parse(text).expect("a valid stable ID")
}

fn value_of(
    layout: &ResultLayout,
    snapshot: &ResultSnapshot,
    component: &str,
    quantity: ResultQuantity,
) -> f64 {
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.component_id.as_str() == component && slot.quantity == quantity)
        .unwrap_or_else(|| panic!("no {quantity:?} slot for {component}"));

    snapshot.values[slot.offset as usize]
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} is not within {tolerance} of {expected}"
    );
}

/// Checks one snapshot of the divider against a golden case. Signs follow the contract:
/// both resistors carry current into `pin1`, and the source, which discharges, carries it
/// out of `pin1`.
fn assert_divider(layout: &ResultLayout, snapshot: &ResultSnapshot, golden: &Value, what: &str) {
    let number = |key: &str| golden[key].as_f64().expect("a number in the fixture");
    let current = number("sourceCurrentAmps");

    assert_close(
        value_of(layout, snapshot, RESISTOR_1_ID, ResultQuantity::Voltage),
        number("resistor1VoltageVolts"),
        VOLTAGE_TOLERANCE_VOLTS,
        &format!("{what}: R1 voltage"),
    );
    assert_close(
        value_of(layout, snapshot, RESISTOR_2_ID, ResultQuantity::Voltage),
        number("resistor2VoltageVolts"),
        VOLTAGE_TOLERANCE_VOLTS,
        &format!("{what}: R2 voltage"),
    );
    assert_close(
        value_of(layout, snapshot, RESISTOR_1_ID, ResultQuantity::Current),
        current,
        CURRENT_TOLERANCE_AMPS,
        &format!("{what}: R1 current"),
    );
    assert_close(
        value_of(layout, snapshot, RESISTOR_2_ID, ResultQuantity::Current),
        current,
        CURRENT_TOLERANCE_AMPS,
        &format!("{what}: R2 current"),
    );
    assert_close(
        value_of(layout, snapshot, SOURCE_ID, ResultQuantity::Current),
        -current,
        CURRENT_TOLERANCE_AMPS,
        &format!("{what}: source current"),
    );
}

struct Session {
    core: SimulationCore,
    capture: Capture,
    now: Instant,
}

impl Session {
    /// Moves the fake clock in 1 ms ticks, acknowledging every frame, until a frame of
    /// `revision` is delivered.
    fn frame_of(&mut self, revision: &str) -> (String, ResultSnapshot) {
        for _ in 0..200 {
            self.now += Duration::from_millis(1);
            self.core.tick(self.now);

            for frame in self.capture.take() {
                let snapshot = decode_frame(&frame).expect("every delivered frame decodes");
                self.core.acknowledge(snapshot.metadata.frame_sequence);

                if snapshot.metadata.document_revision == revision {
                    return (hex(&frame), snapshot);
                }
            }
        }

        panic!("no frame of revision {revision} within 200 ms of simulated time");
    }

    fn set_resistance(
        &mut self,
        run_id: &StableId,
        expected: &str,
        revision: &str,
        value: f64,
    ) -> (SetParameterRequest, SetParameterResult) {
        let request = SetParameterRequest {
            run_id: run_id.clone(),
            expected_document_revision: expected.to_owned(),
            document_revision: revision.to_owned(),
            component_id: id(RESISTOR_1_ID),
            parameter_id: ParameterId::ResistanceOhms,
            value,
        };
        let result = self.core.set_parameter(request.clone());

        (request, result)
    }
}

#[test]
fn a_real_engine_session_meets_the_golden_values_and_is_recorded_for_the_ui() {
    let expected: Value = serde_json::from_str(EXPECTED).expect("expected fixture");
    let compile_request = CompileRequest {
        schematic: serde_json::from_str(SCHEMATIC).expect("schematic fixture"),
        geometry: serde_json::from_str(GEOMETRY).expect("geometry fixture"),
        document_revision: "rev-1".to_owned(),
        faults: None,
    };

    let capture = Capture::default();
    let mut session = Session {
        core: SimulationCore::new(SimulationConfig::default()),
        capture: capture.clone(),
        now: Instant::now(),
    };

    // 1. Compile and start streaming.
    let compile_result = session.core.compile(compile_request.clone());
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = &compile_result
    else {
        panic!("the canonical divider must compile");
    };
    assert_divider(
        layout,
        initial_snapshot,
        &expected["equalResistors"],
        "compile",
    );

    session
        .core
        .start(run_id.clone(), Box::new(CaptureSink(capture)), session.now)
        .expect("start streaming");
    let (compile_frame_hex, compile_frame) = session.frame_of("rev-1");
    assert_divider(
        layout,
        &compile_frame,
        &expected["equalResistors"],
        "first frame",
    );

    // 2. R1 to 3000 ohms.
    let (to_3000_request, to_3000_result) =
        session.set_resistance(run_id, "rev-1", "rev-2", 3000.0);
    let SetParameterResult::Success { snapshot, .. } = &to_3000_result else {
        panic!("R1 = 3000 must be accepted");
    };
    assert_divider(
        layout,
        snapshot,
        &expected["resistor1At3000Ohms"],
        "R1 = 3000 reply",
    );
    let (to_3000_frame_hex, to_3000_frame) = session.frame_of("rev-2");
    assert_divider(
        layout,
        &to_3000_frame,
        &expected["resistor1At3000Ohms"],
        "R1 = 3000 frame",
    );

    // 3. R1 restored to 1000 ohms.
    let (restore_request, restore_result) =
        session.set_resistance(run_id, "rev-2", "rev-3", 1000.0);
    let SetParameterResult::Success { snapshot, .. } = &restore_result else {
        panic!("R1 = 1000 must be accepted");
    };
    assert_divider(
        layout,
        snapshot,
        &expected["equalResistors"],
        "R1 restored reply",
    );
    let (restore_frame_hex, restore_frame) = session.frame_of("rev-3");
    assert_divider(
        layout,
        &restore_frame,
        &expected["equalResistors"],
        "R1 restored frame",
    );

    // 4. A value the UI would refuse, sent straight to the Engine: it must be rejected
    //    with a diagnostic, never answered with a result.
    let (zero_request, zero_result) = session.set_resistance(run_id, "rev-3", "rev-4", 0.0);
    let SetParameterResult::Failure { diagnostics } = &zero_result else {
        panic!("R1 = 0 must be rejected");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);

    // The rejected update left the stream and its values unchanged.
    let (after_rejection_hex, after_rejection) = session.frame_of("rev-3");
    assert_divider(
        layout,
        &after_rejection,
        &expected["equalResistors"],
        "after rejection",
    );

    let recorded = json!({
        "format": 1,
        "description": "A real Engine session as the bridge carries it, written by crates/app/tests/bridge_session.rs on every cargo test. src/store/endToEnd.test.tsx replays it through the TypeScript adapter, controller, and UI.",
        "catalog": component_catalog(),
        "steps": [
            {
                "name": "compile",
                "command": "compile_circuit",
                "request": compile_request,
                "reply": compile_result,
                "frameHex": compile_frame_hex,
            },
            {
                "name": "r1-to-3000",
                "command": "set_parameter",
                "request": to_3000_request,
                "reply": to_3000_result,
                "frameHex": to_3000_frame_hex,
            },
            {
                "name": "r1-restored",
                "command": "set_parameter",
                "request": restore_request,
                "reply": restore_result,
                "frameHex": restore_frame_hex,
            },
            {
                "name": "r1-zero-rejected",
                "command": "set_parameter",
                "request": zero_request,
                "reply": zero_result,
                "frameHex": after_rejection_hex,
            },
        ],
    });

    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bridge-session.json");
    fs::create_dir_all(path.parent().expect("a parent directory")).expect("create the directory");
    fs::write(
        &path,
        serde_json::to_string_pretty(&recorded).expect("the session serializes"),
    )
    .expect("write the session");
}
