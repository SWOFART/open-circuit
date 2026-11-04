//! Test-only constitutive devices exercise the real Engine lifecycle before
//! production semiconductor catalog entries arrive in #111/#112.
use super::*;
use crate::newton::{self, Evaluation, Junction, NonlinearDevice, TerminalCurrent};
use std::cell::Cell;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static FAIL_AFTER: Cell<f64> = const { Cell::new(f64::INFINITY) };
    static TARGET: Cell<&'static str> = const { Cell::new("000000000003") };
    static FAIL_EVENT: Cell<bool> = const { Cell::new(false) };
}

struct Injection;
impl Injection {
    fn start() -> Self {
        ENABLED.set(true);
        TARGET.set("000000000003");
        FAIL_AFTER.set(f64::INFINITY);
        FAIL_EVENT.set(false);
        Self
    }
}
impl Drop for Injection {
    fn drop(&mut self) {
        ENABLED.set(false);
        FAIL_AFTER.set(f64::INFINITY);
        FAIL_EVENT.set(false);
    }
}

pub(super) fn make_test_device(spec: &DeviceSpec, mode: SolveMode<'_>) -> Option<Box<dyn Device>> {
    if !ENABLED.get()
        || !spec.component_id.as_str().ends_with(TARGET.get())
        || !matches!(spec.model, DeviceModel::Resistor { .. })
    {
        return None;
    }
    let time = match mode {
        SolveMode::Transient {
            states,
            step_seconds,
            ..
        } => states.simulation_time_seconds + step_seconds,
        _ => 0.0,
    };
    Some(Box::new(TestJunction {
        id: spec.component_id.clone(),
        terminals: spec.terminals,
        fail: time >= FAIL_AFTER.get()
            || (FAIL_EVENT.get() && matches!(mode, SolveMode::InitialState(_))),
    }))
}

struct TestJunction {
    id: StableId,
    terminals: Terminals,
    fail: bool,
}
impl NonlinearDevice for TestJunction {
    fn id(&self) -> &StableId {
        &self.id
    }
    fn junctions(&self) -> Vec<Junction> {
        vec![Junction {
            positive: self.terminals.pin1,
            negative: self.terminals.pin2,
            thermal_voltage: newton::THERMAL_VOLTAGE,
            saturation_current: 1e-14,
        }]
    }
    fn evaluate(&self, voltages: &[f64]) -> Evaluation {
        let (e, de) = newton::junction_exponential(voltages[0] / newton::THERMAL_VOLTAGE);
        let i = if self.fail { f64::NAN } else { 1e-14 * e };
        let leakage = 1e-12 * voltages[0];
        let g = 1e-14 / newton::THERMAL_VOLTAGE * de + 1e-12;
        let (a, b) = (self.terminals.pin1, self.terminals.pin2);
        Evaluation {
            currents: vec![
                TerminalCurrent {
                    node: a,
                    terms: vec![i, leakage],
                },
                TerminalCurrent {
                    node: b,
                    terms: vec![-i, -leakage],
                },
            ],
            derivatives: vec![(a, a, g), (a, b, -g), (b, a, -g), (b, b, g)],
        }
    }
}
impl Device for TestJunction {
    fn nonlinear(&self) -> Option<&dyn NonlinearDevice> {
        Some(self)
    }
    fn stamp(&self, _: &mut MnaSystem, _: &NodeVariables) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn result(&self, nodes: &[f64], _: &[f64]) -> Result<DeviceResult, Diagnostic> {
        let v = nodes[self.terminals.pin1] - nodes[self.terminals.pin2];
        let i = self.evaluate(&[v]).currents[0].terms.iter().sum();
        Ok(DeviceResult {
            component_id: self.id.clone(),
            voltage_volts: v,
            current_amps: i,
            extra_values: vec![],
            state: None,
        })
    }
}

fn request() -> CompileRequest {
    let mut request = CompileRequest {
        schematic: serde_json::from_str(include_str!(
            "../../../tests/golden/stage-1-divider.schematic.json"
        ))
        .unwrap(),
        geometry: serde_json::from_str(include_str!(
            "../../../tests/golden/stage-1-divider.geometry.json"
        ))
        .unwrap(),
        document_revision: "revision-1".into(),
        faults: None,
    };
    if let ComponentSpec::DcVoltageSource { parameters, .. } = &mut request.schematic.components[0]
    {
        parameters.internal_resistance_ohms = 0.0;
    }
    request
}
fn compiled(engine: &mut Engine) -> StableId {
    let result = engine.compile(request());
    let CompileResult::Success { run_id, .. } = result else {
        panic!("{result:?}");
    };
    run_id
}
fn history(engine: &Engine, run_id: StableId) -> serde_json::Value {
    serde_json::to_value(
        engine
            .read_traces(TraceRequest {
                run_id,
                layout_version: 1,
                signal_keys: vec![format!(
                    "{}:voltage",
                    engine.compiled.as_ref().unwrap().schematic.components[2].id()
                )],
                query: crate::TraceQuery::History {
                    start_step: 0,
                    end_step: 10,
                },
                max_points: 16,
            })
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn analytical_junction_runs_in_engine_and_replays_from_committed_step_start() {
    let _injection = Injection::start();
    let mut engine = Engine::default();
    let run_id = compiled(&mut engine);
    let snapshot = engine.latest_snapshot().unwrap();
    let v = snapshot.values[4];
    let i = snapshot.values[5];
    assert!((v - 0.692887832378056).abs() < 1e-9, "{v}");
    assert!((i - 0.004307112167621944).abs() < 1e-12);
    let first_bits = snapshot
        .values
        .iter()
        .map(|v| v.to_bits())
        .collect::<Vec<_>>();
    let mut repeated = Engine::default();
    compiled(&mut repeated);
    assert_eq!(
        first_bits,
        repeated
            .latest_snapshot()
            .unwrap()
            .values
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    );
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 3,
        })
        .unwrap();
    let before = history(&engine, run_id.clone());
    let update = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "revision-1".into(),
        document_revision: "revision-2".into(),
        component_id: request().schematic.components[0].id().clone(),
        parameter_id: ParameterId::VoltageVolts,
        value: 4.0,
    });
    assert!(matches!(update, SetParameterResult::Success { .. }));
    let after = history(&engine, run_id);
    assert!(before["points"].as_array().unwrap().len() >= 4);
    assert_eq!(before["points"][0], after["points"][0]);
    assert_eq!(before["points"][2], after["points"][2]);
    assert_ne!(before["points"][3], after["points"][3]);
    assert_eq!(
        engine.latest_snapshot().unwrap().metadata.simulation_step,
        3
    );
    assert_eq!(
        engine.latest_snapshot().unwrap().metadata.document_revision,
        "revision-2"
    );
    assert!(
        !engine
            .compiled
            .as_ref()
            .unwrap()
            .step_start_states
            .solver_memory
            .junctions
            .is_empty()
    );
}

#[test]
fn failed_compile_update_and_multi_step_advance_preserve_snapshot_history_and_solver_memory() {
    let _injection = Injection::start();
    let mut engine = Engine::default();
    let run_id = compiled(&mut engine);
    let before = serde_json::to_value(engine.latest_snapshot()).unwrap();
    let trace = history(&engine, run_id.clone());
    let memory = engine
        .compiled
        .as_ref()
        .unwrap()
        .states
        .solver_memory
        .nodes
        .clone();
    let mut bad = request();
    if let ComponentSpec::DcVoltageSource { parameters, .. } = &mut bad.schematic.components[0] {
        parameters.voltage_volts = f64::NAN;
    }
    assert!(matches!(engine.compile(bad), CompileResult::Failure { .. }));
    FAIL_AFTER.set(0.0015);
    let error = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 3,
        })
        .unwrap_err();
    assert_eq!(error[0].code, DiagnosticCode::NonFiniteState);
    assert_eq!(
        serde_json::to_value(engine.latest_snapshot()).unwrap(),
        before
    );
    assert_eq!(history(&engine, run_id.clone()), trace);
    assert_eq!(
        engine.compiled.as_ref().unwrap().states.solver_memory.nodes,
        memory
    );
    FAIL_AFTER.set(f64::INFINITY);
    assert!(
        engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1
            })
            .is_err(),
        "stopped run must not retry silently"
    );
    let update = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "revision-1".into(),
        document_revision: "revision-2".into(),
        component_id: request().schematic.components[0].id().clone(),
        parameter_id: ParameterId::VoltageVolts,
        value: 4.0,
    });
    assert!(matches!(update, SetParameterResult::Success { .. }));
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 2,
        })
        .unwrap();
    let before = serde_json::to_value(engine.latest_snapshot()).unwrap();
    let trace = history(&engine, run_id.clone());
    FAIL_AFTER.set(0.0);
    let update = engine.set_parameter(SetParameterRequest {
        run_id: run_id.clone(),
        expected_document_revision: "revision-2".into(),
        document_revision: "revision-3".into(),
        component_id: request().schematic.components[0].id().clone(),
        parameter_id: ParameterId::VoltageVolts,
        value: 3.0,
    });
    assert!(matches!(update, SetParameterResult::Failure { .. }));
    assert_eq!(
        serde_json::to_value(engine.latest_snapshot()).unwrap(),
        before
    );
    assert_eq!(history(&engine, run_id), trace);
}

#[test]
fn mixed_motor_island_preserves_the_implicitly_coupled_known_answer() {
    let mut request: CompileRequest = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-4-drawn-motor.request.json"
    ))
    .unwrap();
    let mut baseline = Engine::default();
    let CompileResult::Success {
        run_id: base_id, ..
    } = baseline.compile(request.clone())
    else {
        panic!("motor fixture");
    };
    let _injection = Injection::start();
    TARGET.set("00000000000f");
    let id = "00000000-0000-4000-8000-00000000000f";
    request.schematic.components.push(serde_json::from_value(serde_json::json!({"typeId":"resistor","id":id,"position":{"x":12,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":1000}})).unwrap());
    request.geometry.components.push(serde_json::from_value(serde_json::json!({"componentId":id,"pins":[{"pinId":"pin1","position":{"x":7,"y":0}},{"pinId":"pin2","position":{"x":5,"y":0}}]})).unwrap());
    let mut nonlinear = Engine::default();
    let result = nonlinear.compile(request);
    let CompileResult::Success { run_id, .. } = result else {
        panic!("{result:?}");
    };
    for _ in 0..20 {
        let expected = baseline
            .advance(AdvanceRequest {
                run_id: base_id.clone(),
                step_count: 1,
            })
            .unwrap();
        let actual = nonlinear
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        for (a, b) in actual.values.iter().zip(&expected.values) {
            assert!((a - b).abs() < 1e-9, "{a} != {b}");
        }
    }
}

#[test]
fn switched_nonlinear_rc_matches_the_independent_transient_fixture_and_rejects_operator_failure() {
    let _injection = Injection::start();
    TARGET.set("00000000000f");
    let mut json: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-6-drawn-switch.request.json"
    ))
    .unwrap();
    json["schematic"]["components"][0]["parameters"]["voltageVolts"] = serde_json::json!(5.0);
    json["schematic"]["components"][2]["parameters"]["resistanceOhms"] = serde_json::json!(10000.0);
    json["schematic"]["components"][3]["typeId"] = serde_json::json!("capacitor");
    json["schematic"]["components"][3]["parameters"] =
        serde_json::json!({"capacitanceFarads":1e-6,"initialVoltageVolts":0.0});
    let diode_id = "00000000-0000-4000-8000-00000000000f";
    json["schematic"]["components"].as_array_mut().unwrap().push(serde_json::json!({"typeId":"resistor","id":diode_id,"position":{"x":10,"y":0},"rotationDeg":0,"parameters":{"resistanceOhms":1000}}));
    json["geometry"]["components"].as_array_mut().unwrap().push(serde_json::json!({"componentId":diode_id,"pins":[{"pinId":"pin1","position":{"x":9,"y":0}},{"pinId":"pin2","position":{"x":11,"y":0}}]}));
    json["schematic"]["wires"][2]["points"][0]["x"] = serde_json::json!(11);
    let request: CompileRequest = serde_json::from_value(json).unwrap();
    let switch_id = request.schematic.components[1].id().clone();
    let capacitor = request.schematic.components[3].id().clone();
    let mut engine = Engine::default();
    let result = engine.compile(request);
    let CompileResult::Success { run_id, layout, .. } = result else {
        panic!("{result:?}");
    };
    engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 5,
        })
        .unwrap();
    assert!(matches!(
        engine.set_operator_state(SetOperatorStateRequest {
            run_id: run_id.clone(),
            expected_document_revision: "rev-1".into(),
            document_revision: "rev-2".into(),
            component_id: switch_id.clone(),
            position: crate::OperatorPosition::Closed
        }),
        SetOperatorStateResult::Success { .. }
    ));
    let slot = layout
        .slots
        .iter()
        .find(|s| s.component_id == capacitor && s.quantity == ResultQuantity::Voltage)
        .unwrap()
        .offset as usize;
    eprintln!(
        "canonical RC: {} internal substeps per public 1 ms step",
        engine.compiled.as_ref().unwrap().internal_substeps
    );
    for (step, expected) in [(54, 4.43286502), (104, 4.52489018), (204, 4.56192038)] {
        let count = step - engine.latest_snapshot().unwrap().metadata.simulation_step;
        let snapshot = engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: count as u32,
            })
            .unwrap();
        assert!(
            (snapshot.values[slot] - expected).abs() < 0.005,
            "step {step}: {}",
            snapshot.values[slot]
        );
    }
    let before = serde_json::to_value(engine.latest_snapshot()).unwrap();
    FAIL_AFTER.set(0.0);
    assert!(matches!(
        engine.set_operator_state(SetOperatorStateRequest {
            run_id,
            expected_document_revision: "rev-2".into(),
            document_revision: "rev-3".into(),
            component_id: switch_id,
            position: crate::OperatorPosition::Open
        }),
        SetOperatorStateResult::Failure { .. }
    ));
    assert_eq!(
        serde_json::to_value(engine.latest_snapshot()).unwrap(),
        before
    );
}

#[test]
fn nonlinear_same_time_fuse_failure_rolls_back_the_transition_and_history() {
    let _injection = Injection::start();
    TARGET.set("00000000000f");
    let mut json: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-4-drawn-fuse.request.json"
    ))
    .unwrap();
    let id = "00000000-0000-4000-8000-00000000000f";
    let source_pins = json["geometry"]["components"][0]["pins"].clone();
    json["schematic"]["components"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "typeId":"resistor", "id":id, "position":{"x":20,"y":0}, "rotationDeg":0,
            "parameters":{"resistanceOhms":1000}
        }));
    json["geometry"]["components"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "componentId":id, "pins":[
                {"pinId":"pin1", "position":source_pins[1]["position"]},
                {"pinId":"pin2", "position":source_pins[0]["position"]}]
        }));
    let request: CompileRequest = serde_json::from_value(json).unwrap();
    let fuse = request.schematic.components[1].id().clone();
    let mut engine = Engine::default();
    let CompileResult::Success { run_id, .. } = engine.compile(request.clone()) else {
        panic!("nonlinear fuse fixture");
    };
    let before = serde_json::to_value(engine.latest_snapshot()).unwrap();
    let trace = history(&engine, run_id.clone());
    let memory = engine
        .compiled
        .as_ref()
        .unwrap()
        .states
        .solver_memory
        .nodes
        .clone();
    FAIL_EVENT.set(true);
    let errors = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 2,
        })
        .unwrap_err();
    assert_eq!(errors[0].code, DiagnosticCode::NonFiniteState);
    assert_eq!(
        serde_json::to_value(engine.latest_snapshot()).unwrap(),
        before
    );
    assert_eq!(history(&engine, run_id), trace);
    let states = &engine.compiled.as_ref().unwrap().states;
    assert_eq!(states.fuses[&fuse].state, FuseState::Intact);
    assert_eq!(
        states.fuses[&fuse].accumulated_i2t_ampere_squared_seconds,
        0.0
    );
    assert_eq!(states.solver_memory.nodes, memory);
    // The same fixture succeeds and commits the same-time transition without injection.
    FAIL_EVENT.set(false);
    let mut successful = Engine::default();
    let CompileResult::Success { run_id, .. } = successful.compile(request) else {
        panic!("fixture");
    };
    successful
        .advance(AdvanceRequest {
            run_id,
            step_count: 2,
        })
        .unwrap();
    assert_eq!(
        successful.compiled.as_ref().unwrap().states.fuses[&fuse].state,
        FuseState::Tripped
    );
}
