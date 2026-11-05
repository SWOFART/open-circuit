//! Real Engine against the independent #109 model-equation answers.
use open_circuit_engine::*;
use serde_json::{Value, json};

const D: &str = "00000000-0000-4000-8000-000000000003";
const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
fn num(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_str()?.parse().ok()).unwrap()
}
fn reference() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/golden/stage-7-analytical.expected.json"
    ))
    .unwrap()
}
fn manifest() -> Value {
    serde_json::from_str(include_str!("../../../tests/golden/stage-7-manifest.json")).unwrap()
}
fn request(source: f64, resistance: f64, is: f64, n: f64, reversed: bool) -> CompileRequest {
    let mut schematic: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.schematic.json"
    ))
    .unwrap();
    let mut geometry: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.geometry.json"
    ))
    .unwrap();
    schematic["components"][0]["parameters"] =
        json!({"voltageVolts":source,"internalResistanceOhms":0});
    schematic["components"][1]["parameters"] = json!({"resistanceOhms":resistance});
    schematic["components"][2]["typeId"] = json!("diode");
    schematic["components"][2]["parameters"] =
        json!({"saturationCurrentAmps":is,"emissionCoefficient":n});
    geometry["components"][2]["pins"][0]["pinId"] =
        json!(if reversed { "cathode" } else { "anode" });
    geometry["components"][2]["pins"][1]["pinId"] =
        json!(if reversed { "anode" } else { "cathode" });
    CompileRequest {
        schematic: serde_json::from_value(schematic).unwrap(),
        geometry: serde_json::from_value(geometry).unwrap(),
        document_revision: "rev-1".into(),
        faults: None,
    }
}
fn compile(engine: &mut Engine, request: CompileRequest) -> (StableId, ResultLayout) {
    let result = engine.compile(request);
    let CompileResult::Success { run_id, layout, .. } = result else {
        panic!("{result:?}")
    };
    (run_id, layout)
}
fn ideal_request(source: f64) -> CompileRequest {
    let mut r = request(source, 1000.0, 1e-14, 1.0, false);
    r.schematic.components.remove(1);
    r.geometry.components.remove(1);
    r.schematic.wires.clear();
    for i in 0..2 {
        r.geometry.components[1].pins[i].position = r.geometry.components[0].pins[i].position;
    }
    r
}
fn value(snapshot: &ResultSnapshot, layout: &ResultLayout, key: &str) -> f64 {
    snapshot.values[layout.slots.iter().find(|s| s.key == key).unwrap().offset as usize]
}
fn current(snapshot: &ResultSnapshot, layout: &ResultLayout) -> f64 {
    value(snapshot, layout, &format!("{D}:current"))
}
fn voltage(snapshot: &ResultSnapshot, layout: &ResultLayout) -> f64 {
    value(snapshot, layout, &format!("{D}:voltage"))
}
fn close(a: f64, b: f64, relative: f64, absolute: f64) {
    assert!((a - b).abs() <= relative * b.abs() + absolute, "{a} vs {b}");
}

#[test]
fn independent_dc_and_sweep_fixtures_match_in_both_orientations() {
    let refs = reference();
    let manifest = manifest();
    for case in refs["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "diode_dc" || c["kind"] == "diode_sweep")
    {
        let tolerance = &manifest["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == case["id"])
            .unwrap()["engineTolerance"];
        let c = &case["circuit"];
        let points = if case["kind"] == "diode_sweep" {
            case["contract"]["points"].as_array().unwrap().clone()
        } else {
            vec![
                json!({"sourceVolts":num(&c["source"]),"diodeVoltageVolts":case["contract"]["diodeVoltageVolts"],"diodeCurrentAmps":case["contract"]["diodeCurrentAmps"]}),
            ]
        };
        for p in points {
            for reversed in [false, true] {
                let sign = if reversed { -1.0 } else { 1.0 };
                let source = sign * num(&p["sourceVolts"]);
                let mut engine = Engine::default();
                let (_, layout) = compile(
                    &mut engine,
                    request(
                        source,
                        num(&c["resistance"]),
                        num(&c["i_s"]),
                        num(&c["n"]),
                        reversed,
                    ),
                );
                let s = engine.latest_snapshot().unwrap();
                let v = voltage(s, &layout);
                let i = current(s, &layout);
                close(
                    v,
                    num(&p["diodeVoltageVolts"]),
                    0.0,
                    num(&tolerance["voltsAbsolute"]),
                );
                close(
                    i,
                    num(&p["diodeCurrentAmps"]),
                    tolerance["ampsRelative"].as_f64().unwrap_or(0.0),
                    tolerance["ampsAbsoluteFloor"]
                        .as_f64()
                        .or_else(|| tolerance["ampsAbsolute"].as_f64())
                        .unwrap(),
                );
                let r_i = value(s, &layout, "00000000-0000-4000-8000-000000000002:current");
                close(r_i, sign * i, 1e-9, 1e-14);
                assert!(i * v >= 0.0);
                close(
                    source * r_i,
                    num(&c["resistance"]) * r_i * r_i + v * i,
                    1e-9,
                    1e-12,
                );
                eprintln!(
                    "{} reversed={reversed}: dV={:.3e}, dI={:.3e}",
                    case["id"],
                    v - num(&p["diodeVoltageVolts"]),
                    i - num(&p["diodeCurrentAmps"])
                );
            }
        }
    }
}

#[test]
fn bounded_parameters_near_zero_and_extreme_reverse_bias_remain_finite() {
    for is in [1e-18, 1e-9] {
        for n in [1.0, 2.0] {
            for source in [-1e6, -1e-10, 0.0, 1e-10, 5.0] {
                let mut e = Engine::default();
                let (_, l) = compile(&mut e, request(source, 1000.0, is, n, false));
                let s = e.latest_snapshot().unwrap();
                let v = voltage(s, &l);
                let i = current(s, &l);
                assert!(s.values.iter().all(|v| v.is_finite()));
                close((source - v) / 1000.0, i, 1e-9, 1e-12);
                assert!(v * i >= 0.0);
            }
        }
    }
}

#[test]
fn compile_project_and_live_edit_boundaries_reject_invalid_inputs_atomically() {
    for (is, n) in [
        (0.0, 1.0),
        (1e-19, 1.0),
        (1e-8, 1.0),
        (1e-14, 0.99),
        (1e-14, 2.01),
        (f64::NAN, 1.0),
        (1e-14, f64::INFINITY),
    ] {
        let mut r = request(5.0, 1000.0, 1e-14, 1.0, false);
        let ComponentSpec::Diode { parameters, .. } = &mut r.schematic.components[2] else {
            unreachable!()
        };
        parameters.saturation_current_amps = is;
        parameters.emission_coefficient = n;
        assert!(
            r.schematic
                .validate()
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidParameter)
        );
        assert!(project::validate_project_schematic(&r.schematic).is_err());
        assert!(matches!(
            Engine::default().compile(r),
            CompileResult::Failure { .. }
        ));
    }
    let mut e = Engine::default();
    let (run, l) = compile(&mut e, ideal_request(1.5));
    let before = serde_json::to_value(e.latest_snapshot()).unwrap();
    for (parameter, value) in [
        (ParameterId::EmissionCoefficient, 2.1),
        (ParameterId::SaturationCurrentAmps, 1e-19),
        (ParameterId::VoltageVolts, 5.0),
    ] {
        let result = e.set_parameter(SetParameterRequest {
            run_id: run.clone(),
            expected_document_revision: "rev-1".into(),
            document_revision: "bad".into(),
            component_id: StableId::parse(if parameter == ParameterId::VoltageVolts {
                SOURCE
            } else {
                D
            })
            .unwrap(),
            parameter_id: parameter,
            value,
        });
        let SetParameterResult::Failure { diagnostics, .. } = result else {
            panic!("{result:?}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidParameter
                    || d.code == DiagnosticCode::ModelRangeExceeded)
        );
        assert_eq!(serde_json::to_value(e.latest_snapshot()).unwrap(), before);
    }
    assert!(voltage(e.latest_snapshot().unwrap(), &l) < 2.0);
}

#[test]
fn model_range_failure_is_repeatable_and_references_the_diode() {
    let r = ideal_request(5.0);
    for _ in 0..2 {
        let result = Engine::default().compile(r.clone());
        let CompileResult::Failure { diagnostics } = result else {
            panic!("{result:?}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::ModelRangeExceeded
                    && d.references.contains(&DiagnosticReference::Component {
                        component_id: StableId::parse(D).unwrap()
                    }))
        );
    }
}

#[test]
fn project_roundtrip_saves_model_inputs_only_and_old_documents_still_decode() {
    let r = request(5.0, 1000.0, 1e-14, 1.8, false);
    let bytes = serde_json::to_vec(&r.schematic).unwrap();
    assert_eq!(project::decode_project_json(&bytes).unwrap(), r.schematic);
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("solver") && !text.contains("simulationStep"));
    assert!(
        project::decode_project_json(include_bytes!(
            "../../../tests/golden/stage-1-divider.schematic.json"
        ))
        .is_ok()
    );
    let unknown = text.replace("\"diode\"", "\"future_diode\"");
    assert!(project::decode_project_json(unknown.as_bytes()).is_err());
}

#[test]
fn live_model_updates_recompute_current_step_and_preserve_prior_trace_samples() {
    let r = request(5.0, 1000.0, 1e-14, 1.0, false);
    let mut a = Engine::default();
    let mut b = Engine::default();
    let (run, l) = compile(&mut a, r.clone());
    let (other, _) = compile(&mut b, r);
    let old = a
        .advance(AdvanceRequest {
            run_id: run.clone(),
            step_count: 3,
        })
        .unwrap();
    let repeated = b
        .advance(AdvanceRequest {
            run_id: other,
            step_count: 3,
        })
        .unwrap();
    assert_eq!(
        old.values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        repeated
            .values
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    );
    let trace_request = TraceRequest {
        run_id: run.clone(),
        layout_version: 1,
        signal_keys: vec![format!("{D}:voltage")],
        query: TraceQuery::History {
            start_step: 0,
            end_step: 3,
        },
        max_points: 16,
    };
    let before = serde_json::to_value(a.read_traces(trace_request.clone()).unwrap()).unwrap();
    let result = a.set_parameter(SetParameterRequest {
        run_id: run,
        expected_document_revision: "rev-1".into(),
        document_revision: "rev-2".into(),
        component_id: StableId::parse(D).unwrap(),
        parameter_id: ParameterId::EmissionCoefficient,
        value: 1.8,
    });
    let SetParameterResult::Success { snapshot, .. } = result else {
        panic!("{result:?}")
    };
    assert_eq!(snapshot.metadata.simulation_step, 3);
    assert_eq!(snapshot.metadata.document_revision, "rev-2");
    assert!(voltage(&snapshot, &l) > voltage(&old, &l));
    let after = serde_json::to_value(a.read_traces(trace_request).unwrap()).unwrap();
    assert_eq!(before["points"][0], after["points"][0]);
    assert_eq!(before["points"][2], after["points"][2]);
    assert_ne!(before["points"][3], after["points"][3]);
}

#[test]
fn switched_rc_and_reversed_rc_match_independent_timeline() {
    let refs = reference();
    for reversed in [false, true] {
        let mut r: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/stage-6-drawn-switch.request.json"
        ))
        .unwrap();
        r["schematic"]["components"][0]["parameters"]["voltageVolts"] = json!(5.0);
        r["schematic"]["components"][2]["parameters"]["resistanceOhms"] = json!(10000.0);
        r["schematic"]["components"][3]["typeId"] = json!("capacitor");
        r["schematic"]["components"][3]["parameters"] =
            json!({"capacitanceFarads":1e-6,"initialVoltageVolts":0.0});
        let id = "00000000-0000-4000-8000-00000000000f";
        r["schematic"]["components"].as_array_mut().unwrap().push(json!({"typeId":"diode","id":id,"position":{"x":10,"y":0},"rotationDeg":0,"parameters":{"saturationCurrentAmps":1e-14,"emissionCoefficient":1}}));
        r["geometry"]["components"].as_array_mut().unwrap().push(json!({"componentId":id,"pins":[{"pinId":if reversed {"cathode"}else{"anode"},"position":{"x":9,"y":0}},{"pinId":if reversed {"anode"}else{"cathode"},"position":{"x":11,"y":0}}]}));
        r["schematic"]["wires"][2]["points"][0]["x"] = json!(11);
        let r: CompileRequest = serde_json::from_value(r).unwrap();
        let switch = r.schematic.components[1].id().clone();
        let capacitor = r.schematic.components[3].id().clone();
        let mut e = Engine::default();
        let (run, l) = compile(&mut e, r);
        e.advance(AdvanceRequest {
            run_id: run.clone(),
            step_count: 4,
        })
        .unwrap();
        assert!(matches!(
            e.set_operator_state(SetOperatorStateRequest {
                run_id: run.clone(),
                expected_document_revision: "rev-1".into(),
                document_revision: "rev-2".into(),
                component_id: switch,
                position: OperatorPosition::Closed
            }),
            SetOperatorStateResult::Success { .. }
        ));
        let id = if reversed {
            "rc-diode-reversed"
        } else {
            "rc-diode-charging"
        };
        let case = refs["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        for (time, expected) in case["contract"]["capacitorVoltsAtSeconds"]
            .as_object()
            .unwrap()
        {
            let step = (time.parse::<f64>().unwrap() * 1000.0).round() as u64;
            let count = step - e.latest_snapshot().unwrap().metadata.simulation_step;
            let s = e
                .advance(AdvanceRequest {
                    run_id: run.clone(),
                    step_count: count as u32,
                })
                .unwrap();
            let v = value(&s, &l, &format!("{capacitor}:voltage"));
            close(v, num(expected), 0.0, 0.005);
            eprintln!("{id} {time}s: dV={:.3e}", v - num(expected));
        }
    }
}
