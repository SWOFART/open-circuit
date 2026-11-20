use std::collections::BTreeMap;

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, Engine, FuseState, GridPoint,
    OperatorPosition, RelayContactState, SetOperatorStateRequest, SetOperatorStateResult, StableId,
};
use serde::Deserialize;

const STARTERS: &str = include_str!("../../../tests/golden/stage-10/starters.json");
const ANALYTICAL: &str = include_str!("../../../tests/golden/stage-10/analytical.json");
const NGSPICE: &str = include_str!("../../../tests/golden/stage-10/ngspice.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Operation {
    step: u32,
    component_id: StableId,
    position: OperatorPosition,
}

#[derive(Deserialize)]
struct Starter {
    request: CompileRequest,
    motor: StableId,
    source: StableId,
    states: BTreeMap<String, StableId>,
    operations: Vec<Operation>,
}

#[test]
fn direct_on_line_matches_both_oracles() {
    check_starter("direct-on-line");
}

#[test]
fn swapping_direct_on_line_phases_reverses_the_motor() {
    check_starter("direct-on-line-swapped");
}

#[test]
fn star_delta_matches_both_oracles_and_preserves_the_interlocks() {
    check_starter("star-delta");
}

#[test]
fn swapping_star_delta_phases_reverses_the_motor() {
    check_starter("star-delta-swapped");
}

#[test]
fn a_direct_on_line_overload_opens_the_main_coil_circuit() {
    let mut starters: BTreeMap<String, Starter> = serde_json::from_str(STARTERS).unwrap();
    let mut fixture = starters.remove("direct-on-line").unwrap();
    let overload = &fixture.states["overload"];
    let component = fixture
        .request
        .schematic
        .components
        .iter_mut()
        .find(|component| component.id() == overload)
        .unwrap();
    let open_circuit_engine::ComponentSpec::ThermalOverloadThreePole { parameters, .. } = component
    else {
        unreachable!("overload ID identifies a thermal relay");
    };
    parameters.rated_current_amps = 2.0;
    parameters.trip_scale_seconds = 0.01;
    let mut engine = Engine::default();
    let CompileResult::Success {
        mut initial_snapshot,
        ..
    } = engine.compile(compact_wiring(fixture.request))
    else {
        panic!("overload starter must compile");
    };
    let run_id = initial_snapshot.metadata.run_id.clone();
    let mut picked = false;
    let mut trip_step = None;
    for step in 1..=80 {
        initial_snapshot = engine
            .advance(AdvanceRequest {
                run_id: run_id.clone(),
                step_count: 1,
            })
            .unwrap();
        if step == fixture.operations[0].step {
            let operation = &fixture.operations[0];
            let SetOperatorStateResult::Success { snapshot, .. } =
                engine.set_operator_state(SetOperatorStateRequest {
                    run_id: run_id.clone(),
                    expected_document_revision: initial_snapshot.metadata.document_revision.clone(),
                    document_revision: "start-requested".to_owned(),
                    component_id: operation.component_id.clone(),
                    position: operation.position,
                })
            else {
                panic!("close the main control switch");
            };
            initial_snapshot = snapshot;
        }
        let main_is_picked = initial_snapshot.states.iter().any(|state| {
            matches!(state,
                DeviceState::Contactor { component_id, state: RelayContactState::Picked, .. }
                    if component_id == &fixture.states["main"]
            )
        });
        let tripped = initial_snapshot.states.iter().any(|state| matches!(state,
            DeviceState::Fuse { component_id, state: FuseState::Tripped, .. } if component_id == overload
        ));
        if tripped {
            trip_step.get_or_insert(step);
        }
        if picked && !main_is_picked {
            assert!(trip_step.is_some_and(|trip| trip < step));
            return;
        }
        picked |= main_is_picked;
    }
    panic!("main contactor did not drop when overload opened the coil path");
}

fn check_starter(name: &str) {
    let mut starters: BTreeMap<String, Starter> = serde_json::from_str(STARTERS).unwrap();
    let fixture = starters.remove(name).unwrap();
    let analytical = read_starter_rows(ANALYTICAL);
    let ngspice = read_starter_rows(NGSPICE);
    let compact = compact_wiring(fixture.request.clone());
    let CompileResult::Success {
        initial_snapshot: drawn,
        ..
    } = Engine::default().compile(fixture.request)
    else {
        panic!("{name}: the drawn network must compile");
    };
    let mut engine = Engine::default();
    let result = engine.compile(compact);
    let CompileResult::Success {
        layout,
        mut initial_snapshot,
        ..
    } = result
    else {
        panic!("{name}: {result:?}");
    };
    assert_eq!(
        drawn.values, initial_snapshot.values,
        "{name}: compact topology"
    );
    assert_eq!(
        drawn.states, initial_snapshot.states,
        "{name}: compact topology"
    );
    let run_id = initial_snapshot.metadata.run_id.clone();
    let keys = [
        (&fixture.motor, "u/current"),
        (&fixture.motor, "v/current"),
        (&fixture.motor, "w/current"),
        (&fixture.motor, "angular_velocity"),
        (&fixture.motor, "electromagnetic_torque"),
        (&fixture.source, "l1/current"),
        (&fixture.source, "l2/current"),
        (&fixture.source, "l3/current"),
    ];
    let offsets = keys.map(|(id, key)| {
        let key = format!("{}/{key}", id.as_str());
        layout
            .slots
            .iter()
            .find(|slot| slot.key == key)
            .unwrap()
            .offset as usize
    });
    let mut operations = fixture.operations.iter().enumerate().peekable();
    let mut references = analytical[name].iter().zip(&ngspice[name]).peekable();
    let mut saw_star = false;
    let mut saw_gap = false;
    let mut saw_delta = false;
    let mut final_speed = 0.0;
    let mut source_squares = [[[0.0; 3]; 3]; 2];
    for step in 0..=243 {
        while let Some((i, operation)) = operations.next_if(|(_, operation)| operation.step == step)
        {
            let result = engine.set_operator_state(SetOperatorStateRequest {
                run_id: run_id.clone(),
                expected_document_revision: initial_snapshot.metadata.document_revision.clone(),
                document_revision: format!("operator-{i}"),
                component_id: operation.component_id.clone(),
                position: operation.position,
            });
            let SetOperatorStateResult::Success { snapshot, .. } = result else {
                panic!("{name} at step {step}: {result:?}");
            };
            initial_snapshot = snapshot;
        }
        let is_picked = |name: &str| {
            fixture.states.get(name).is_some_and(|id| {
                    initial_snapshot.states.iter().any(|state| matches!(state,
                        DeviceState::Contactor { component_id, state: RelayContactState::Picked, .. }
                            if component_id == id
                    ))
                })
        };
        let (main, star, delta) = (is_picked("main"), is_picked("star"), is_picked("delta"));
        assert!(
            !(star && delta),
            "{name}: star and delta overlap at step {step}"
        );
        if fixture.states.contains_key("star") {
            if (25..=110).contains(&step) {
                assert!(main && star && !delta, "{name} at step {step}");
            }
            if (125..=140).contains(&step) {
                assert!(main && !star && !delta, "{name} at step {step}");
            }
            if step >= 145 {
                assert!(main && !star && delta, "{name} at step {step}");
            }
            saw_star |= star;
            saw_gap |= saw_star && !star && !delta;
            saw_delta |= delta;
        }
        assert!(
            initial_snapshot.states.iter().any(|state| matches!(state,
                DeviceState::Fuse { component_id, state: FuseState::Intact, .. }
                    if component_id == &fixture.states["overload"]
            )),
            "{name}: unexpected overload trip at {step}"
        );

        if let Some((first, second)) =
            references.next_if(|(row, _)| (row[0] * 1000.0).round() == f64::from(step))
        {
            assert_eq!(first[0], second[0], "{name}");
            let source_total = offsets[5..]
                .iter()
                .map(|index| initial_snapshot.values[*index])
                .sum::<f64>();
            assert!(
                source_total.abs() < 1e-8,
                "{name} step {step}: Engine source KCL"
            );
            for reference in [first, second] {
                assert!(
                    reference[6..].iter().sum::<f64>().abs() < 1e-8,
                    "{name}: reference source KCL"
                );
            }
            for (phase, offset) in offsets[5..].iter().enumerate() {
                let actual = initial_snapshot.values[*offset];
                if !main || (fixture.states.contains_key("star") && !star && !delta) {
                    for value in [actual, first[6 + phase], second[6 + phase]] {
                        assert!(value.abs() < 1e-8, "{name} step {step}: open source phase");
                    }
                    continue;
                }
                let mode = usize::from(delta);
                source_squares[mode][0][phase] += actual * actual;
                for (oracle, row) in [first, second].into_iter().enumerate() {
                    let expected = row[6 + phase];
                    source_squares[mode][1 + oracle][phase] += expected * expected;
                    let branch = if name.ends_with("-swapped") {
                        [0, 2, 1][phase]
                    } else {
                        phase
                    };
                    let winding_error = |index: usize| 0.12_f64.max(0.02 * row[1 + index].abs());
                    // A delta line current is the difference of two branch
                    // currents. Their pointwise errors can add near a zero
                    // crossing. The sampled RMS check below still uses 2%.
                    let propagated_error = if delta {
                        winding_error(branch) + winding_error((branch + 2) % 3)
                    } else {
                        winding_error(branch)
                    };
                    assert!(
                        (actual - expected).abs() <= propagated_error,
                        "{name} step {step} source phase {}: actual {actual}, expected {expected}",
                        phase + 1
                    );
                }
            }
            for i in 0..5 {
                let actual = initial_snapshot.values[offsets[i]];
                let (absolute, relative) = match i {
                    3 => (0.5, 0.02),
                    4 => (0.05, 0.03),
                    _ => (0.12_f64, 0.02_f64),
                };
                for expected in [first[i + 1], second[i + 1]] {
                    assert!(
                        (actual - expected).abs() <= absolute.max(relative * expected.abs()),
                        "{name} step {step} {}: actual {actual}, expected {expected}",
                        keys[i].1
                    );
                }
            }
        }
        final_speed = initial_snapshot.values[offsets[3]];
        if step < 243 {
            initial_snapshot = engine
                .advance(AdvanceRequest {
                    run_id: run_id.clone(),
                    step_count: 1,
                })
                .unwrap_or_else(|diagnostics| panic!("{name} at step {step}: {diagnostics:?}"));
        }
    }
    assert!(operations.next().is_none() && references.next().is_none());
    if fixture.states.contains_key("star") {
        assert!(saw_star && saw_gap && saw_delta, "{name}");
    }
    for [actual, analytical, ngspice] in source_squares {
        for i in 0..3 {
            for expected in [analytical[i], ngspice[i]] {
                assert!(
                    (actual[i].sqrt() - expected.sqrt()).abs() <= 0.02 * expected.sqrt(),
                    "{name}: source phase {} sample RMS differs by more than 2%",
                    i + 1
                );
            }
        }
    }
    if name.ends_with("-swapped") {
        assert!(final_speed < -100.0, "{name}: wrong phase sequence");
    } else {
        assert!(final_speed > 100.0, "{name}: correct phase sequence");
    }
}

fn compact_wiring(mut request: CompileRequest) -> CompileRequest {
    use open_circuit_engine::netlist::{ReferenceKind, derive_netlist};
    // Resolve the drawn network once, then colocate exactly the pins on
    // each resolved node so internal substeps need not intersect its wires.
    let layout = derive_netlist(&request.schematic, &request.geometry).unwrap();
    let grid = |index: usize| GridPoint {
        x: i32::try_from(index).unwrap(),
        y: 0,
    };
    let root = layout
        .islands
        .iter()
        .find(|island| island.reference_kind == Some(ReferenceKind::ExplicitMarker))
        .and_then(|island| island.reference_node)
        .unwrap();
    let pin_positions: BTreeMap<_, _> = layout
        .nodes
        .iter()
        .flat_map(|node| {
            node.pins.iter().map(move |pin| {
                (
                    (pin.component_id.as_str(), pin.pin_id.as_str()),
                    grid(node.index),
                )
            })
        })
        .collect();
    for component in &mut request.geometry.components {
        for pin in &mut component.pins {
            pin.position = pin_positions[&(component.component_id.as_str(), pin.pin_id.as_str())];
        }
    }
    request.schematic.wires.clear();
    request.schematic.reference_markers[0].position = grid(root);
    request
}

fn read_starter_rows(text: &str) -> BTreeMap<String, Vec<[f64; 9]>> {
    let values: BTreeMap<String, serde_json::Value> = serde_json::from_str(text).unwrap();
    values
        .into_iter()
        .filter(|(id, _)| id.starts_with("direct-on-line") || id.starts_with("star-delta"))
        .map(|(id, rows)| (id, serde_json::from_value(rows).unwrap()))
        .collect()
}
