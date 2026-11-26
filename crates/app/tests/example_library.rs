//! Run the actual bundled drawings with the geometry from the corresponding golden cases.

use open_circuit_app_lib::examples::{list_examples, read_example};
use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, Engine, OperatorPosition,
    RelayContactState, ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult, StableId,
};

const FIXTURES: &[(&str, &str)] = &[
    (
        "rc",
        include_str!("../../../tests/golden/stage-3-drawn-rc.request.json"),
    ),
    (
        "relay-latch",
        include_str!("../../../tests/golden/stage-4-drawn-relay.request.json"),
    ),
    (
        "start-stop",
        include_str!("../../../tests/golden/stage-6-drawn-latching.request.json"),
    ),
    (
        "rail-circuit",
        include_str!("../../../tests/golden/stage-8-drawn-divider.request.json"),
    ),
    (
        "three-phase-starter",
        include_str!("../../../tests/golden/stage-14/timed-star-delta.request.json"),
    ),
];

fn component(suffix: u8) -> StableId {
    StableId::parse(format!("00000000-0000-4000-8000-{suffix:012}")).unwrap()
}

fn advance(engine: &mut Engine, snapshot: &mut ResultSnapshot, steps: u32) {
    *snapshot = engine
        .advance(AdvanceRequest {
            run_id: snapshot.metadata.run_id.clone(),
            step_count: steps,
        })
        .unwrap();
}

fn operate(
    engine: &mut Engine,
    snapshot: &mut ResultSnapshot,
    suffix: u8,
    position: OperatorPosition,
) {
    let result = engine.set_operator_state(SetOperatorStateRequest {
        run_id: snapshot.metadata.run_id.clone(),
        expected_document_revision: snapshot.metadata.document_revision.clone(),
        document_revision: format!("operator-{}", snapshot.metadata.frame_sequence),
        component_id: component(suffix),
        position,
    });
    let SetOperatorStateResult::Success { snapshot: next, .. } = result else {
        panic!(
            "operator command {suffix} {position:?} at step {}: {result:?}",
            snapshot.metadata.simulation_step
        );
    };
    *snapshot = next;
}

fn picked(snapshot: &ResultSnapshot, suffix: u8) -> bool {
    snapshot.states.iter().any(|state| {
        matches!(state,
            DeviceState::Relay { component_id, state: RelayContactState::Picked, .. }
            | DeviceState::Contactor { component_id, state: RelayContactState::Picked, .. }
            if component_id == &component(suffix)
        )
    })
}

#[test]
fn every_bundled_example_opens_and_reaches_a_known_result() {
    let entries = list_examples().unwrap();
    assert_eq!(entries.len(), 1 + FIXTURES.len());
    for [id, _, _] in entries {
        let schematic = read_example(&id, false).unwrap();
        let request = if id == "divider" {
            assert_eq!(
                schematic,
                serde_json::from_str(include_str!(
                    "../../../tests/golden/stage-1-divider.schematic.json"
                ))
                .unwrap()
            );
            CompileRequest {
                schematic,
                geometry: serde_json::from_str(include_str!(
                    "../../../tests/golden/stage-1-divider.geometry.json"
                ))
                .unwrap(),
                document_revision: "rev-1".into(),
                faults: None,
            }
        } else {
            let (_, json) = FIXTURES.iter().find(|(key, _)| *key == id).unwrap();
            let request: CompileRequest = serde_json::from_str(json).unwrap();
            assert_eq!(schematic, request.schematic, "{id}");
            CompileRequest {
                schematic,
                ..request
            }
        };
        let mut engine = Engine::default();
        let result = engine.compile(request);
        let CompileResult::Success {
            layout,
            initial_snapshot: mut snapshot,
            ..
        } = result
        else {
            panic!("{id}: {result:?}")
        };
        let read_voltage = |snapshot: &ResultSnapshot, suffix| {
            let key = format!("{}:voltage", component(suffix).as_str());
            let offset = layout
                .slots
                .iter()
                .find(|slot| slot.key == key)
                .unwrap()
                .offset;
            snapshot.values[offset as usize]
        };
        advance(&mut engine, &mut snapshot, 5);
        match id.as_str() {
            "divider" | "rail-circuit" => {
                assert!((read_voltage(&snapshot, 2) - 5.0 * 1000.0 / 2000.1).abs() < 1e-9);
            }
            "rc" => {
                let expected = 10.0 * (1.0 - (-0.5_f64).exp());
                assert!((read_voltage(&snapshot, 3).abs() - expected).abs() < 0.02);
            }
            "relay-latch" => assert!(snapshot.states.iter().any(|state| matches!(
                state,
                DeviceState::Relay {
                    state: RelayContactState::Picked,
                    ..
                }
            ))),
            "start-stop" => {
                assert!(!picked(&snapshot, 4));
                operate(&mut engine, &mut snapshot, 3, OperatorPosition::Pressed);
                advance(&mut engine, &mut snapshot, 5);
                operate(&mut engine, &mut snapshot, 3, OperatorPosition::Released);
                assert!(picked(&snapshot, 4));
                advance(&mut engine, &mut snapshot, 5);
                operate(&mut engine, &mut snapshot, 2, OperatorPosition::Pressed);
                assert!(!picked(&snapshot, 4));
            }
            "three-phase-starter" => {
                advance(&mut engine, &mut snapshot, 15);
                operate(&mut engine, &mut snapshot, 4, OperatorPosition::Closed);
                advance(&mut engine, &mut snapshot, 60);
                assert!(picked(&snapshot, 9) && !picked(&snapshot, 11));
                advance(&mut engine, &mut snapshot, 100);
                assert!(!picked(&snapshot, 9) && picked(&snapshot, 11));
            }
            _ => panic!("missing known result for {id}"),
        }
    }
}
