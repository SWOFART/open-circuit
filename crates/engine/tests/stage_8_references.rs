//! Stage 8 independent data and executable wire baselines (#129).
//! Rail JSON deliberately stays untyped until #130; no rail connectivity is implemented here.

use std::collections::BTreeSet;

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticSeverity, Engine, ResultLayout,
    ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest,
    SetParameterResult, StableId, decode_project_json, netlist::derive_netlist,
};
use serde_json::Value;

fn data(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn list(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn number(v: &Value) -> f64 {
    v.as_f64().filter(|n| n.is_finite()).unwrap()
}

fn close(a: f64, b: f64, abs: f64, rel: f64) {
    assert!(a.is_finite() && b.is_finite());
    assert!((a - b).abs() <= abs + rel * b.abs(), "{a} != {b}");
}

fn wire_request(pair: &Value) -> CompileRequest {
    CompileRequest {
        schematic: decode_project_json(&serde_json::to_vec(&pair["wireSchematic"]).unwrap())
            .unwrap(),
        geometry: serde_json::from_value(pair["geometry"].clone()).unwrap(),
        document_revision: format!("reference-{}", text(&pair["id"])),
        faults: None,
    }
}

fn partitions(nodes: &[open_circuit_engine::netlist::NetNode]) -> BTreeSet<BTreeSet<String>> {
    nodes
        .iter()
        .filter(|node| !node.pins.is_empty())
        .map(|node| {
            node.pins
                .iter()
                .map(|p| format!("{}/{}", p.component_id, p.pin_id.as_str()))
                .collect()
        })
        .collect()
}

fn expected_partitions(pair: &Value) -> BTreeSet<BTreeSet<String>> {
    list(&pair["connectedPins"])
        .iter()
        .map(|group| list(group).iter().map(|v| text(v).to_owned()).collect())
        .collect()
}

#[test]
fn paired_drawings_preserve_component_pin_and_signal_identity() {
    let data = data("stage-8-circuits.json");
    let pairs = list(&data["circuits"]);
    assert_eq!(pairs.len(), 6);
    for pair in pairs {
        let rails = &pair["railSchematic"];
        let wires = &pair["wireSchematic"];
        assert_eq!(rails["schemaVersion"], 2);
        assert_eq!(wires["schemaVersion"], 1);
        for key in ["components", "junctions", "referenceMarkers"] {
            assert_eq!(rails[key], wires[key], "{}: {key}", pair["id"]);
        }
        assert_eq!(
            list(&rails["rails"]).len(),
            list(&pair["railWireMap"]).len()
        );
        assert_eq!(
            list(&wires["wires"]).len(),
            list(&rails["wires"]).len() + list(&rails["rails"]).len()
        );
        for wire in list(&rails["wires"]) {
            assert!(list(&wires["wires"]).contains(wire));
        }
        let mut ids = BTreeSet::new();
        for key in [
            "components",
            "wires",
            "junctions",
            "referenceMarkers",
            "rails",
        ] {
            for object in list(&rails[key]) {
                assert!(ids.insert(text(&object["id"])), "duplicate object ID");
                StableId::parse(text(&object["id"])).unwrap();
            }
        }
        for (r, mapping) in list(&rails["rails"]).iter().zip(list(&pair["railWireMap"])) {
            assert_eq!(r["id"], mapping["railId"]);
            let wire = list(&wires["wires"])
                .iter()
                .find(|w| w["id"] == mapping["wireId"])
                .unwrap();
            assert_eq!(wire["points"], serde_json::json!([r["start"], r["end"]]));
        }
        let request = wire_request(pair);
        assert!(request.geometry.validate_for(&request.schematic).is_empty());
        let netlist = derive_netlist(&request.schematic, &request.geometry).unwrap();
        assert_eq!(
            partitions(&netlist.nodes),
            expected_partitions(pair),
            "{}",
            pair["id"]
        );
        if let Some(count) = pair["expectedIslandCount"].as_u64() {
            assert_eq!(netlist.islands.len() as u64, count);
        }
    }
}

#[test]
fn handwritten_topology_oracles_match_explicit_wire_baselines_and_permutations() {
    let data = data("stage-8-connectivity.json");
    let cases = list(&data["cases"]);
    assert_eq!(cases.len(), 27);
    for case in cases {
        let errors = list(&case["errorCodes"]);
        // Name/schema/rail-extent validation belongs to #130, not a wire conversion.
        if errors
            .iter()
            .any(|e| !["FloatingNode", "DetachedReferenceMarker"].contains(&text(e)))
        {
            continue;
        }
        let mut request = wire_request(case);
        for _ in 0..2 {
            let derived = derive_netlist(&request.schematic, &request.geometry);
            if errors.is_empty() {
                assert_eq!(
                    partitions(&derived.unwrap().nodes),
                    expected_partitions(case),
                    "{}",
                    case["id"]
                );
            } else if case["legacyWireLimitation"]["issue"] == 137 {
                // #137 is recorded explicitly, not used to change the independent oracle.
                // #131 must correct conductor validation and remove this baseline limitation.
                let netlist = derived.unwrap();
                assert!(request.schematic.components.is_empty());
                assert_eq!(netlist.nodes.len(), 1);
                assert!(netlist.nodes[0].pins.is_empty());
                assert!(!request.schematic.reference_markers.is_empty());
                assert_eq!(errors, &[serde_json::json!("FloatingNode")]);
            } else {
                let codes = derived
                    .unwrap_err()
                    .into_iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Error)
                    .map(|d| format!("{:?}", d.code))
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    codes,
                    errors.iter().map(|v| text(v).to_owned()).collect(),
                    "{}",
                    case["id"]
                );
            }
            request.schematic.components.reverse();
            request.schematic.wires.reverse();
            request.geometry.components.reverse();
        }
    }
}

fn observe(pair: &Value, row: &Value, layout: &ResultLayout, snapshot: &ResultSnapshot) {
    for value in list(&row["values"]) {
        let slot = layout
            .slots
            .iter()
            .find(|s| s.key == text(&value["key"]))
            .unwrap();
        assert_eq!(serde_json::to_value(slot.unit).unwrap(), value["unit"]);
        close(
            snapshot.values[slot.offset as usize],
            number(&value["expected"]),
            number(&value["toleranceAbs"]),
            number(&value["toleranceRel"]),
        );
    }
    for expected in list(&row["states"]) {
        let actual = snapshot
            .states
            .iter()
            .find(|s| s.component_id().as_str() == text(&expected["componentId"]))
            .unwrap();
        let actual = serde_json::to_value(actual).unwrap();
        for (key, value) in expected.as_object().unwrap() {
            assert_eq!(
                &actual[key], value,
                "{} step {}: {key}",
                pair["id"], row["step"]
            );
        }
    }
    if text(&pair["id"]) == "divider" {
        let read = |id: &str, quantity: &str| {
            let key = format!("{id}:{quantity}");
            let slot = layout.slots.iter().find(|s| s.key == key).unwrap();
            snapshot.values[slot.offset as usize]
        };
        let ids = list(&pair["wireSchematic"]["components"])
            .iter()
            .map(|c| text(&c["id"]))
            .collect::<Vec<_>>();
        let i = read(ids[1], "current");
        close(read(ids[0], "current") + i, 0.0, 1e-9, 0.0);
        close(read(ids[2], "current") - i, 0.0, 1e-9, 0.0);
        let power = ids
            .iter()
            .map(|id| read(id, "voltage") * read(id, "current"))
            .sum::<f64>();
        close(power, 0.0, 1e-8, 0.0);
    }
}

fn replay_wire(pair: &Value) {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        diagnostics,
        ..
    } = engine.compile(wire_request(pair))
    else {
        panic!("wire baseline {} should compile", pair["id"]);
    };
    let mut warnings = diagnostics
        .iter()
        .map(|d| format!("{:?}", d.code))
        .collect::<Vec<_>>();
    warnings.sort();
    let mut expected = list(&pair["expectedWarningCodes"])
        .iter()
        .map(|v| text(v).to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(warnings, expected);
    let mut snapshot = initial_snapshot;
    let last = list(&pair["observations"])
        .iter()
        .map(|row| row["step"].as_u64().unwrap())
        .max()
        .unwrap();
    for step in 0..=last {
        if step > 0 {
            snapshot = engine
                .advance(AdvanceRequest {
                    run_id: run_id.clone(),
                    step_count: 1,
                })
                .unwrap();
        }
        let rows = list(&pair["observations"])
            .iter()
            .filter(|r| r["step"] == step)
            .collect::<Vec<_>>();
        for row in rows
            .iter()
            .filter(|r| r["afterEvent"].is_null() && r["afterEventsAtStep"] != true)
        {
            observe(pair, row, &layout, &snapshot);
        }
        for (index, event) in list(&pair["events"])
            .iter()
            .enumerate()
            .filter(|(_, e)| e["newestStep"] == step)
        {
            let component_id = StableId::parse(text(&event["componentId"])).unwrap();
            let expected_document_revision = snapshot.metadata.document_revision.clone();
            let document_revision = format!("event-{index}-{step}");
            if event["kind"] == "parameter" {
                let SetParameterResult::Success {
                    snapshot: updated, ..
                } = engine.set_parameter(SetParameterRequest {
                    run_id: run_id.clone(),
                    expected_document_revision,
                    document_revision,
                    component_id,
                    parameter_id: serde_json::from_value(event["parameterId"].clone()).unwrap(),
                    value: number(&event["value"]),
                })
                else {
                    panic!("parameter event failed");
                };
                snapshot = updated;
            } else {
                let SetOperatorStateResult::Success {
                    snapshot: updated, ..
                } = engine.set_operator_state(SetOperatorStateRequest {
                    run_id: run_id.clone(),
                    expected_document_revision,
                    document_revision,
                    component_id,
                    position: serde_json::from_value(event["position"].clone()).unwrap(),
                })
                else {
                    panic!("operator event failed");
                };
                snapshot = updated;
            }
        }
        for row in rows
            .iter()
            .filter(|r| !r["afterEvent"].is_null() || r["afterEventsAtStep"] == true)
        {
            observe(pair, row, &layout, &snapshot);
        }
    }
}

#[test]
fn explicit_wire_circuits_meet_independent_physics_and_operator_oracles() {
    for pair in list(&data("stage-8-circuits.json")["circuits"]) {
        replay_wire(pair);
    }
}

#[test]
fn reference_numbers_preserve_independent_equations_and_existing_tolerances() {
    let source = data("stage-6-start-stop-latching.expected.json");
    for row in list(&source["expected"]) {
        let current = match row["step"].as_u64().unwrap() {
            5 | 8 => 5.0 / (100.0 + 0.01 + 0.01 * 0.1 / (0.01 + 0.1)),
            12 => 5.0 / 100.11,
            _ => 0.0,
        };
        close(current, number(&row["coilCurrentAmps"]), 1e-16, 0.0);
    }
    let source = data("stage-6-button-capacitor-hold.expected.json");
    for row in list(&source["expected"]) {
        let step = number(&row["step"]);
        let active = if step < 50.0 {
            step
        } else if step < 100.0 {
            49.0
        } else {
            step - 50.0
        };
        close(
            10.0 * (1.0 - (-active * 0.001 / 0.100001).exp()),
            number(&row["capacitorVolts"]),
            1e-13,
            0.0,
        );
    }
}

#[test]
fn diagnostic_reference_oracles_are_canonical_and_resolve_to_drawn_objects() {
    for case in list(&data("stage-8-connectivity.json")["cases"]) {
        let ids = list(&case["errorReferences"])
            .iter()
            .map(|reference| {
                let field = if reference["kind"] == "rail" {
                    "railId"
                } else {
                    "referenceMarkerId"
                };
                let id = text(&reference[field]);
                StableId::parse(id).unwrap();
                let objects = if field == "railId" {
                    "rails"
                } else {
                    "referenceMarkers"
                };
                assert!(
                    list(&case["railSchematic"][objects])
                        .iter()
                        .any(|o| o["id"] == id)
                );
                id
            })
            .collect::<Vec<_>>();
        assert!(
            ids.windows(2).all(|pair| pair[0] < pair[1]),
            "{}: canonical reference order",
            case["id"]
        );
    }
}
