//! Shared by the Stage 10 session and acceptance tests: the starter fixtures of
//! `tests/golden/stage-10`, a run through the application core with their operator events, the same
//! events sent straight to an Engine, and the independent oracles.

use std::collections::BTreeMap;

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, Engine, OperatorPosition, ResultLayout,
    ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult, StableId,
};
use serde::Deserialize;
use serde_json::Value;

use super::Run;

pub const STARTERS: &str = include_str!("../../../../tests/golden/stage-10/starters.json");
pub const ANALYTICAL: &str = include_str!("../../../../tests/golden/stage-10/analytical.json");
pub const NGSPICE: &str = include_str!("../../../../tests/golden/stage-10/ngspice.json");
pub const LAST_STEP: u32 = 243;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub step: u32,
    pub component_id: StableId,
    pub position: OperatorPosition,
}

#[derive(Deserialize)]
pub struct Starter {
    pub request: CompileRequest,
    pub motor: StableId,
    pub states: BTreeMap<String, StableId>,
    pub operations: Vec<Operation>,
}

pub fn starters() -> BTreeMap<String, Starter> {
    let mut all: BTreeMap<String, Starter> = serde_json::from_str(STARTERS).unwrap();
    for starter in all.values_mut() {
        // The application's first document revision, as every session test names it.
        starter.request.document_revision = "rev-1".to_owned();
    }
    all
}

pub fn starter(name: &str) -> Starter {
    starters().remove(name).unwrap()
}

pub fn rows(text: &str, name: &str) -> Vec<[f64; 9]> {
    let all: BTreeMap<String, Value> = serde_json::from_str(text).unwrap();
    serde_json::from_value(all[name].clone()).unwrap()
}

pub fn offset_of(layout: &ResultLayout, component: &StableId, key: &str) -> usize {
    let key = format!("{}/{key}", component.as_str());
    layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot `{key}`"))
        .offset as usize
}

pub fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, id: &str, key: &str) -> f64 {
    let id = StableId::parse(id).unwrap();
    snapshot.values[offset_of(layout, &id, key)]
}

/// A starter run through the application core, the operator events applied at their steps as the
/// application sends them. One snapshot per public step, the one the application would show.
pub fn bridged(starter: &Starter) -> (ResultLayout, Vec<ResultSnapshot>, Run) {
    let mut run = Run::start_every_step(starter.request.clone());
    let layout = run.layout.clone();
    let mut snapshot = run.initial_snapshot.clone();
    let mut snapshots = Vec::new();

    for step in 0..=LAST_STEP {
        if step > 0 {
            assert_eq!(run.advance(1), u64::from(step));
            snapshot = run.delivered.last().unwrap().0.clone();
            assert_eq!(snapshot.metadata.simulation_step, u64::from(step));
        }
        for operation in starter.operations.iter().filter(|op| op.step == step) {
            let (_, result) =
                run.set_operator_state(operation.component_id.as_str(), operation.position);
            let SetOperatorStateResult::Success {
                snapshot: after, ..
            } = result
            else {
                panic!("operator event at step {step} refused: {result:?}");
            };
            snapshot = after;
        }
        snapshots.push(snapshot.clone());
    }

    (layout, snapshots, run)
}

/// The same events sent straight to an Engine with no application in between.
pub fn direct(starter: &Starter) -> Vec<ResultSnapshot> {
    let mut engine = Engine::default();
    let CompileResult::Success {
        run_id,
        mut initial_snapshot,
        ..
    } = engine.compile(starter.request.clone())
    else {
        panic!("the starter compiles");
    };
    let mut snapshots = Vec::new();
    let mut counter = 1;

    for step in 0..=LAST_STEP {
        if step > 0 {
            initial_snapshot = engine
                .advance(AdvanceRequest {
                    run_id: run_id.clone(),
                    step_count: 1,
                })
                .unwrap();
        }
        for operation in starter.operations.iter().filter(|op| op.step == step) {
            counter += 1;
            let result = engine.set_operator_state(SetOperatorStateRequest {
                run_id: run_id.clone(),
                expected_document_revision: initial_snapshot.metadata.document_revision.clone(),
                document_revision: format!("rev-{counter}"),
                component_id: operation.component_id.clone(),
                position: operation.position,
            });
            let SetOperatorStateResult::Success { snapshot, .. } = result else {
                panic!("operator event refused: {result:?}");
            };
            initial_snapshot = snapshot;
        }
        snapshots.push(initial_snapshot.clone());
    }

    snapshots
}

pub fn bits(snapshots: &[ResultSnapshot]) -> Vec<Vec<u64>> {
    snapshots
        .iter()
        .map(|snapshot| {
            snapshot
                .values
                .iter()
                .map(|value| value.to_bits())
                .collect()
        })
        .collect()
}

/// Holds a run of the starter `name` to the independent answers: the analytical and ngspice rows,
/// with the tolerances the Engine itself is held to (stage-10 contract section 8).
pub fn meet_the_oracles(
    name: &str,
    motor: &StableId,
    layout: &ResultLayout,
    snapshots: &[ResultSnapshot],
) {
    let keys = [
        "u/current",
        "v/current",
        "w/current",
        "angular_velocity",
        "electromagnetic_torque",
    ];
    let offsets = keys.map(|key| offset_of(layout, motor, key));
    for (oracle, text) in [("analytical", ANALYTICAL), ("ngspice", NGSPICE)] {
        let mut checked = 0;
        for row in rows(text, name) {
            let step = (row[0] * 1000.0).round() as usize;
            let snapshot = &snapshots[step];
            for i in 0..5 {
                let actual = snapshot.values[offsets[i]];
                let (absolute, relative): (f64, f64) = match i {
                    3 => (0.5, 0.02),
                    4 => (0.05, 0.03),
                    _ => (0.12, 0.02),
                };
                let expected = row[i + 1];
                assert!(
                    (actual - expected).abs() <= absolute.max(relative * expected.abs()),
                    "{name} {oracle} step {step} {}: {actual} against {expected}",
                    keys[i]
                );
            }
            checked += 1;
        }
        assert!(checked > 20, "{name}: {oracle} rows were compared");
    }
}
