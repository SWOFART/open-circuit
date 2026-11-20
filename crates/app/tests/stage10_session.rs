//! The Stage 10 bridge (issue #179): three-phase equipment run by the real Engine behind
//! `SimulationCore`, saved and opened by the application's project persistence, and carried the
//! way the application carries everything (JSON for commands, binary OCF2 frames, OCT1 traces).
//! The circuits and the answers are `tests/golden/stage-10/*` (independent oracles, never Engine
//! output). Nothing here adds a model: the bridge carries the new devices, their parameters, their
//! operator and reset commands and their result keys, and the Engine alone computes them.

mod common;

use common::stage10::{
    bits, bridged, direct, meet_the_oracles, offset_of, starter, starters, value,
};
use std::time::Instant;

use common::{Capture, Run};
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_engine::{
    CompileRequest, CompileResult, DeviceState, DiagnosticCode, FuseState, OperatorPosition,
    ParameterId, ResultQuantity, ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult,
    SetParameterResult, StableId, TraceQuery, TraceRequest,
};
use serde_json::{Value, json};

fn check_starter_through_the_bridge(name: &str) {
    let fixture = starter(name);
    let (layout, through_bridge, run) = bridged(&fixture);
    let straight = direct(&fixture);

    // The bridge carries values and discrete states exactly, step for step.
    assert_eq!(bits(&through_bridge), bits(&straight), "{name}: values");
    for (step, (a, b)) in through_bridge.iter().zip(&straight).enumerate() {
        assert_eq!(a.states, b.states, "{name}: discrete states at step {step}");
    }

    // Frame, layout and sequence checks hold for every delivered frame.
    let mut previous_sequence = 0;
    for (snapshot, _) in &run.delivered {
        assert_eq!(
            snapshot.metadata.layout_version, layout.layout_version,
            "{name}"
        );
        assert_eq!(snapshot.metadata.run_id, run.run_id, "{name}");
        assert!(
            snapshot.metadata.frame_sequence > previous_sequence,
            "{name}"
        );
        previous_sequence = snapshot.metadata.frame_sequence;
    }

    // And the independent answers: the same rows and tolerances the Engine is held to.
    meet_the_oracles(name, &fixture.motor, &layout, &through_bridge);
}

#[test]
fn direct_on_line_through_the_bridge_equals_the_engine_bit_for_bit_and_meets_the_oracles() {
    check_starter_through_the_bridge("direct-on-line");
}

#[test]
fn swapped_direct_on_line_through_the_bridge_equals_the_engine_and_meets_the_oracles() {
    check_starter_through_the_bridge("direct-on-line-swapped");
}

#[test]
fn star_delta_through_the_bridge_equals_the_engine_bit_for_bit_and_meets_the_oracles() {
    check_starter_through_the_bridge("star-delta");
}

#[test]
fn swapped_star_delta_through_the_bridge_equals_the_engine_and_meets_the_oracles() {
    check_starter_through_the_bridge("star-delta-swapped");
}

#[test]
fn the_new_result_keys_units_and_discrete_records_reach_the_application() {
    let fixture = starter("star-delta");
    let (layout, snapshots, _) = bridged(&fixture);
    let last = snapshots.last().unwrap();
    let components = serde_json::to_value(&fixture.request.schematic.components).unwrap();
    let mut seen = std::collections::BTreeSet::new();

    for component in components.as_array().unwrap() {
        let id = StableId::parse(component["id"].as_str().unwrap()).unwrap();
        let kind = component["typeId"].as_str().unwrap();
        let expected: Vec<&str> = match kind {
            "induction_motor_three_phase" => vec![
                "u/voltage",
                "u/current",
                "v/voltage",
                "v/current",
                "w/voltage",
                "w/current",
                "angular_velocity",
                "electromagnetic_torque",
            ],
            "contactor_three_pole" => vec![
                "coil/voltage",
                "coil/current",
                "coil/current_squared",
                "pole1/voltage",
                "pole1/current",
                "pole2/voltage",
                "pole2/current",
                "pole3/voltage",
                "pole3/current",
                "no/voltage",
                "no/current",
                "nc/voltage",
                "nc/current",
            ],
            "thermal_overload_three_pole" => vec![
                "pole1/voltage",
                "pole1/current",
                "pole2/voltage",
                "pole2/current",
                "pole3/voltage",
                "pole3/current",
                "no/voltage",
                "no/current",
                "nc/voltage",
                "nc/current",
                "thermal_dose",
            ],
            _ => continue,
        };
        seen.insert(kind);
        let offsets: Vec<usize> = expected
            .iter()
            .map(|key| offset_of(&layout, &id, key))
            .collect();
        // Within a device the keys are published in the contract's order.
        assert!(
            offsets.windows(2).all(|pair| pair[0] < pair[1]),
            "{kind}: order"
        );
        assert!(
            offsets
                .iter()
                .all(|offset| last.values[*offset].is_finite()),
            "{kind}"
        );

        let quantity_of = |key: &str| {
            layout
                .slots
                .iter()
                .find(|slot| slot.key == format!("{}/{key}", id.as_str()))
                .unwrap()
                .quantity
        };
        match kind {
            "contactor_three_pole" => {
                assert_eq!(
                    quantity_of("coil/current_squared"),
                    ResultQuantity::MeanSquareCurrent
                );
            }
            "thermal_overload_three_pole" => {
                assert_eq!(quantity_of("thermal_dose"), ResultQuantity::ThermalDose);
            }
            "induction_motor_three_phase" => {
                assert_eq!(
                    quantity_of("electromagnetic_torque"),
                    ResultQuantity::ElectromagneticTorque
                );
            }
            _ => {}
        }
    }
    assert_eq!(
        seen.len(),
        3,
        "motor, contactor and overload were all checked"
    );

    // One discrete record per switching device, using the existing record kinds.
    let contactors = last
        .states
        .iter()
        .filter(|state| matches!(state, DeviceState::Contactor { .. }))
        .count();
    let overloads = last
        .states
        .iter()
        .filter(|state| matches!(state, DeviceState::Fuse { .. }))
        .count();
    let three_pole_contactors = components
        .as_array()
        .unwrap()
        .iter()
        .filter(|component| component["typeId"] == "contactor_three_pole")
        .count();
    assert_eq!(contactors, three_pole_contactors);
    assert_eq!(overloads, 1);
}

fn index_id(index: usize) -> String {
    format!("00000000-0000-4000-8000-{index:012}")
}

type Part<'a> = (&'a str, Value, &'a [(&'a str, i32)]);

fn request(parts: &[Part<'_>]) -> CompileRequest {
    let components: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(index, (kind, parameters, _))| {
            json!({"id": index_id(index), "typeId": kind, "position": {"x": index, "y": 2},
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
            json!({"componentId": index_id(index), "pins": pins})
        })
        .collect();
    serde_json::from_value(json!({
        "documentRevision": "rev-1", "geometry": {"components": geometry},
        "schematic": {"schemaVersion": 3, "documentation": {"designators":[],"wireNumbers":[],"terminals":[],"notes":[],"titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},"sheets":[]}, "components": components, "wires": [],
            "rails": [], "junctions": [], "referenceMarkers": [
                {"id": index_id(parts.len()), "position": {"x": 0, "y": 0}}
            ]}
    }))
    .unwrap()
}

fn breaker_request() -> CompileRequest {
    request(&[
        (
            "breaker_three_pole",
            json!({"closedContactResistanceOhms": 0.01, "instantaneousTripAmps": 2}),
            &[
                ("l1", 1),
                ("t1", 2),
                ("l2", 1),
                ("t2", 3),
                ("l3", 1),
                ("t3", 4),
            ],
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts": 6, "internalResistanceOhms": 0}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms": 20}),
            &[("pin1", 2), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms": 10}),
            &[("pin1", 3), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms": 5}),
            &[("pin1", 4), ("pin2", 0)],
        ),
    ])
}

fn overload_request(volts: f64) -> CompileRequest {
    let source = json!({"voltageVolts": volts, "internalResistanceOhms": 0});
    request(&[
        (
            "thermal_overload_three_pole",
            json!({"closedContactResistanceOhms": 0.01, "senseResistanceOhms": 1,
            "ratedCurrentAmps": 10, "thermalSenseSeconds": 0.02, "tripScaleSeconds": 0.1,
            "coolSeconds": 0.1, "resetDose": 0.25}),
            &[
                ("l1", 1),
                ("t1", 0),
                ("l2", 2),
                ("t2", 0),
                ("l3", 3),
                ("t3", 0),
                ("no_1", 4),
                ("no_2", 5),
                ("nc_1", 4),
                ("nc_2", 6),
            ],
        ),
        (
            "dc_voltage_source",
            source.clone(),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "dc_voltage_source",
            source.clone(),
            &[("pin1", 2), ("pin2", 0)],
        ),
        ("dc_voltage_source", source, &[("pin1", 3), ("pin2", 0)]),
        (
            "dc_voltage_source",
            json!({"voltageVolts": 12, "internalResistanceOhms": 0}),
            &[("pin1", 4), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms": 10}),
            &[("pin1", 5), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms": 10}),
            &[("pin1", 6), ("pin2", 0)],
        ),
    ])
}

fn tripped(run: &Run, snapshot: &ResultSnapshot) -> f64 {
    value(&run.layout, snapshot, &index_id(0), "tripped")
}

fn operate(run: &mut Run, component: usize, position: OperatorPosition) -> SetOperatorStateResult {
    run.set_operator_state(&index_id(component), position).1
}

fn latest(run: &Run) -> ResultSnapshot {
    run.delivered.last().unwrap().0.clone()
}

#[test]
fn a_breaker_is_closed_tripped_refused_and_reset_through_the_application_commands() {
    let mut run = Run::start_every_step(breaker_request());
    run.advance(5);
    let SetOperatorStateResult::Success { snapshot, .. } =
        operate(&mut run, 0, OperatorPosition::Closed)
    else {
        panic!("closing is accepted");
    };
    for (index, resistance) in [20.0, 10.0, 5.0].into_iter().enumerate() {
        let current = value(
            &run.layout,
            &snapshot,
            &index_id(0),
            &format!("pole{}/current", index + 1),
        );
        assert!((current - 6.0 / (resistance + 0.01)).abs() < 1e-10);
    }
    assert!(snapshot.states.iter().any(|state| matches!(
        state,
        DeviceState::Switch {
            conducting: true,
            ..
        }
    )));

    // A live source edit pushes the current over the trip threshold.
    let (_, result) = run.set_parameter(&index_id(1), ParameterId::VoltageVolts, 12.0);
    let SetParameterResult::Success { snapshot, .. } = result else {
        panic!("a live source edit is accepted");
    };
    assert_eq!(tripped(&run, &snapshot), 1.0);
    assert!(snapshot.states.iter().any(|state| matches!(
        state,
        DeviceState::Switch {
            conducting: false,
            ..
        }
    )));
    for pole in 1..=3 {
        assert_eq!(
            value(
                &run.layout,
                &snapshot,
                &index_id(0),
                &format!("pole{pole}/current")
            ),
            0.0
        );
    }

    // Closing while latched is refused, and a refusal leaves the revision where it was.
    let revision = run.revision();
    let result = operate(&mut run, 0, OperatorPosition::Closed);
    let SetOperatorStateResult::Failure { diagnostics } = result else {
        panic!("closing a latched breaker is refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    assert_eq!(run.revision(), revision);
    run.advance(2);
    assert_eq!(tripped(&run, &latest(&run)), 1.0);
    assert_eq!(latest(&run).metadata.document_revision, revision);

    // An explicit open resets the latch; closing on the overcurrent trips again in the same reply.
    let SetOperatorStateResult::Success { snapshot, .. } =
        operate(&mut run, 0, OperatorPosition::Open)
    else {
        panic!("an explicit open is accepted");
    };
    assert_eq!(tripped(&run, &snapshot), 0.0);
    let SetOperatorStateResult::Success { snapshot, .. } =
        operate(&mut run, 0, OperatorPosition::Closed)
    else {
        panic!("closing a reset breaker is accepted");
    };
    assert_eq!(tripped(&run, &snapshot), 1.0);

    // The trip threshold is a live parameter; raising it does not clear the latch.
    let (_, result) = run.set_parameter(&index_id(0), ParameterId::InstantaneousTripAmps, 3.0);
    let SetParameterResult::Success { snapshot, .. } = result else {
        panic!("the trip threshold is a live parameter");
    };
    assert_eq!(tripped(&run, &snapshot), 1.0);
}

#[test]
fn an_overload_relay_trips_refuses_an_early_reset_and_resets_after_cooling() {
    let mut run = Run::start_every_step(overload_request(20.0));
    let dose = |run: &Run| value(&run.layout, &latest(run), &index_id(0), "thermal_dose");
    let overload_state = |run: &Run| {
        latest(run)
            .states
            .iter()
            .find_map(|state| match state {
                DeviceState::Fuse {
                    component_id,
                    state,
                    ..
                } if component_id.as_str() == index_id(0) => Some(*state),
                _ => None,
            })
            .expect("the overload's discrete record")
    };

    run.advance(1);
    assert_eq!(overload_state(&run), FuseState::Intact);
    run.advance(120);
    assert_eq!(overload_state(&run), FuseState::Tripped);
    assert!((dose(&run) - 1.0).abs() < 1e-9);

    // Hot and still overloaded: the reset is refused and nothing changes.
    let revision = run.revision();
    let result = operate(&mut run, 0, OperatorPosition::Pressed);
    let SetOperatorStateResult::Failure { diagnostics } = result else {
        panic!("an early reset is refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    assert_eq!(run.revision(), revision);

    // Remove the overload by a live edit of each phase, let the detector cool, then reset.
    for source in 1..=3 {
        let (_, result) = run.set_parameter(&index_id(source), ParameterId::VoltageVolts, 2.0);
        assert!(matches!(result, SetParameterResult::Success { .. }));
    }
    run.advance(400);
    assert_eq!(
        overload_state(&run),
        FuseState::Tripped,
        "cooling alone does not reset"
    );
    let cooled = dose(&run);
    assert!(cooled <= 0.25, "cooled to {cooled}");

    let SetOperatorStateResult::Success { snapshot, .. } =
        operate(&mut run, 0, OperatorPosition::Pressed)
    else {
        panic!("the reset is accepted once cooled");
    };
    assert!(snapshot.states.iter().any(|state| matches!(
        state,
        DeviceState::Fuse {
            state: FuseState::Intact,
            ..
        }
    )));
    // The reset keeps the accumulated dose.
    assert!((value(&run.layout, &snapshot, &index_id(0), "thermal_dose") - cooled).abs() < 1e-12);
}

#[test]
fn refused_updates_change_nothing_and_a_live_load_torque_edit_is_accepted() {
    let fixture = starter("direct-on-line");
    let mut actual = Run::start_every_step(fixture.request.clone());
    let mut control = Run::start_every_step(fixture.request.clone());
    actual.advance(3);
    control.advance(3);
    let contactor = fixture.states["main"].as_str().to_owned();

    // Pole pairs are a compile-only input; the contactor is driven by its coil, not by hand.
    let (_, refused) = actual.set_parameter(fixture.motor.as_str(), ParameterId::PolePairs, 3.0);
    assert!(matches!(refused, SetParameterResult::Failure { .. }));
    let (_, direct) = actual.set_operator_state(&contactor, OperatorPosition::Closed);
    let SetOperatorStateResult::Failure { diagnostics } = direct else {
        panic!("a contactor cannot be operated directly");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    assert_eq!(
        actual.revision(),
        "rev-1",
        "refusals do not adopt a revision"
    );

    // The motor load torque is live, and the same edit on a control run gives the same numbers.
    for run in [&mut actual, &mut control] {
        let (_, result) = run.set_parameter(
            fixture.motor.as_str(),
            ParameterId::LoadTorqueNewtonMeters,
            0.2,
        );
        assert!(matches!(result, SetParameterResult::Success { .. }));
        run.advance(5);
    }
    assert_eq!(
        bits(&[latest(&actual)]),
        bits(&[latest(&control)]),
        "refused updates left no trace in the run"
    );
}

#[test]
fn stale_run_and_revision_replies_are_refused_and_a_replaced_run_cannot_be_read() {
    let fixture = starter("direct-on-line");
    let mut run = Run::start_every_step(fixture.request.clone());
    run.advance(3);
    let old_run = run.run_id.clone();
    let motor = fixture.motor.clone();
    let command = |run_id: &StableId, expected: &str, next: &str| SetOperatorStateRequest {
        run_id: run_id.clone(),
        expected_document_revision: expected.to_owned(),
        document_revision: next.to_owned(),
        component_id: fixture.operations[0].component_id.clone(),
        position: OperatorPosition::Closed,
    };

    // A wrong run, a wrong expected revision and a reused revision are all refused.
    let unknown = StableId::parse(index_id(999)).unwrap();
    for (request, code) in [
        (
            command(&unknown, "rev-1", "rev-2"),
            DiagnosticCode::StaleRun,
        ),
        (
            command(&old_run, "rev-0", "rev-2"),
            DiagnosticCode::StaleDocumentRevision,
        ),
        (
            command(&old_run, "rev-1", "rev-1"),
            DiagnosticCode::StaleDocumentRevision,
        ),
    ] {
        let SetOperatorStateResult::Failure { diagnostics } = run.core.set_operator_state(request)
        else {
            panic!("expected a refusal with {code:?}");
        };
        assert_eq!(diagnostics[0].code, code);
    }

    // A new compile replaces the run: the old stream ends and the old run id is stale.
    let CompileResult::Success { run_id, .. } = run.core.compile(fixture.request.clone()) else {
        panic!("the replacement compiles");
    };
    assert_ne!(run_id, old_run);
    assert!(!run.core.is_streaming());
    let SetOperatorStateResult::Failure { diagnostics } = run
        .core
        .set_operator_state(command(&old_run, "rev-1", "rev-2"))
    else {
        panic!("the replaced run is stale");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::StaleRun);

    let read = TraceRequest {
        run_id: old_run,
        layout_version: run.layout.layout_version,
        signal_keys: vec![format!("{}/angular_velocity", motor.as_str())],
        query: TraceQuery::History {
            start_step: 0,
            end_step: 3,
        },
        max_points: 16,
    };
    let refused = run
        .core
        .read_traces(read, Box::new(Capture::default()), Instant::now());
    assert_eq!(refused.unwrap_err()[0].code, DiagnosticCode::StaleRun);
}

#[test]
fn a_saved_and_opened_three_phase_project_keeps_its_inputs_and_stores_nothing_derived() {
    let directory = tempfile::tempdir().unwrap();
    let mut requests: Vec<(String, CompileRequest)> = starters()
        .into_iter()
        .map(|(name, fixture)| (name, fixture.request))
        .collect();
    requests.push(("breaker".to_owned(), breaker_request()));
    requests.push(("overload".to_owned(), overload_request(20.0)));

    for (name, request) in requests {
        let path = directory.path().join(format!("{name}.ocircuit"));
        let mut run = Run::start_every_step(request.clone());
        run.advance(5);

        write_project_file(&path, &request.schematic).expect("saved after the run");
        let file = std::fs::read_to_string(&path).unwrap();
        for derived in [
            "snapshot",
            "trace",
            "runId",
            "layout",
            "thermalDose",
            "thermal_dose",
            "angularVelocity",
            "electromagneticTorque",
            "tripped",
            "conducting",
            "operating",
            "frameSequence",
        ] {
            assert!(
                !file.contains(derived),
                "{name}: the file holds `{derived}`"
            );
        }
        let reopened = read_project_file(&path).expect("reopens");
        assert_eq!(
            reopened, request.schematic,
            "{name}: every drawn parameter round-trips"
        );

        // A run of the reopened document starts at step 0 and recomputes the same numbers.
        let again = CompileRequest {
            schematic: reopened,
            ..request.clone()
        };
        let mut second = Run::start_every_step(again);
        assert_eq!(second.initial_snapshot.metadata.simulation_step, 0);
        second.advance(5);
        let delivered = |run: &Run| {
            run.delivered
                .iter()
                .map(|(frame, _)| frame.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            bits(&delivered(&run)),
            bits(&delivered(&second)),
            "{name}: recalculated"
        );
    }
}

/// The live parameters of the Stage 10 devices, as the contract names them (section 7): the
/// motor's load torque, the contactor's pickup and dropout thresholds and the breaker's trip
/// threshold. Every other input of these devices is compile-only. The application's own
/// classification (`liveParameterUpdate` in `src/ui/editorCommands.ts`) pins the same list in its
/// tests, so a drawn edit is sent as a live update exactly when the Engine accepts one.
fn live_parameters(kind: &str) -> &'static [&'static str] {
    match kind {
        "induction_motor_three_phase" => &["loadTorqueNewtonMeters"],
        "contactor_three_pole" => &["pickupCurrentAmps", "dropoutCurrentAmps"],
        "breaker_three_pole" => &["instantaneousTripAmps"],
        _ => &[],
    }
}

#[test]
fn only_the_contract_live_parameters_of_the_new_devices_are_accepted_as_live_updates() {
    let catalog = open_circuit_engine::component_catalog();
    let mut circuits = vec![("direct-on-line", starter("direct-on-line").request)];
    circuits.push(("breaker", breaker_request()));
    let mut checked = std::collections::BTreeSet::new();

    for (name, request) in circuits {
        let components = serde_json::to_value(&request.schematic.components).unwrap();
        let mut run = Run::start_every_step(request);
        run.advance(2);

        for component in components.as_array().unwrap() {
            let kind = component["typeId"].as_str().unwrap();
            if !matches!(
                kind,
                "contactor_three_pole"
                    | "thermal_overload_three_pole"
                    | "induction_motor_three_phase"
                    | "breaker_three_pole"
            ) {
                continue;
            }
            let entry = catalog
                .component_types
                .iter()
                .find(|entry| serde_json::to_value(entry.type_id).unwrap() == kind)
                .unwrap();
            for definition in &entry.parameters {
                let key = serde_json::to_value(definition.id).unwrap();
                let key = key.as_str().unwrap();
                let Some(current) = component["parameters"][key].as_f64() else {
                    continue;
                };
                let revision = run.revision();
                let (_, result) =
                    run.set_parameter(component["id"].as_str().unwrap(), definition.id, current);
                let live = live_parameters(kind).contains(&key);
                assert_eq!(
                    matches!(result, SetParameterResult::Success { .. }),
                    live,
                    "{name} {kind} {key}: a live update must be accepted exactly when the contract says so"
                );
                if !live {
                    assert_eq!(
                        run.revision(),
                        revision,
                        "{kind} {key}: a refusal adopts no revision"
                    );
                }
                checked.insert((kind.to_owned(), key.to_owned()));
            }
        }
    }
    // The classification was exercised for live and for compile-only inputs of each device.
    for kind in [
        "contactor_three_pole",
        "thermal_overload_three_pole",
        "induction_motor_three_phase",
        "breaker_three_pole",
    ] {
        assert!(
            checked.iter().filter(|(k, _)| k == kind).count() >= 2,
            "{kind}"
        );
    }
}
