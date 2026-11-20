//! Stage 10 acceptance (issue #181): the direct-on-line and star-delta starters, each drawn from a
//! blank document with the editor's own commands (`src/store/stage10Drawing.test.ts`, pinned as
//! `tests/golden/stage-10-drawn-*.request.json`), checked with the Engine's own connectivity
//! against the independent fixture, run through the application core against the independent
//! analytical and ngspice answers, and put through saving, reopening and replay. The wrong
//! wirings (two phases swapped) are drawn too and must show their distinct result. Nothing here
//! adds a model: the answers are `tests/golden/stage-10/*`, never Engine output.

mod common;

use std::collections::BTreeSet;

use common::Run;
use common::stage10::{
    LAST_STEP, Starter, bits, bridged, meet_the_oracles, offset_of, starter, starters,
};
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_engine::netlist::derive_netlist;
use open_circuit_engine::{
    CompileRequest, DeviceState, FuseState, OperatorPosition, RelayContactState,
    SetOperatorStateResult,
};

const DRAWN_DIRECT_ON_LINE: &str =
    include_str!("../../../tests/golden/stage-10-drawn-direct-on-line.request.json");
const DRAWN_DIRECT_ON_LINE_SWAPPED: &str =
    include_str!("../../../tests/golden/stage-10-drawn-direct-on-line-swapped.request.json");
const DRAWN_STAR_DELTA: &str =
    include_str!("../../../tests/golden/stage-10-drawn-star-delta.request.json");
const DRAWN_STAR_DELTA_SWAPPED: &str =
    include_str!("../../../tests/golden/stage-10-drawn-star-delta-swapped.request.json");

const NAMES: [&str; 4] = [
    "direct-on-line",
    "direct-on-line-swapped",
    "star-delta",
    "star-delta-swapped",
];

fn drawn(name: &str) -> CompileRequest {
    let text = match name {
        "direct-on-line" => DRAWN_DIRECT_ON_LINE,
        "direct-on-line-swapped" => DRAWN_DIRECT_ON_LINE_SWAPPED,
        "star-delta" => DRAWN_STAR_DELTA,
        "star-delta-swapped" => DRAWN_STAR_DELTA_SWAPPED,
        other => panic!("no drawing named {other}"),
    };
    serde_json::from_str(text).expect("the pinned drawn request decodes")
}

/// The fixture of `name` with its drawing swapped for the one the editor drew; the parts have
/// the same IDs, so the fixture's operator events name the same switches.
fn drawn_starter(name: &str) -> Starter {
    let mut fixture = starter(name);
    fixture.request = drawn(name);
    fixture
}

type Joined = BTreeSet<BTreeSet<(String, String)>>;

/// The pins that each net joins, by the Engine's own connectivity (`derive_netlist`).
fn nets(request: &CompileRequest) -> Joined {
    derive_netlist(&request.schematic, &request.geometry)
        .expect("the drawing resolves")
        .nodes
        .iter()
        .map(|node| {
            node.pins
                .iter()
                .map(|pin| {
                    (
                        pin.component_id.as_str().to_owned(),
                        pin.pin_id.as_str().to_owned(),
                    )
                })
                .collect()
        })
        .collect()
}

#[test]
fn every_drawn_starter_has_the_fixtures_parts_parameters_and_reference() {
    for name in NAMES {
        let fixture = starters().remove(name).unwrap().request;
        let drawing = drawn(name);
        let parts = |request: &CompileRequest| {
            serde_json::to_value(&request.schematic.components)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|part| {
                    (
                        part["id"].clone(),
                        part["typeId"].clone(),
                        part["parameters"].clone(),
                    )
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(parts(&drawing), parts(&fixture), "{name}");
        assert_eq!(drawing.schematic.reference_markers.len(), 1, "{name}");
        assert!(drawing.schematic.junctions.is_empty(), "{name}");
    }
}

#[test]
fn every_drawn_starter_joins_exactly_the_pins_of_its_fixture() {
    for name in NAMES {
        let fixture = starters().remove(name).unwrap().request;

        assert_eq!(
            nets(&drawn(name)),
            nets(&fixture),
            "{name}: the editor-drawn wiring is the fixture's, net for net"
        );
    }
}

#[test]
fn the_two_wrong_wirings_join_other_pins_than_the_correct_starters() {
    assert_ne!(
        nets(&drawn("direct-on-line")),
        nets(&drawn("direct-on-line-swapped"))
    );
    assert_ne!(
        nets(&drawn("star-delta")),
        nets(&drawn("star-delta-swapped"))
    );
}

fn picked(
    snapshot: &open_circuit_engine::ResultSnapshot,
    id: &open_circuit_engine::StableId,
) -> bool {
    snapshot.states.iter().any(|state| {
        matches!(state, DeviceState::Contactor { component_id, state: RelayContactState::Picked, .. }
            if component_id == id)
    })
}

fn check_drawn_starter(name: &str) {
    let fixture = drawn_starter(name);
    let (layout, snapshots, run) = bridged(&fixture);

    // The independent answers, on the frames the bridge delivers.
    meet_the_oracles(name, &fixture.motor, &layout, &snapshots);

    // Interlocks and the overload relay, as the drawn circuit behaves.
    let (main, star, delta) = (
        fixture.states.get("main").unwrap(),
        fixture.states.get("star"),
        fixture.states.get("delta"),
    );
    let overload = &fixture.states["overload"];
    for (step, snapshot) in snapshots.iter().enumerate() {
        assert!(
            snapshot.states.iter().any(|state| matches!(state,
                DeviceState::Fuse { component_id, state: FuseState::Intact, .. }
                    if component_id == overload)),
            "{name}: the overload relay tripped at step {step}"
        );
        if let (Some(star), Some(delta)) = (star, delta) {
            assert!(
                !(picked(snapshot, star) && picked(snapshot, delta)),
                "{name}: star and delta overlap at step {step}"
            );
            if (25..=110).contains(&step) {
                assert!(
                    picked(snapshot, main) && picked(snapshot, star) && !picked(snapshot, delta)
                );
            }
            if (125..=140).contains(&step) {
                assert!(
                    picked(snapshot, main) && !picked(snapshot, star) && !picked(snapshot, delta)
                );
            }
            if step >= 145 {
                assert!(
                    picked(snapshot, main) && !picked(snapshot, star) && picked(snapshot, delta)
                );
            }
        }
    }

    // The wrong wiring is visible and not repaired: the motor runs the other way.
    let speed = snapshots[LAST_STEP as usize].values
        [offset_of(&layout, &fixture.motor, "angular_velocity")];
    if name.ends_with("-swapped") {
        assert!(
            speed < -100.0,
            "{name}: swapped phases reverse the motor, speed {speed}"
        );
    } else {
        assert!(
            speed > 100.0,
            "{name}: the correct sequence runs forward, speed {speed}"
        );
    }
    assert_eq!(
        run.delivered.len() as u32,
        LAST_STEP,
        "{name}: one frame per public step"
    );
}

#[test]
fn the_drawn_direct_on_line_starter_runs_to_the_independent_answers() {
    check_drawn_starter("direct-on-line");
}

#[test]
fn the_drawn_direct_on_line_starter_with_two_phases_swapped_runs_the_other_way() {
    check_drawn_starter("direct-on-line-swapped");
}

#[test]
fn the_drawn_star_delta_starter_runs_to_the_independent_answers_and_keeps_its_interlocks() {
    check_drawn_starter("star-delta");
}

#[test]
fn the_drawn_star_delta_starter_with_two_phases_swapped_runs_the_other_way() {
    check_drawn_starter("star-delta-swapped");
}

#[test]
fn two_runs_of_a_drawn_starter_are_bit_identical_in_this_environment() {
    for name in ["direct-on-line", "star-delta"] {
        let fixture = drawn_starter(name);
        let (_, first, _) = bridged(&fixture);
        let (_, second, _) = bridged(&fixture);

        assert_eq!(bits(&first), bits(&second), "{name}");
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.states, b.states, "{name}");
        }
    }
}

#[test]
fn a_saved_and_reopened_drawn_starter_recomputes_the_same_run_from_step_zero() {
    let directory = tempfile::tempdir().unwrap();

    for name in NAMES {
        let request = drawn(name);
        let path = directory.path().join(format!("{name}.ocircuit"));
        write_project_file(&path, &request.schematic).expect("saved");
        let file = std::fs::read_to_string(&path).unwrap();
        for derived in [
            "snapshot",
            "trace",
            "runId",
            "layout",
            "thermalDose",
            "angularVelocity",
            "tripped",
            "conducting",
            "operating",
            "nodeId",
        ] {
            assert!(
                !file.contains(derived),
                "{name}: the file holds `{derived}`"
            );
        }
        let reopened = read_project_file(&path).expect("reopens");
        assert_eq!(
            reopened, request.schematic,
            "{name}: every drawn input round-trips"
        );

        // A run of the reopened drawing starts at step 0, and its first steps equal the original's.
        let again = CompileRequest {
            schematic: reopened,
            ..request.clone()
        };
        let mut first = Run::start_every_step(request);
        let mut second = Run::start_every_step(again);
        assert_eq!(second.initial_snapshot.metadata.simulation_step, 0);
        first.advance(30);
        second.advance(30);
        let frames = |run: &Run| {
            run.delivered
                .iter()
                .map(|(snapshot, _)| snapshot.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&frames(&first)), bits(&frames(&second)), "{name}");
    }
}

#[test]
fn a_drawn_starter_whose_motor_current_has_no_return_path_stops_the_run_with_its_last_good_frame() {
    // The shunts across the motor windings give the interrupted inductive current somewhere to
    // go (stage-10 contract section 8). Without them, opening the control circuit is an
    // incompatible ideal interruption: the Engine must say so and leave the run as it was.
    let fixture = starter("direct-on-line");
    let mut request = drawn("direct-on-line");
    let shunts = [8, 9, 10].map(|number| format!("00000000-0000-4000-8000-{number:012}"));
    request
        .schematic
        .components
        .retain(|part| !shunts.iter().any(|id| id.as_str() == part.id().as_str()));
    request.geometry.components.retain(|part| {
        !shunts
            .iter()
            .any(|id| id.as_str() == part.component_id.as_str())
    });
    let control = fixture.operations[0].component_id.as_str().to_owned();

    let mut run = Run::start_every_step(request);
    run.advance(21);
    let (_, closed) = run.set_operator_state(&control, OperatorPosition::Closed);
    assert!(
        matches!(closed, SetOperatorStateResult::Success { .. }),
        "{closed:?}"
    );
    run.advance(60);
    let before = run.delivered.last().unwrap().0.clone();

    let (_, opened) = run.set_operator_state(&control, OperatorPosition::Open);
    assert!(
        matches!(opened, SetOperatorStateResult::Success { .. }),
        "the open itself is a valid operator command: {opened:?}"
    );

    // The coil drops out a few milliseconds later and interrupts the motor current. The shunts
    // are gone, so the Engine cannot continue: the run stops instead of inventing a result.
    run.advance(300);
    assert!(
        !run.core.is_streaming(),
        "an interruption with no return path stops the run"
    );
    let last = run.delivered.last().unwrap().0.clone();
    assert!(last.metadata.simulation_step >= before.metadata.simulation_step);
    assert!(
        last.metadata.simulation_step < before.metadata.simulation_step + 300,
        "the run stopped when the contactor released, not at the end of the wait"
    );
    assert!(
        last.values.iter().all(|value| value.is_finite()),
        "the last frame is a real result"
    );
    assert_eq!(
        run.revision(),
        "rev-3",
        "the close and the open each adopted a revision"
    );
}
