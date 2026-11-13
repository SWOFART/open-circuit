//! Stage 8 independent data (#129), executed against both conductors by #131.

use std::collections::BTreeSet;

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticSeverity, Engine, ResultLayout,
    ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest,
    SetParameterResult, StableId, TraceBatch, TraceQuery, TraceRequest, decode_project_json,
    netlist::derive_netlist,
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
    drawing_request(pair, "wireSchematic", false)
}

fn drawing_request(pair: &Value, key: &str, permuted: bool) -> CompileRequest {
    let mut request = CompileRequest {
        schematic: if key == "wireSchematic" {
            decode_project_json(&serde_json::to_vec(&pair[key]).unwrap()).unwrap()
        } else {
            serde_json::from_value(pair[key].clone()).unwrap()
        },
        geometry: serde_json::from_value(pair["geometry"].clone()).unwrap(),
        document_revision: format!("reference-{}", text(&pair["id"])),
        faults: None,
    };
    if permuted {
        request.schematic.components.reverse();
        request.schematic.wires.reverse();
        request.schematic.rails.reverse();
        request.schematic.junctions.reverse();
        request.schematic.reference_markers.reverse();
        request.geometry.components.reverse();
        for component in &mut request.geometry.components {
            component.pins.reverse();
        }
    }
    request
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

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct IslandMeaning {
    pin_groups: BTreeSet<BTreeSet<String>>,
    component_ids: BTreeSet<String>,
    reference_pins: Option<BTreeSet<String>>,
    reference_kind: Option<&'static str>,
}

fn mapped_islands(netlist: &open_circuit_engine::netlist::Netlist) -> BTreeSet<IslandMeaning> {
    use open_circuit_engine::netlist::ReferenceKind;
    let pins = |node: usize| {
        netlist.nodes[node]
            .pins
            .iter()
            .map(|p| format!("{}/{}", p.component_id, p.pin_id.as_str()))
            .collect::<BTreeSet<_>>()
    };
    netlist
        .islands
        .iter()
        .map(|island| IslandMeaning {
            pin_groups: island.node_indices.iter().map(|node| pins(*node)).collect(),
            component_ids: island
                .component_ids
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect(),
            reference_pins: island.reference_node.map(pins),
            reference_kind: island.reference_kind.map(|kind| match kind {
                ReferenceKind::ExplicitMarker => "explicitMarker",
                ReferenceKind::Automatic => "automatic",
            }),
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
        let rail_request = drawing_request(pair, "railSchematic", false);
        let rail_netlist = derive_netlist(&rail_request.schematic, &rail_request.geometry).unwrap();
        assert_eq!(partitions(&rail_netlist.nodes), expected_partitions(pair));
        assert_eq!(mapped_islands(&rail_netlist), mapped_islands(&netlist));
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

#[test]
fn all_rail_truth_table_cases_match_exact_partitions_references_and_permutations() {
    for case in list(&data("stage-8-connectivity.json")["cases"]) {
        let mut previous = None;
        for permuted in [false, true, false] {
            let request = drawing_request(case, "railSchematic", permuted);
            let actual = derive_netlist(&request.schematic, &request.geometry);
            if list(&case["errorCodes"]).is_empty() {
                let netlist = actual.as_ref().unwrap();
                assert_eq!(
                    partitions(&netlist.nodes),
                    expected_partitions(case),
                    "{}",
                    case["id"]
                );
                let wire = wire_request(case);
                let wire = derive_netlist(&wire.schematic, &wire.geometry).unwrap();
                assert_eq!(mapped_islands(netlist), mapped_islands(&wire));
                for mapping in list(&case["railWireMap"]) {
                    let rail_node = netlist
                        .nodes
                        .iter()
                        .find(|n| {
                            n.rail_ids
                                .iter()
                                .any(|id| id.as_str() == text(&mapping["railId"]))
                        })
                        .unwrap();
                    let wire_node = wire
                        .nodes
                        .iter()
                        .find(|n| {
                            n.wire_ids
                                .iter()
                                .any(|id| id.as_str() == text(&mapping["wireId"]))
                        })
                        .unwrap();
                    assert_eq!(rail_node.pins, wire_node.pins);
                }
            } else {
                let errors = actual
                    .as_ref()
                    .unwrap_err()
                    .iter()
                    .filter(|d| d.severity == DiagnosticSeverity::Error)
                    .collect::<Vec<_>>();
                assert_eq!(
                    errors
                        .iter()
                        .map(|d| format!("{:?}", d.code))
                        .collect::<BTreeSet<_>>(),
                    list(&case["errorCodes"])
                        .iter()
                        .map(|v| text(v).to_owned())
                        .collect(),
                    "{}",
                    case["id"]
                );
                let references = errors
                    .iter()
                    .flat_map(|d| d.references.iter())
                    .map(|r| serde_json::to_value(r).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(references, list(&case["errorReferences"]), "{}", case["id"]);
            }
            if let Some(previous) = &previous {
                assert_eq!(&actual, previous, "{}", case["id"]);
            }
            previous = Some(actual);
        }
    }
}

#[test]
fn rail_contacts_are_invariant_under_grid_translation_even_near_i32_limits() {
    fn translate(value: &mut Value, dx: i64, dy: i64) {
        match value {
            Value::Object(fields) if fields.contains_key("x") && fields.contains_key("y") => {
                for (key, offset) in [("x", dx), ("y", dy)] {
                    let coordinate = i32::try_from(fields[key].as_i64().unwrap() + offset).unwrap();
                    fields.insert(key.into(), coordinate.into());
                }
            }
            Value::Object(fields) => fields.values_mut().for_each(|v| translate(v, dx, dy)),
            Value::Array(values) => values.iter_mut().for_each(|v| translate(v, dx, dy)),
            _ => {}
        }
    }
    for case in list(&data("stage-8-connectivity.json")["cases"])
        .iter()
        .filter(|c| list(&c["errorCodes"]).is_empty())
    {
        for (dx, dy) in [
            (-17, 29),
            (i64::from(i32::MAX) - 100, i64::from(i32::MIN) + 100),
            (i64::from(i32::MIN) + 100, i64::from(i32::MAX) - 100),
        ] {
            let mut moved = case.clone();
            translate(&mut moved["railSchematic"], dx, dy);
            translate(&mut moved["geometry"], dx, dy);
            let request = drawing_request(&moved, "railSchematic", false);
            let netlist = derive_netlist(&request.schematic, &request.geometry).unwrap();
            assert_eq!(
                partitions(&netlist.nodes),
                expected_partitions(case),
                "{} at {dx},{dy}",
                case["id"]
            );
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

struct Replay {
    layout: ResultLayout,
    snapshots: Vec<ResultSnapshot>,
    traces: TraceBatch,
}

fn replay(pair: &Value, key: &str, permuted: bool) -> Replay {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        diagnostics,
        ..
    } = engine.compile(drawing_request(pair, key, permuted))
    else {
        panic!("{key} {} should compile", pair["id"]);
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
    let mut snapshots = Vec::new();
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
        snapshots.push(snapshot.clone());
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
            snapshots.push(snapshot.clone());
        }
        for row in rows
            .iter()
            .filter(|r| !r["afterEvent"].is_null() || r["afterEventsAtStep"] == true)
        {
            observe(pair, row, &layout, &snapshot);
        }
    }
    let traces = engine
        .read_traces(TraceRequest {
            run_id,
            layout_version: layout.layout_version,
            signal_keys: layout.slots.iter().take(8).map(|s| s.key.clone()).collect(),
            query: TraceQuery::History {
                start_step: 0,
                end_step: last,
            },
            max_points: 4096,
        })
        .unwrap();
    assert_eq!(traces.points.len() as u64, last + 1);
    assert_eq!(traces.metadata.sample_interval_seconds, 0.001);
    // A live edit replaces the latest trace sample, keeping earlier history intact.
    for point in &traces.points {
        let latest = snapshots
            .iter()
            .rev()
            .find(|s| s.metadata.simulation_step == point.simulation_step)
            .unwrap();
        for (index, signal) in traces.signals.iter().enumerate() {
            let slot = layout.slots.iter().find(|s| s.key == signal.key).unwrap();
            assert_eq!(
                point.minimum_values[index],
                latest.values[slot.offset as usize]
            );
            assert_eq!(
                point.maximum_values[index],
                latest.values[slot.offset as usize]
            );
        }
    }
    Replay {
        layout,
        snapshots,
        traces,
    }
}

#[test]
fn explicit_wire_circuits_meet_independent_physics_and_operator_oracles() {
    for pair in list(&data("stage-8-circuits.json")["circuits"]) {
        replay(pair, "wireSchematic", false);
    }
}

fn equivalent_states(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, value) in a {
                if key == "lastTransitionTimeSeconds" {
                    close(number(value), number(&b[key]), 1e-9, 0.0);
                } else {
                    assert_eq!(value, &b[key], "{key}");
                }
            }
        }
        _ => assert_eq!(a, b),
    }
}

#[test]
fn all_six_rail_runs_match_wire_physics_states_timing_and_traces_repeated_and_permuted() {
    for pair in list(&data("stage-8-circuits.json")["circuits"]) {
        let wire = replay(pair, "wireSchematic", false);
        let mut previous: Option<Replay> = None;
        for permuted in [false, true, false] {
            let rail = replay(pair, "railSchematic", permuted);
            assert_eq!(rail.layout, wire.layout); // No rail slots, values, states or quantities.
            assert_eq!(rail.snapshots.len(), wire.snapshots.len());
            for (a, b) in rail.snapshots.iter().zip(&wire.snapshots) {
                assert_eq!(a.metadata.simulation_step, b.metadata.simulation_step);
                assert_eq!(
                    a.metadata.simulation_time_seconds,
                    b.metadata.simulation_time_seconds
                );
                assert_eq!(a.metadata.document_revision, b.metadata.document_revision);
                assert_eq!(a.metadata.frame_sequence, b.metadata.frame_sequence);
                for (a, b) in a.values.iter().zip(&b.values) {
                    close(*a, *b, 1e-9, 1e-9);
                }
                assert_eq!(a.states.len(), b.states.len());
                for (a, b) in a.states.iter().zip(&b.states) {
                    equivalent_states(
                        &serde_json::to_value(a).unwrap(),
                        &serde_json::to_value(b).unwrap(),
                    );
                }
            }
            assert_eq!(rail.traces.signals, wire.traces.signals);
            for (a, b) in rail.traces.points.iter().zip(&wire.traces.points) {
                assert_eq!(a.simulation_step, b.simulation_step);
                for (a, b) in a
                    .minimum_values
                    .iter()
                    .chain(&a.maximum_values)
                    .zip(b.minimum_values.iter().chain(&b.maximum_values))
                {
                    close(*a, *b, 1e-9, 1e-9);
                }
            }
            if let Some(previous) = &previous {
                assert_eq!(rail.layout, previous.layout);
                assert_eq!(rail.traces.points, previous.traces.points);
                for (a, b) in rail.snapshots.iter().zip(&previous.snapshots) {
                    assert_eq!(a.values, b.values);
                    assert_eq!(a.states, b.states);
                }
            }
            previous = Some(rail);
        }
    }
}

#[test]
fn rail_compile_failures_are_atomic_and_source_errors_keep_drawable_members() {
    use open_circuit_engine::{
        ComponentSpec, DiagnosticCode, DiagnosticReference, GridPoint, WireSpec,
    };
    let fixtures = data("stage-8-circuits.json");
    let divider = &fixtures["circuits"][0];
    let request = drawing_request(divider, "railSchematic", false);
    let mut engine = Engine::default();
    let CompileResult::Success { run_id, layout, .. } = engine.compile(request.clone()) else {
        panic!("divider");
    };
    let before = engine
        .advance(AdvanceRequest {
            run_id: run_id.clone(),
            step_count: 4,
        })
        .unwrap();
    let query = TraceRequest {
        run_id: run_id.clone(),
        layout_version: layout.layout_version,
        signal_keys: vec![layout.slots[0].key.clone()],
        query: TraceQuery::History {
            start_step: 0,
            end_step: 4,
        },
        max_points: 16,
    };
    let history = engine.read_traces(query.clone()).unwrap();
    let mut invalid = request.clone();
    invalid.schematic.rails[0].name = " invalid ".into();
    let mut short = request.clone();
    let source_id = short.schematic.components[0].id().clone();
    if let ComponentSpec::DcVoltageSource { parameters, .. } = &mut short.schematic.components[0] {
        parameters.internal_resistance_ohms = 0.0;
    }
    short.schematic.wires.push(WireSpec {
        id: StableId::parse("00000000-0000-4000-8000-000000009000").unwrap(),
        points: vec![GridPoint { x: -1, y: 0 }, GridPoint { x: 9, y: 0 }],
    });
    let mut conflict = request.clone();
    if let ComponentSpec::DcVoltageSource { parameters, .. } = &mut conflict.schematic.components[0]
    {
        parameters.internal_resistance_ohms = 0.0;
    }
    let mut second = serde_json::to_value(&conflict.schematic.components[0]).unwrap();
    let second_id = StableId::parse("00000000-0000-4000-8000-000000009001").unwrap();
    second["id"] = serde_json::to_value(&second_id).unwrap();
    second["parameters"]["voltageVolts"] = 10.into();
    conflict
        .schematic
        .components
        .push(serde_json::from_value(second).unwrap());
    let mut geometry = conflict.geometry.components[0].clone();
    geometry.component_id = second_id;
    conflict.geometry.components.push(geometry);
    for (failed, code) in [
        (invalid, DiagnosticCode::InvalidRailName),
        (short, DiagnosticCode::ShortCircuit),
        (conflict, DiagnosticCode::InconsistentIdealConstraint),
    ] {
        let CompileResult::Failure { diagnostics } = engine.compile(failed) else {
            panic!("expected {code:?}");
        };
        let diagnostic = diagnostics.iter().find(|d| d.code == code).unwrap();
        if code != DiagnosticCode::InvalidRailName {
            assert!(
                diagnostic
                    .references
                    .iter()
                    .any(|r| matches!(r, DiagnosticReference::Component { .. }))
            );
            for rail in &request.schematic.rails {
                assert!(diagnostic.references.contains(&DiagnosticReference::Rail {
                    rail_id: rail.id.clone()
                }));
            }
            if code == DiagnosticCode::ShortCircuit {
                assert!(
                    diagnostic
                        .references
                        .contains(&DiagnosticReference::Component {
                            component_id: source_id.clone()
                        })
                );
            }
        }
        assert_eq!(engine.latest_snapshot(), Some(&before));
        assert_eq!(engine.read_traces(query.clone()).unwrap(), history);
    }
    // A structural rail rename creates a fresh run, with a new trace history at time zero.
    let mut renamed = request;
    renamed.schematic.rails[0].name = "renamed".into();
    renamed.document_revision = "renamed-rail".into();
    let CompileResult::Success {
        run_id: new_run,
        initial_snapshot,
        layout: new_layout,
        ..
    } = engine.compile(renamed)
    else {
        panic!("rename");
    };
    assert_ne!(new_run, run_id);
    assert_eq!(initial_snapshot.metadata.simulation_step, 0);
    assert_eq!(new_layout, layout);
    assert_eq!(initial_snapshot.values, before.values);
    assert_eq!(initial_snapshot.states, before.states);
    let reset = engine
        .read_traces(TraceRequest {
            run_id: new_run,
            ..query.clone()
        })
        .unwrap();
    assert_eq!(reset.points.len(), 1);
    assert_eq!(reset.points[0].simulation_step, 0);
    assert!(engine.read_traces(query).is_err());
}

#[test]
fn pinless_mixed_conductor_reports_every_member_and_attached_marker() {
    use open_circuit_engine::{DiagnosticCode, DiagnosticReference};
    let fixtures = data("stage-8-connectivity.json");
    let mut case = list(&fixtures["cases"])
        .iter()
        .find(|c| c["id"] == "empty-referenced-rail")
        .unwrap()
        .clone();
    case["railSchematic"]["wires"] = serde_json::json!([{
        "id":"00000000-0000-4000-8000-000000009000", "points":[{"x":0,"y":0},{"x":0,"y":3}]
    }]);
    case["railSchematic"]["junctions"] = serde_json::json!([{
        "id":"00000000-0000-4000-8000-000000009001", "position":{"x":0,"y":1}
    }]);
    let request = drawing_request(&case, "railSchematic", false);
    let errors = derive_netlist(&request.schematic, &request.geometry).unwrap_err();
    let floating = errors
        .iter()
        .find(|d| d.code == DiagnosticCode::FloatingNode)
        .unwrap();
    assert_eq!(floating.references.len(), 4);
    assert!(
        floating
            .references
            .iter()
            .any(|r| matches!(r, DiagnosticReference::Rail { .. }))
    );
    assert!(
        floating
            .references
            .iter()
            .any(|r| matches!(r, DiagnosticReference::Wire { .. }))
    );
    assert!(
        floating
            .references
            .iter()
            .any(|r| matches!(r, DiagnosticReference::Junction { .. }))
    );
    assert!(
        floating
            .references
            .iter()
            .any(|r| matches!(r, DiagnosticReference::ReferenceMarker { .. }))
    );
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
