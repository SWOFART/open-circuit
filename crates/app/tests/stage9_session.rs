//! Stage 9 delivery through the real application Engine, project files, OCF2 and OCT1.

mod common;

use std::time::Instant;

use common::{Capture, Run};
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, Engine, ParameterId,
    SetParameterResult,
};
use serde_json::{Value, json};

fn id(index: usize) -> String {
    format!("00000000-0000-4000-8000-{index:012}")
}

type Part<'a> = (&'a str, Value, &'a [(&'a str, i32)]);

fn request(parts: &[Part<'_>]) -> CompileRequest {
    let components: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(index, (kind, parameters, _))| {
            json!({"id": id(index), "typeId": kind, "position": {"x": index, "y": 2},
               "rotationDeg": 0, "parameters": parameters})
        })
        .collect();
    let geometry: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(index, (_, _, pins))| {
            let pins: Vec<_> = pins
                .iter()
                .map(|(pin, node)| json!({"pinId": pin, "position": {"x": node, "y": 0}}))
                .collect();
            json!({"componentId": id(index), "pins": pins})
        })
        .collect();
    serde_json::from_value(json!({
        "documentRevision": "rev-1",
        "schematic": {
            "schemaVersion": 2, "components": components, "wires": [], "junctions": [],
            "rails": [], "referenceMarkers": [{"id": id(parts.len()), "position": {"x": 0, "y": 0}}]
        },
        "geometry": {"components": geometry}
    }))
    .unwrap()
}

fn single_phase() -> CompileRequest {
    request(&[
        (
            "ac_voltage_source",
            json!({"voltageRmsVolts": 10.0, "frequencyHz": 50.0,
            "phaseOffsetDegrees": 0.0, "internalResistanceOhms": 0.1}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "ac_voltmeter",
            json!({"inputResistanceOhms": 1e7, "frequencyHz": 50.0}),
            &[("pin1", 3), ("pin2", 0)],
        ),
        (
            "ac_ammeter",
            json!({"burdenResistanceOhms": 0.1, "frequencyHz": 50.0}),
            &[("pin1", 1), ("pin2", 2)],
        ),
        (
            "wattmeter",
            json!({"inputResistanceOhms": 1e7, "burdenResistanceOhms": 0.1,
            "frequencyHz": 50.0}),
            &[("v1", 3), ("v2", 0), ("i1", 2), ("i2", 3)],
        ),
        (
            "resistor",
            json!({"resistanceOhms": 100.0}),
            &[("pin1", 3), ("pin2", 0)],
        ),
    ])
}

fn save_and_open(mut request: CompileRequest) -> CompileRequest {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stage-9.ocircuit");
    write_project_file(&path, &request.schematic).unwrap();
    let document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let inputs = serde_json::to_value(&request.schematic).unwrap();
    assert_eq!(
        document, inputs,
        "the project contains only the drawn schematic"
    );
    request.schematic = read_project_file(&path).unwrap();
    assert_eq!(serde_json::to_value(&request.schematic).unwrap(), inputs);
    request
}

fn assert_transport(
    run: &mut Run,
    step: u64,
    signals: &[(usize, &str, &str)],
    expected: &open_circuit_engine::ResultSnapshot,
    windowed: bool,
) {
    let (snapshot, bytes) = run.frame_at(step);
    assert_eq!(&bytes[..4], b"OCF2");
    assert_eq!(snapshot.metadata.run_id, run.run_id);
    assert_eq!(snapshot.metadata.document_revision, "rev-1");
    assert_eq!(snapshot.metadata.layout_version, run.layout.layout_version);
    assert!(snapshot.metadata.frame_sequence > run.frame_at(1).0.metadata.frame_sequence);
    assert_eq!(snapshot.values, expected.values);

    // OCT1 can carry at most eight signals per response.
    let mut last_sequence = 0;
    for group in signals.chunks(8) {
        let keys: Vec<_> = group
            .iter()
            .map(|(index, suffix, _)| format!("{}{suffix}", id(*index)))
            .collect();
        let offsets: Vec<_> = keys
            .iter()
            .zip(group)
            .map(|(key, (_, suffix, unit))| {
                let slot = run
                    .layout
                    .slots
                    .iter()
                    .find(|slot| &slot.key == key)
                    .unwrap();
                let offset = slot.offset as usize;
                assert_eq!(serde_json::to_value(slot.unit).unwrap(), *unit);
                if windowed
                    && !suffix.starts_with(':')
                    && !matches!(*suffix, "/voltage" | "/current")
                {
                    assert_eq!(run.initial_snapshot.values[offset], 0.0);
                    assert_eq!(run.frame_at(1).0.values[offset], 0.0);
                }
                if suffix.ends_with("valid") {
                    assert_eq!(expected.values[offset], 1.0, "{key}");
                }
                offset
            })
            .collect();
        let (trace, _, _) = run.read(&keys, 1, step);
        assert_eq!(trace.header["metadata"]["runId"], run.run_id.as_str());
        assert_eq!(trace.header["metadata"]["documentRevision"], "rev-1");
        assert_eq!(
            trace.header["metadata"]["layoutVersion"],
            run.layout.layout_version
        );
        let sequence: u64 = trace.header["traceSequence"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!(sequence > last_sequence);
        last_sequence = sequence;
        for (index, ((key, (_, _, unit)), offset)) in
            keys.iter().zip(group).zip(offsets).enumerate()
        {
            assert_eq!(trace.header["signals"][index]["key"], *key);
            assert_eq!(trace.header["signals"][index]["unit"], *unit);
            assert_eq!(trace.points.first().unwrap().0, 1);
            let first = run.frame_at(1).0.values[offset];
            assert_eq!(trace.points.first().unwrap().1[index], (first, first));
            assert_eq!(trace.points.last().unwrap().0, step);
            let value = expected.values[offset];
            assert_eq!(trace.points.last().unwrap().1[index], (value, value));
        }
    }
}

fn direct_snapshot(request: CompileRequest, steps: u32) -> open_circuit_engine::ResultSnapshot {
    let mut engine = Engine::default();
    let CompileResult::Success { run_id, .. } = engine.compile(request) else {
        panic!("reference run did not compile");
    };
    engine
        .advance(AdvanceRequest {
            run_id,
            step_count: steps,
        })
        .unwrap()
}

#[test]
fn measurements_keep_their_units_and_validity_in_frames_and_traces() {
    let request = save_and_open(single_phase());
    let expected = direct_snapshot(request.clone(), 60);
    let mut run = Run::start_every_step(request);
    assert_eq!(run.advance(60), 60);
    assert_transport(
        &mut run,
        60,
        &[
            (0, ":voltage", "volt"),
            (0, ":current", "amp"),
            (1, ":voltage", "volt"),
            (1, ":current", "amp"),
            (1, "/rms_voltage", "volt"),
            (1, "/mean_voltage", "volt"),
            (1, "/peak_voltage", "volt"),
            (1, "/frequency", "hertz"),
            (1, "/frequency_valid", "dimensionless"),
            (1, "/valid", "dimensionless"),
            (2, ":voltage", "volt"),
            (2, ":current", "amp"),
            (2, "/rms_current", "amp"),
            (2, "/mean_current", "amp"),
            (2, "/peak_current", "amp"),
            (2, "/frequency", "hertz"),
            (2, "/frequency_valid", "dimensionless"),
            (2, "/valid", "dimensionless"),
            (3, "/voltage", "volt"),
            (3, "/current", "amp"),
            (3, "/rms_voltage", "volt"),
            (3, "/rms_current", "amp"),
            (3, "/real_power", "watt"),
            (3, "/reactive_power", "volt_ampere_reactive"),
            (3, "/apparent_power", "volt_ampere"),
            (3, "/power_factor", "dimensionless"),
            (3, "/phase_angle", "degree"),
            (3, "/valid", "dimensionless"),
        ],
        &expected,
        true,
    );
}

#[test]
fn a_refused_update_preserves_the_revision_and_replacing_the_run_rejects_stale_requests() {
    let request = single_phase();
    let mut run = Run::start_every_step(request.clone());
    run.advance(60);
    let keys = [format!("{}/valid", id(3))];
    let (before, _, old_trace) = run.read(&keys, 59, 60);
    let (_, refused) = run.set_parameter(&id(0), ParameterId::FrequencyHz, 500.0);
    let SetParameterResult::Failure { diagnostics } = refused else {
        panic!("out-of-range frequency was accepted");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    let (unchanged, _, _) = run.read(&keys, 59, 60);
    assert_eq!(unchanged.header["metadata"]["documentRevision"], "rev-1");
    assert_eq!(unchanged.points, before.points);
    let (update, result) = run.set_parameter(&id(0), ParameterId::FrequencyHz, 60.0);
    assert!(matches!(result, SetParameterResult::Success { .. }));
    let (changed, _, _) = run.read(&keys, 59, 60);
    assert_eq!(changed.header["metadata"]["documentRevision"], "rev-2");
    assert_eq!(
        changed.points,
        vec![(59, vec![(1.0, 1.0)]), (60, vec![(0.0, 0.0)])]
    );

    let CompileResult::Success {
        run_id,
        initial_snapshot,
        ..
    } = run.core.compile(request)
    else {
        panic!("replacement did not compile");
    };
    assert_ne!(run_id, run.run_id);
    assert_eq!(initial_snapshot.metadata.simulation_step, 0);
    assert!(!run.core.is_streaming());
    let SetParameterResult::Failure { diagnostics } = run.core.set_parameter(update) else {
        panic!("old run accepted an update");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);
    let diagnostics = run
        .core
        .read_traces(old_trace, Box::new(Capture::default()), Instant::now())
        .unwrap_err();
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);
}

#[test]
fn both_three_phase_sources_reopen_with_all_winding_parameters() {
    for (kind, voltage_parameter, signal, pins) in [
        (
            "three_phase_source_star",
            "phaseVoltageRmsVolts",
            "l1/voltage",
            &[("l1", 1), ("l2", 2), ("l3", 3), ("n", 0)][..],
        ),
        (
            "three_phase_source_delta",
            "lineVoltageRmsVolts",
            "l12/voltage",
            &[("l1", 1), ("l2", 2), ("l3", 3)][..],
        ),
    ] {
        let parameters = json!({(voltage_parameter): 230.0, "frequencyHz": 60.0,
            "phaseOffsetDegrees": 35.0, "internalResistanceOhms": 0.2, "reversedSequence": 1.0});
        let mut request = request(&[
            (kind, parameters, pins),
            (
                "resistor",
                json!({"resistanceOhms": 100.0}),
                &[("pin1", 1), ("pin2", 0)],
            ),
            (
                "resistor",
                json!({"resistanceOhms": 110.0}),
                &[("pin1", 2), ("pin2", 0)],
            ),
            (
                "resistor",
                json!({"resistanceOhms": 120.0}),
                &[("pin1", 3), ("pin2", 0)],
            ),
        ]);
        let mut run = Run::start_every_step(request.clone());
        let (_, result) = run.set_parameter(&id(0), ParameterId::PhaseOffsetDegrees, 70.0);
        let SetParameterResult::Success { snapshot, .. } = result else {
            panic!("phase update failed: {result:?}");
        };
        // Persist the edited drawing, as the frontend does after a successful live update.
        let mut schematic = serde_json::to_value(&request.schematic).unwrap();
        schematic["components"][0]["parameters"]["phaseOffsetDegrees"] = json!(70.0);
        request.schematic = serde_json::from_value(schematic).unwrap();
        let reopened_request = save_and_open(request);
        let expected = direct_snapshot(reopened_request.clone(), 10);
        let mut reopened = Run::start_every_step(reopened_request);
        assert_ne!(reopened.run_id, run.run_id);
        let key = format!("{}/{signal}", id(0));
        let offset = reopened
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == key)
            .unwrap()
            .offset as usize;
        assert_ne!(
            reopened.initial_snapshot.values[offset],
            run.initial_snapshot.values[offset]
        );
        assert_eq!(
            reopened.initial_snapshot.values[offset],
            snapshot.values[offset]
        );
        assert_eq!(reopened.advance(10), 10);
        let signals = if kind == "three_phase_source_star" {
            &[
                (0, "/l1/voltage", "volt"),
                (0, "/l1/current", "amp"),
                (0, "/l2/voltage", "volt"),
                (0, "/l2/current", "amp"),
                (0, "/l3/voltage", "volt"),
                (0, "/l3/current", "amp"),
                (0, "/n/current", "amp"),
            ][..]
        } else {
            &[
                (0, "/l12/voltage", "volt"),
                (0, "/l23/voltage", "volt"),
                (0, "/l31/voltage", "volt"),
                (0, "/l1/current", "amp"),
                (0, "/l2/current", "amp"),
                (0, "/l3/current", "amp"),
            ][..]
        };
        assert_transport(&mut reopened, 10, signals, &expected, false);
    }
}

#[test]
fn a_failed_step_in_an_ac_project_sends_the_notice_without_a_partial_frame() {
    use open_circuit_app_lib::simulation::{
        FrameSink, SimulationConfig, SimulationCore, SinkClosed,
    };
    use open_circuit_engine::StreamNotice;
    use std::sync::mpsc;
    use std::time::Duration;

    struct Sink(mpsc::Sender<Result<Vec<u8>, StreamNotice>>);
    impl FrameSink for Sink {
        fn send(&mut self, bytes: Vec<u8>) -> Result<(), SinkClosed> {
            self.0.send(Ok(bytes)).map_err(|_| SinkClosed)
        }
        fn notice(&mut self, notice: &StreamNotice) -> Result<(), SinkClosed> {
            self.0.send(Err(notice.clone())).map_err(|_| SinkClosed)
        }
    }

    // The DC fuse branch overflows its I²t calculation during the first transient step.
    let mut core = SimulationCore::new(SimulationConfig::default());
    let CompileResult::Success { run_id, .. } = core.compile(request(&[
        (
            "ac_voltage_source",
            json!({"voltageRmsVolts": 10.0, "frequencyHz": 50.0,
            "phaseOffsetDegrees": 0.0, "internalResistanceOhms": 0.1}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "ac_voltmeter",
            json!({"inputResistanceOhms": 1e7, "frequencyHz": 50.0}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts": 1e154, "internalResistanceOhms": 0.0}),
            &[("pin1", 2), ("pin2", 0)],
        ),
        (
            "fuse",
            json!({"intactResistanceOhms": 0.01, "tripI2tAmpereSquaredSeconds": 1e300}),
            &[("pin1", 2), ("pin2", 3)],
        ),
        (
            "resistor",
            json!({"resistanceOhms": 1.0}),
            &[("pin1", 3), ("pin2", 0)],
        ),
    ])) else {
        panic!("the circuit must compile before failing on advance");
    };
    let (sender, receiver) = mpsc::channel();
    let now = Instant::now();
    core.start(run_id.clone(), Box::new(Sink(sender)), now)
        .unwrap();
    core.tick(now + Duration::from_millis(5));
    let StreamNotice::RunStopped {
        run_id: stopped,
        document_revision,
        simulation_step,
        diagnostics,
    } = receiver
        .try_recv()
        .unwrap()
        .expect_err("the failed step must send a notice");
    assert_eq!(stopped, run_id);
    assert_eq!(document_revision, "rev-1");
    assert_eq!(simulation_step, 0);
    assert_eq!(diagnostics[0].code, DiagnosticCode::NonFiniteState);
    assert!(!core.is_streaming());
    assert!(receiver.try_recv().is_err());
}
