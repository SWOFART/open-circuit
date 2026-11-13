//! Actual Engine comparisons to independent #109 Ebers–Moll reference answers.
use open_circuit_engine::*;
use serde_json::{Value, json};
fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}
fn num(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_str()?.parse().ok()).unwrap()
}
fn references() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/golden/stage-7-analytical.expected.json"
    ))
    .unwrap()
}
fn manifest() -> Value {
    serde_json::from_str(include_str!("../../../tests/golden/stage-7-manifest.json")).unwrap()
}
fn component(n: u64, kind: &str, parameters: Value) -> Value {
    json!({"id":id(n),"typeId":kind,"position":{"x":n as i32*4,"y":12},"rotationDeg":0,"parameters":parameters})
}
fn geometry(n: u64, pins: &[(&str, i32, i32)]) -> Value {
    json!({"componentId":id(n),"pins":pins.iter().map(|(pin,x,y)|json!({"pinId":pin,"position":{"x":x,"y":y}})).collect::<Vec<_>>()})
}
fn circuit(c: &Value) -> CompileRequest {
    let swapped = c["swapped"].as_bool().unwrap();
    let (collector, emitter) = if swapped {
        ((0, 0), (8, 4))
    } else {
        ((8, 4), (0, 0))
    };
    serde_json::from_value(json!({"documentRevision":"rev-1", "schematic":{
        "schemaVersion":2,"rails":[],"components":[
            component(1,"dc_voltage_source",json!({"voltageVolts":num(&c["vcc"]),"internalResistanceOhms":0})),
            component(2,"dc_voltage_source",json!({"voltageVolts":num(&c["vbb"]),"internalResistanceOhms":0})),
            component(3,"resistor",json!({"resistanceOhms":num(&c["rc"])})),
            component(4,"resistor",json!({"resistanceOhms":num(&c["rb"])})),
            component(5,if num(&c["polarity"])>0.0 {"bjt_npn"}else{"bjt_pnp"},json!({"saturationCurrentAmps":num(&c["i_s"]),"forwardBeta":num(&c["bf"]),"reverseBeta":num(&c["br"])}))
        ],"wires":[],"junctions":[],"referenceMarkers":[{"id":id(100),"position":{"x":0,"y":0}}]},
        "geometry":{"components":[geometry(1,&[("pin1",0,4),("pin2",0,0)]),geometry(2,&[("pin1",4,4),("pin2",0,0)]),
            geometry(3,&[("pin1",0,4),("pin2",8,4)]),geometry(4,&[("pin1",4,4),("pin2",4,0)]),
            geometry(5,&[("collector",collector.0,collector.1),("base",4,0),("emitter",emitter.0,emitter.1)])]}
    })).unwrap()
}
fn compile(e: &mut Engine, r: CompileRequest) -> (StableId, ResultLayout) {
    let result = e.compile(r);
    let CompileResult::Success { run_id, layout, .. } = result else {
        panic!("{result:?}")
    };
    (run_id, layout)
}
fn get(s: &ResultSnapshot, l: &ResultLayout, suffix: &str) -> f64 {
    let key = format!("{}/{suffix}", id(5));
    s.values[l.slots.iter().find(|s| s.key == key).unwrap().offset as usize]
}
fn slot(s: &ResultSnapshot, l: &ResultLayout, key: &str) -> f64 {
    s.values[l.slots.iter().find(|s| s.key == key).unwrap().offset as usize]
}
fn close(a: f64, b: f64, rel: f64, abs: f64) {
    assert!((a - b).abs() <= rel * b.abs() + abs, "{a} vs {b}");
}
fn active() -> Value {
    references()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "npn-active")
        .unwrap()["circuit"]
        .clone()
}

#[test]
fn all_bias_regions_and_pnp_mirrors_match_independent_fixtures_with_five_named_slots() {
    let refs = references();
    let manifest = manifest();
    for case in refs["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "bjt_dc")
    {
        let c = &case["circuit"];
        let expected = &case["contract"];
        let tol = &manifest["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == case["id"])
            .unwrap()["engineTolerance"];
        let mut e = Engine::default();
        let (_, l) = compile(&mut e, circuit(c));
        let s = e.latest_snapshot().unwrap();
        let keys = l
            .slots
            .iter()
            .filter(|s| s.component_id.as_str() == id(5))
            .map(|s| s.key.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            [
                "base_emitter/voltage",
                "collector_emitter/voltage",
                "collector/current",
                "base/current",
                "emitter/current"
            ]
            .map(|k| format!("{}/{k}", id(5)))
        );
        let load = num(&expected["loadNodeVoltageVolts"]);
        let emitter = if c["swapped"] == true { load } else { 0.0 };
        let collector = if c["swapped"] == true { 0.0 } else { load };
        close(
            get(s, &l, "base_emitter/voltage"),
            num(&expected["baseVoltageVolts"]) - emitter,
            0.0,
            num(&tol["voltsAbsolute"]),
        );
        close(
            get(s, &l, "collector_emitter/voltage"),
            collector - emitter,
            0.0,
            num(&tol["voltsAbsolute"]),
        );
        for (key, answer) in [
            ("collector/current", "collectorCurrentAmps"),
            ("base/current", "baseCurrentAmps"),
            ("emitter/current", "emitterCurrentAmps"),
        ] {
            close(
                get(s, &l, key),
                num(&expected[answer]),
                tol["ampsRelative"].as_f64().unwrap_or(0.0),
                tol["ampsAbsoluteFloor"]
                    .as_f64()
                    .or_else(|| tol["ampsAbsolute"].as_f64())
                    .unwrap(),
            );
        }
        let (ic, ib, ie) = (
            get(s, &l, "collector/current"),
            get(s, &l, "base/current"),
            get(s, &l, "emitter/current"),
        );
        assert_eq!(ic + ib + ie, 0.0);
        close(slot(s, &l, &format!("{}:current", id(4))), ib, 1e-9, 1e-13);
        close(
            slot(s, &l, &format!("{}:current", id(3))),
            if c["swapped"] == true { ie } else { ic },
            1e-9,
            1e-13,
        );
        let source_power =
            num(&c["vcc"]) * slot(s, &l, &format!("{}:current", id(3))) + num(&c["vbb"]) * ib;
        let dissipated = (num(&c["vcc"]) - load).powi(2) / num(&c["rc"])
            + (num(&c["vbb"]) - num(&expected["baseVoltageVolts"])).powi(2) / num(&c["rb"])
            + collector * ic
            + num(&expected["baseVoltageVolts"]) * ib
            + emitter * ie;
        close(source_power, dissipated, 1e-8, 1e-12);
        eprintln!(
            "{}: dIc={:.3e}, dIb={:.3e}",
            case["id"],
            ic - num(&expected["collectorCurrentAmps"]),
            ib - num(&expected["baseCurrentAmps"])
        );
    }
}

#[test]
fn both_polarities_and_swapped_nodes_are_exact_mirrors_on_this_build() {
    let mut c = active();
    for swapped in [false, true] {
        c["swapped"] = json!(swapped);
        let mut npn = Engine::default();
        let (_, l) = compile(&mut npn, circuit(&c));
        let mut mirrored = c.clone();
        mirrored["polarity"] = json!(-1);
        mirrored["vcc"] = json!(-num(&c["vcc"]));
        mirrored["vbb"] = json!(-num(&c["vbb"]));
        let mut pnp = Engine::default();
        let (_, lp) = compile(&mut pnp, circuit(&mirrored));
        for key in [
            "base_emitter/voltage",
            "collector_emitter/voltage",
            "collector/current",
            "base/current",
            "emitter/current",
        ] {
            close(
                get(npn.latest_snapshot().unwrap(), &l, key),
                -get(pnp.latest_snapshot().unwrap(), &lp, key),
                1e-10,
                1e-14,
            );
        }
    }
}

#[test]
fn parameter_limits_and_cross_combinations_are_finite_and_invalid_edits_rollback() {
    for p in [1, -1] {
        for (is, bf, br) in [
            (1e-18, 1.0, 100.0),
            (1e-9, 1000.0, 0.01),
            (1e-14, 1.0, 100.0),
        ] {
            let mut c = active();
            c["polarity"] = json!(p);
            c["vcc"] = json!(10 * p);
            c["vbb"] = json!(5 * p);
            c["i_s"] = json!(is);
            c["bf"] = json!(bf);
            c["br"] = json!(br);
            let mut e = Engine::default();
            compile(&mut e, circuit(&c));
            assert!(
                e.latest_snapshot()
                    .unwrap()
                    .values
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
    }
    for (parameter, bad) in [
        (ParameterId::SaturationCurrentAmps, 1e-19),
        (ParameterId::ForwardBeta, 0.99),
        (ParameterId::ForwardBeta, 1001.0),
        (ParameterId::ReverseBeta, 0.009),
        (ParameterId::ReverseBeta, 101.0),
        (ParameterId::ReverseBeta, f64::NAN),
    ] {
        let mut e = Engine::default();
        let (run, _) = compile(&mut e, circuit(&active()));
        let before = serde_json::to_value(e.latest_snapshot()).unwrap();
        let result = e.set_parameter(SetParameterRequest {
            run_id: run,
            expected_document_revision: "rev-1".into(),
            document_revision: "rev-2".into(),
            component_id: StableId::parse(id(5)).unwrap(),
            parameter_id: parameter,
            value: bad,
        });
        let SetParameterResult::Failure { diagnostics, .. } = result else {
            panic!("{result:?}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidParameter)
        );
        assert_eq!(serde_json::to_value(e.latest_snapshot()).unwrap(), before);
        let mut r = circuit(&active());
        let ComponentSpec::BjtNpn { parameters, .. } = &mut r.schematic.components[4] else {
            unreachable!()
        };
        match parameter {
            ParameterId::SaturationCurrentAmps => parameters.saturation_current_amps = bad,
            ParameterId::ForwardBeta => parameters.forward_beta = bad,
            _ => parameters.reverse_beta = bad,
        };
        assert!(project::validate_project_schematic(&r.schematic).is_err());
        assert!(matches!(
            Engine::default().compile(r),
            CompileResult::Failure { .. }
        ));
    }
}

fn ideal_base(source: f64) -> CompileRequest {
    let mut c = active();
    c["vbb"] = json!(source);
    let mut r = circuit(&c);
    r.schematic.components.remove(3);
    r.geometry.components.remove(3);
    r.geometry.components[3].pins[1].position = GridPoint { x: 4, y: 4 };
    r
}
#[test]
fn ideal_forward_junction_is_refused_and_live_failure_preserves_snapshot_and_run() {
    for _ in 0..2 {
        let result = Engine::default().compile(ideal_base(5.0));
        let CompileResult::Failure { diagnostics } = result else {
            panic!("{result:?}")
        };
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::ModelRangeExceeded
                    && d.references.contains(&DiagnosticReference::Component {
                        component_id: StableId::parse(id(5)).unwrap()
                    })),
            "{diagnostics:?}"
        );
    }
    let mut e = Engine::default();
    let (run, _) = compile(&mut e, ideal_base(0.65));
    let before = serde_json::to_value(e.latest_snapshot()).unwrap();
    let result = e.set_parameter(SetParameterRequest {
        run_id: run.clone(),
        expected_document_revision: "rev-1".into(),
        document_revision: "bad".into(),
        component_id: StableId::parse(id(2)).unwrap(),
        parameter_id: ParameterId::VoltageVolts,
        value: 5.0,
    });
    assert!(
        matches!(result, SetParameterResult::Failure { .. }),
        "{result:?}"
    );
    assert_eq!(serde_json::to_value(e.latest_snapshot()).unwrap(), before);
    e.advance(AdvanceRequest {
        run_id: run,
        step_count: 1,
    })
    .unwrap();
}

#[test]
fn persisted_inputs_only_roundtrip_for_both_types_and_reject_unknown_fields() {
    for p in [1, -1] {
        let mut c = active();
        c["polarity"] = json!(p);
        let r = circuit(&c);
        let bytes = serde_json::to_vec(&r.schematic).unwrap();
        assert_eq!(project::decode_project_json(&bytes).unwrap(), r.schematic);
        let mut value = serde_json::to_value(&r.schematic).unwrap();
        value["components"][4]["parameters"]["collectorCurrentAmps"] = json!(1.0);
        assert!(project::decode_project_json(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn live_beta_edit_replays_current_step_and_repeat_runs_are_bit_identical() {
    let r = circuit(&active());
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
    let query = TraceRequest {
        run_id: run.clone(),
        layout_version: 1,
        signal_keys: vec![format!("{}/collector/current", id(5))],
        query: TraceQuery::History {
            start_step: 0,
            end_step: 3,
        },
        max_points: 16,
    };
    let before = serde_json::to_value(a.read_traces(query.clone()).unwrap()).unwrap();
    let result = a.set_parameter(SetParameterRequest {
        run_id: run,
        expected_document_revision: "rev-1".into(),
        document_revision: "rev-2".into(),
        component_id: StableId::parse(id(5)).unwrap(),
        parameter_id: ParameterId::ForwardBeta,
        value: 30.0,
    });
    let SetParameterResult::Success { snapshot, .. } = result else {
        panic!("{result:?}")
    };
    assert_eq!(snapshot.metadata.simulation_step, 3);
    assert_eq!(snapshot.metadata.document_revision, "rev-2");
    assert!(get(&snapshot, &l, "collector/current") < get(&old, &l, "collector/current"));
    let after = serde_json::to_value(a.read_traces(query).unwrap()).unwrap();
    assert_eq!(before["points"][0], after["points"][0]);
    assert_eq!(before["points"][2], after["points"][2]);
    assert_ne!(before["points"][3], after["points"][3]);
}

#[test]
fn nonlinear_capacitor_and_stage6_switch_preserve_initial_state_then_reach_known_bias_point() {
    let mut c = active();
    c["vbb"] = json!(5);
    let mut r = circuit(&c);
    let cap = component(
        6,
        "capacitor",
        json!({"capacitanceFarads":1e-5,"initialVoltageVolts":0}),
    );
    let switch = component(7, "switch_spst", json!({"closedResistanceOhms":0.01}));
    r.schematic.components.extend([
        serde_json::from_value(cap).unwrap(),
        serde_json::from_value(switch).unwrap(),
    ]);
    r.geometry.components.extend([
        serde_json::from_value(geometry(6, &[("pin1", 8, 4), ("pin2", 0, 0)])).unwrap(),
        serde_json::from_value(geometry(7, &[("pin1", 4, 4), ("pin2", 4, 8)])).unwrap(),
    ]);
    r.geometry.components[3].pins[0].position = GridPoint { x: 4, y: 8 };
    let mut e = Engine::default();
    let (run, l) = compile(&mut e, r);
    assert_eq!(
        get(
            e.latest_snapshot().unwrap(),
            &l,
            "collector_emitter/voltage"
        ),
        0.0
    );
    let previous = e
        .advance(AdvanceRequest {
            run_id: run.clone(),
            step_count: 39,
        })
        .unwrap();
    let start = get(&previous, &l, "collector_emitter/voltage");
    let s = e
        .advance(AdvanceRequest {
            run_id: run.clone(),
            step_count: 1,
        })
        .unwrap();
    let v = get(&s, &l, "collector_emitter/voltage");
    close(v, 10.0 * (1.0 - (-4.0_f64).exp()), 0.0, 0.005);
    assert!(matches!(
        e.set_operator_state(SetOperatorStateRequest {
            run_id: run.clone(),
            expected_document_revision: "rev-1".into(),
            document_revision: "rev-2".into(),
            component_id: StableId::parse(id(7)).unwrap(),
            position: OperatorPosition::Closed
        }),
        SetOperatorStateResult::Success { .. }
    ));
    // A current-step contact edit replays the newest interval from its committed
    // start using event-related backward Euler; it does not freeze its old end.
    let expected_current = references()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "npn-active")
        .unwrap()["contract"]["collectorCurrentAmps"]
        .as_f64()
        .unwrap();
    let expected = (0.01 * start + 0.01 - expected_current) / 0.011;
    assert_eq!(e.latest_snapshot().unwrap().metadata.simulation_step, 40);
    close(
        get(
            e.latest_snapshot().unwrap(),
            &l,
            "collector_emitter/voltage",
        ),
        expected,
        0.0,
        1e-7,
    );
    let s = e
        .advance(AdvanceRequest {
            run_id: run,
            step_count: 200,
        })
        .unwrap();
    let answer = references()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "npn-active")
        .unwrap()["contract"]["loadNodeVoltageVolts"]
        .as_f64()
        .unwrap();
    close(get(&s, &l, "collector_emitter/voltage"), answer, 0.0, 1e-5);
}
