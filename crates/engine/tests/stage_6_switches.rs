//! Stage 6 switches, push buttons, and operator state
//! (docs/contracts/stage-6-control-circuits.md sections 3.1, 4, and 5).

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentGeometry, ComponentSpec,
    DcVoltageSourceParameters, DeviceState, DiagnosticCode, Engine, FaultKind, FaultSpec,
    GeometryEnvelope, GridPoint, OperatorPosition, ParameterId, PushButtonParameters,
    PushButtonPosition, ReferenceMarkerSpec, RelayContactState, RelayParameters,
    ResistorParameters, ResolvedPinPosition, ResultLayout, ResultSnapshot, SCHEMA_VERSION,
    Schematic, SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest,
    SetParameterResult, StableId, SwitchParameters, SwitchPosition, TraceQuery, TraceRequest,
    component_catalog,
};
use serde_json::Value;

const DIVIDER: &str = include_str!("../../../tests/golden/stage-6-switch-divider.expected.json");
const HOLD: &str =
    include_str!("../../../tests/golden/stage-6-button-capacitor-hold.expected.json");

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const CONTACT: &str = "00000000-0000-4000-8000-000000000002";
const R1: &str = "00000000-0000-4000-8000-000000000003";
const R2: &str = "00000000-0000-4000-8000-000000000004";
const MARKER: &str = "00000000-0000-4000-8000-000000000009";

fn id(value: &str) -> StableId {
    StableId::parse(value).unwrap()
}

fn point(x: i32, y: i32) -> GridPoint {
    GridPoint { x, y }
}

fn pin(pin_id: &str, position: GridPoint) -> ResolvedPinPosition {
    ResolvedPinPosition {
        pin_id: pin_id.to_owned(),
        position,
    }
}

/// A component and the two grid points its pins sit on; equal points are one node.
struct Part {
    spec: ComponentSpec,
    pin1: GridPoint,
    pin2: GridPoint,
}

fn resistor(component_id: &str, ohms: f64, pin1: GridPoint, pin2: GridPoint) -> Part {
    Part {
        spec: ComponentSpec::Resistor {
            id: id(component_id),
            position: pin1,
            rotation_deg: 0,
            parameters: ResistorParameters {
                resistance_ohms: ohms,
            },
        },
        pin1,
        pin2,
    }
}

fn source(volts: f64, pin1: GridPoint, pin2: GridPoint) -> Part {
    Part {
        spec: ComponentSpec::DcVoltageSource {
            id: id(SOURCE),
            position: pin1,
            rotation_deg: 0,
            parameters: DcVoltageSourceParameters {
                voltage_volts: volts,
                internal_resistance_ohms: 0.0,
            },
        },
        pin1,
        pin2,
    }
}

fn switch(initial_closed: Option<f64>, pin1: GridPoint, pin2: GridPoint) -> Part {
    Part {
        spec: ComponentSpec::SwitchSpst {
            id: id(CONTACT),
            position: pin1,
            rotation_deg: 0,
            parameters: SwitchParameters {
                closed_resistance_ohms: 0.01,
                initial_closed,
            },
        },
        pin1,
        pin2,
    }
}

fn button(normally_closed: bool, pin1: GridPoint, pin2: GridPoint) -> Part {
    let parameters = PushButtonParameters {
        closed_resistance_ohms: 0.01,
    };
    let spec = if normally_closed {
        ComponentSpec::PushButtonNc {
            id: id(CONTACT),
            position: pin1,
            rotation_deg: 0,
            parameters,
        }
    } else {
        ComponentSpec::PushButtonNo {
            id: id(CONTACT),
            position: pin1,
            rotation_deg: 0,
            parameters,
        }
    };
    Part { spec, pin1, pin2 }
}

fn request(parts: Vec<Part>, ground: GridPoint, revision: &str) -> CompileRequest {
    let geometry = GeometryEnvelope {
        components: parts
            .iter()
            .map(|part| ComponentGeometry {
                component_id: part.spec.id().clone(),
                pins: vec![pin("pin1", part.pin1), pin("pin2", part.pin2)],
            })
            .collect(),
    };
    CompileRequest {
        schematic: Schematic {
            schema_version: SCHEMA_VERSION,
            components: parts.into_iter().map(|part| part.spec).collect(),
            wires: Vec::new(),
            junctions: Vec::new(),
            reference_markers: vec![ReferenceMarkerSpec {
                id: id(MARKER),
                position: ground,
            }],
        },
        geometry,
        document_revision: revision.to_owned(),
        faults: None,
    }
}

/// An ideal 10 V source, a contact, and two 1 kOhm resistors in series.
fn divider(contact: Part, revision: &str) -> CompileRequest {
    let ground = point(0, 4);
    request(
        vec![
            source(10.0, point(0, 0), ground),
            contact,
            resistor(R1, 1000.0, point(4, 0), point(6, 0)),
            resistor(R2, 1000.0, point(6, 0), ground),
        ],
        ground,
        revision,
    )
}

fn divider_switch(initial_closed: Option<f64>, revision: &str) -> CompileRequest {
    divider(switch(initial_closed, point(0, 0), point(4, 0)), revision)
}

struct Run {
    engine: Engine,
    run_id: StableId,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
    revisions: u32,
}

impl Run {
    fn new(request: CompileRequest) -> Self {
        let mut engine = Engine::default();
        let CompileResult::Success {
            run_id,
            layout,
            initial_snapshot,
            ..
        } = engine.compile(request)
        else {
            panic!("the circuit should compile");
        };
        Self {
            engine,
            run_id,
            layout,
            snapshot: initial_snapshot,
            revisions: 0,
        }
    }

    fn revision(&self) -> String {
        self.snapshot.metadata.document_revision.clone()
    }

    fn advance_to(&mut self, step: u64) {
        let count = step - self.snapshot.metadata.simulation_step;
        if count == 0 {
            return;
        }
        self.snapshot = self
            .engine
            .advance(AdvanceRequest {
                run_id: self.run_id.clone(),
                step_count: u32::try_from(count).unwrap(),
            })
            .expect("the run should advance");
    }

    fn operate_request(&mut self, position: OperatorPosition) -> SetOperatorStateRequest {
        self.revisions += 1;
        SetOperatorStateRequest {
            run_id: self.run_id.clone(),
            expected_document_revision: self.revision(),
            document_revision: format!("operator-{}", self.revisions),
            component_id: id(CONTACT),
            position,
        }
    }

    fn operate(&mut self, position: OperatorPosition) {
        let request = self.operate_request(position);
        match self.engine.set_operator_state(request) {
            SetOperatorStateResult::Success { snapshot, .. } => self.snapshot = snapshot,
            SetOperatorStateResult::Failure { diagnostics } => {
                panic!("the position change should succeed: {diagnostics:?}")
            }
        }
    }

    fn value(&self, key: &str) -> f64 {
        value(&self.layout, &self.snapshot, key)
    }
}

fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, key: &str) -> f64 {
    let slot = layout.slots.iter().find(|slot| slot.key == key).unwrap();
    snapshot.values[slot.offset as usize]
}

/// The position, conduction, and last transition of the one contact of the circuit.
fn contact_state(snapshot: &ResultSnapshot) -> (String, bool, Option<f64>) {
    let states = snapshot
        .states
        .iter()
        .filter(|state| !matches!(state, DeviceState::Relay { .. } | DeviceState::Fuse { .. }))
        .collect::<Vec<_>>();
    let [state] = states.as_slice() else {
        panic!("the snapshot contains exactly one contact state: {states:?}");
    };
    match state {
        DeviceState::Switch {
            position,
            conducting,
            last_transition_time_seconds,
            ..
        } => (
            match position {
                SwitchPosition::Open => "open",
                SwitchPosition::Closed => "closed",
            }
            .to_owned(),
            *conducting,
            *last_transition_time_seconds,
        ),
        DeviceState::PushButton {
            position,
            conducting,
            last_transition_time_seconds,
            ..
        } => (
            match position {
                PushButtonPosition::Released => "released",
                PushButtonPosition::Pressed => "pressed",
            }
            .to_owned(),
            *conducting,
            *last_transition_time_seconds,
        ),
        _ => unreachable!("filtered above"),
    }
}

fn operator_position(word: &str) -> OperatorPosition {
    match word {
        "open" => OperatorPosition::Open,
        "closed" => OperatorPosition::Closed,
        "released" => OperatorPosition::Released,
        "pressed" => OperatorPosition::Pressed,
        other => panic!("unknown position {other}"),
    }
}

fn close_to(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} differs from {expected} by more than {tolerance}"
    );
}

/// Replays a fixture: at each step, first the requests sent while it was the newest, then the
/// checks of that step.
fn replay(fixture: &str, mut run: Run, check: impl Fn(&Run, &Value)) {
    let fixture: Value = serde_json::from_str(fixture).unwrap();
    let events = fixture["operatorEvents"].as_array().unwrap();
    let expected = fixture["expected"].as_array().unwrap();
    let last_step = expected
        .iter()
        .map(|entry| entry["step"].as_u64().unwrap())
        .max()
        .unwrap();
    for step in 0..=last_step {
        run.advance_to(step);
        for event in events {
            if event["sentWhenNewestStep"].as_u64().unwrap() == step {
                run.operate(operator_position(event["position"].as_str().unwrap()));
            }
        }
        for entry in expected {
            if entry["step"].as_u64().unwrap() == step {
                let (position, conducting, _) = contact_state(&run.snapshot);
                assert_eq!(
                    position,
                    entry
                        .get("switchPosition")
                        .or(entry.get("buttonPosition"))
                        .unwrap()
                        .as_str()
                        .unwrap(),
                    "position at step {step}"
                );
                assert_eq!(
                    conducting,
                    entry
                        .get("switchConducting")
                        .or(entry.get("buttonConducting"))
                        .unwrap()
                        .as_bool()
                        .unwrap(),
                    "conduction at step {step}"
                );
                check(&run, entry);
            }
        }
    }
}

#[test]
fn a_switch_in_a_divider_follows_the_known_answers() {
    replay(
        DIVIDER,
        Run::new(divider_switch(None, "divider")),
        |run, entry| {
            let step = entry["step"].as_u64().unwrap();
            close_to(
                run.value(&format!("{R2}:voltage")),
                entry["outputVolts"].as_f64().unwrap(),
                entry["voltToleranceVolts"].as_f64().unwrap(),
                &format!("output at step {step}"),
            );
            close_to(
                run.value(&format!("{R2}:current")),
                entry["currentAmps"].as_f64().unwrap(),
                entry["currentToleranceAmps"].as_f64().unwrap(),
                &format!("current at step {step}"),
            );
            // The contact reports the same current, and nothing when it is open.
            close_to(
                run.value(&format!("{CONTACT}:current")),
                entry["currentAmps"].as_f64().unwrap(),
                entry["currentToleranceAmps"].as_f64().unwrap(),
                &format!("contact current at step {step}"),
            );
        },
    );
}

#[test]
fn a_normally_closed_button_holds_a_capacitor_and_releases_it_again() {
    let ground = point(0, 4);
    let capacitor = |pin1: GridPoint, pin2: GridPoint| Part {
        spec: serde_json::from_value(serde_json::json!({
            "typeId": "capacitor",
            "id": R2,
            "position": {"x": pin1.x, "y": pin1.y},
            "rotationDeg": 0,
            "parameters": {"capacitanceFarads": 1e-4, "initialVoltageVolts": 0.0},
        }))
        .unwrap(),
        pin1,
        pin2,
    };
    let circuit = request(
        vec![
            source(10.0, point(0, 0), ground),
            button(true, point(0, 0), point(4, 0)),
            resistor(R1, 1000.0, point(4, 0), point(6, 0)),
            capacitor(point(6, 0), ground),
        ],
        ground,
        "hold",
    );
    replay(HOLD, Run::new(circuit), |run, entry| {
        let step = entry["step"].as_u64().unwrap();
        close_to(
            run.value(&format!("{R2}:voltage")),
            entry["capacitorVolts"].as_f64().unwrap(),
            entry["toleranceVolts"].as_f64().unwrap(),
            &format!("capacitor at step {step}"),
        );
        if let Some(expected) = entry.get("currentAmps") {
            close_to(
                run.value(&format!("{CONTACT}:current")),
                expected.as_f64().unwrap(),
                entry["currentToleranceAmps"].as_f64().unwrap(),
                &format!("button current at step {step}"),
            );
        }
    });
}

#[test]
fn a_position_change_replaces_only_the_newest_sample() {
    let mut run = Run::new(divider_switch(None, "history"));
    run.advance_to(3);
    run.operate(OperatorPosition::Closed);
    let key = format!("{R2}:voltage");
    let batch = run
        .engine
        .read_traces(TraceRequest {
            run_id: run.run_id.clone(),
            layout_version: run.layout.layout_version,
            signal_keys: vec![key.clone()],
            query: TraceQuery::History {
                start_step: 0,
                end_step: 3,
            },
            max_points: 16,
        })
        .unwrap();
    let samples = batch
        .points
        .iter()
        .map(|point| (point.simulation_step, point.minimum_values[0]))
        .collect::<Vec<_>>();
    assert_eq!(samples.len(), 4);
    for (step, volts) in &samples[..3] {
        assert_eq!(*volts, 0.0, "step {step} is earlier and unchanged");
    }
    close_to(samples[3].1, 4.999975000125, 1e-9, "recomputed step 3");
}

#[test]
fn the_transition_time_is_the_start_of_the_recomputed_step() {
    let mut run = Run::new(divider_switch(None, "time"));
    assert_eq!(
        contact_state(&run.snapshot),
        ("open".to_owned(), false, None)
    );
    run.advance_to(5);
    run.operate(OperatorPosition::Closed);
    let (position, conducting, transition) = contact_state(&run.snapshot);
    assert_eq!((position.as_str(), conducting), ("closed", true));
    close_to(transition.unwrap(), 0.004, 1e-15, "transition time");
    assert_eq!(run.snapshot.metadata.simulation_step, 5);

    // The next step keeps the position, and the transition time stays.
    run.advance_to(6);
    assert_eq!(contact_state(&run.snapshot).2, transition);
}

#[test]
fn a_change_at_step_zero_restarts_the_initial_solve() {
    let mut run = Run::new(divider_switch(None, "zero"));
    run.operate(OperatorPosition::Closed);
    assert_eq!(run.snapshot.metadata.simulation_step, 0);
    close_to(
        run.value(&format!("{R2}:voltage")),
        4.999975000125,
        1e-9,
        "output",
    );
    assert_eq!(contact_state(&run.snapshot).2, Some(0.0));
    // The run goes on from the new circuit.
    run.advance_to(2);
    close_to(
        run.value(&format!("{R2}:voltage")),
        4.999975000125,
        1e-9,
        "output after advancing",
    );
}

#[test]
fn the_same_requests_at_the_same_steps_are_bit_identical() {
    let play = || {
        let mut run = Run::new(divider_switch(None, "determinism"));
        run.advance_to(3);
        run.operate(OperatorPosition::Closed);
        run.advance_to(7);
        run.operate(OperatorPosition::Open);
        run.advance_to(12);
        run.snapshot
    };
    let first = play();
    let second = play();
    let bits = |snapshot: &ResultSnapshot| {
        snapshot
            .values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(bits(&first), bits(&second));
    assert_eq!(first.states, second.states);
}

#[test]
fn a_request_for_the_current_position_adopts_the_revision_and_changes_nothing() {
    let mut run = Run::new(divider_switch(None, "same"));
    run.advance_to(4);
    let before = run.snapshot.clone();
    run.operate(OperatorPosition::Open);
    assert_eq!(run.snapshot.metadata.document_revision, "operator-1");
    assert_eq!(run.snapshot.metadata.simulation_step, 4);
    assert_eq!(run.snapshot.values, before.values);
    assert_eq!(run.snapshot.states, before.states);
    assert_eq!(contact_state(&run.snapshot).2, None);
    assert!(run.snapshot.metadata.frame_sequence > before.metadata.frame_sequence);

    // The new revision is the one the next request must expect.
    run.operate(OperatorPosition::Closed);
    assert_eq!(run.snapshot.metadata.document_revision, "operator-2");
}

#[test]
fn a_refused_request_leaves_the_run_unchanged() {
    let mut run = Run::new(divider_switch(None, "refusals"));
    run.advance_to(3);
    let snapshot = run.snapshot.clone();
    let refusal = |engine: &mut Engine, request: SetOperatorStateRequest| {
        let SetOperatorStateResult::Failure { diagnostics } = engine.set_operator_state(request)
        else {
            panic!("the request should be refused");
        };
        diagnostics[0].code
    };

    let mut stale_run = run.operate_request(OperatorPosition::Closed);
    stale_run.run_id = id("00000000-0000-4000-8000-0000000000aa");
    assert_eq!(
        refusal(&mut run.engine, stale_run),
        DiagnosticCode::StaleRun
    );

    let mut stale_revision = run.operate_request(OperatorPosition::Closed);
    stale_revision.expected_document_revision = "not-the-revision".to_owned();
    assert_eq!(
        refusal(&mut run.engine, stale_revision),
        DiagnosticCode::StaleDocumentRevision
    );

    let mut reused_revision = run.operate_request(OperatorPosition::Closed);
    reused_revision.document_revision = run.revision();
    assert_eq!(
        refusal(&mut run.engine, reused_revision),
        DiagnosticCode::StaleDocumentRevision
    );

    // A switch has no `pressed`, and a resistor cannot be operated at all.
    let wrong_position = run.operate_request(OperatorPosition::Pressed);
    assert_eq!(
        refusal(&mut run.engine, wrong_position),
        DiagnosticCode::InvalidParameter
    );
    let mut not_a_contact = run.operate_request(OperatorPosition::Closed);
    not_a_contact.component_id = id(R1);
    assert_eq!(
        refusal(&mut run.engine, not_a_contact),
        DiagnosticCode::InvalidParameter
    );
    let mut unknown = run.operate_request(OperatorPosition::Closed);
    unknown.component_id = id("00000000-0000-4000-8000-0000000000bb");
    assert_eq!(
        refusal(&mut run.engine, unknown),
        DiagnosticCode::InvalidParameter
    );

    assert_eq!(run.engine.latest_snapshot(), Some(&snapshot));
    // The run still takes a valid request with the unchanged revision.
    run.operate(OperatorPosition::Closed);
    assert_eq!(contact_state(&run.snapshot).0, "closed");
}

#[test]
fn a_push_button_rejects_the_positions_of_a_switch() {
    let mut run = Run::new(divider(button(false, point(0, 0), point(4, 0)), "button"));
    for position in [OperatorPosition::Open, OperatorPosition::Closed] {
        let request = run.operate_request(position);
        let SetOperatorStateResult::Failure { diagnostics } =
            run.engine.set_operator_state(request)
        else {
            panic!("a push button has no {position:?}");
        };
        assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    }
    run.operate(OperatorPosition::Pressed);
    assert_eq!(
        contact_state(&run.snapshot),
        ("pressed".to_owned(), true, Some(0.0))
    );
}

#[test]
fn a_compile_starts_every_position_at_its_initial_one() {
    let mut run = Run::new(divider_switch(Some(1.0), "initial"));
    assert_eq!(
        contact_state(&run.snapshot),
        ("closed".to_owned(), true, None)
    );
    close_to(
        run.value(&format!("{R2}:voltage")),
        4.999975000125,
        1e-9,
        "closed from the start",
    );
    run.advance_to(2);
    run.operate(OperatorPosition::Open);
    assert_eq!(contact_state(&run.snapshot).0, "open");

    // A new compile of the same document is a new run at the saved initial position.
    let mut fresh = Run::new(divider_switch(Some(1.0), "initial-again"));
    assert_eq!(contact_state(&fresh.snapshot).0, "closed");
    assert_ne!(fresh.run_id, run.run_id);

    // The earlier run is gone: a request for it is stale.
    let request = run.operate_request(OperatorPosition::Closed);
    let SetOperatorStateResult::Failure { .. } = fresh.engine.set_operator_state(request) else {
        panic!("a request for another run should be refused");
    };
    let initial_open = Run::new(divider_switch(Some(0.0), "explicit-open"));
    assert_eq!(contact_state(&initial_open.snapshot).0, "open");
}

#[test]
fn a_parameter_update_keeps_the_position() {
    let mut run = Run::new(divider_switch(None, "parameter"));
    run.advance_to(2);
    run.operate(OperatorPosition::Closed);
    let update = SetParameterRequest {
        run_id: run.run_id.clone(),
        expected_document_revision: run.revision(),
        document_revision: "parameter-2".to_owned(),
        component_id: id(CONTACT),
        parameter_id: ParameterId::ClosedResistanceOhms,
        value: 1000.0,
    };
    let SetParameterResult::Success { snapshot, .. } = run.engine.set_parameter(update) else {
        panic!("the closed resistance is a live parameter");
    };
    assert_eq!(contact_state(&snapshot).0, "closed");
    // 10 V over 1 kOhm, 1 kOhm, and 1 kOhm.
    close_to(
        value(&run.layout, &snapshot, &format!("{R2}:current")),
        10.0 / 3000.0,
        1e-12,
        "current with the new closed resistance",
    );
    run.snapshot = snapshot;

    // The initial position is an initial condition: only a compile changes it.
    let refused = SetParameterRequest {
        run_id: run.run_id.clone(),
        expected_document_revision: run.revision(),
        document_revision: "parameter-3".to_owned(),
        component_id: id(CONTACT),
        parameter_id: ParameterId::InitialClosed,
        value: 1.0,
    };
    let SetParameterResult::Failure { diagnostics } = run.engine.set_parameter(refused) else {
        panic!("initialClosed is not a live parameter");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
}

#[test]
fn an_initial_position_other_than_zero_or_one_is_an_invalid_parameter() {
    for bad in [0.5, -1.0, 2.0, f64::NAN] {
        let mut engine = Engine::default();
        let CompileResult::Failure { diagnostics } =
            engine.compile(divider_switch(Some(bad), "bad"))
        else {
            panic!("initialClosed = {bad} should be refused");
        };
        assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    }
}

#[test]
fn a_closed_resistance_that_is_not_positive_is_an_invalid_parameter() {
    let mut circuit = divider_switch(None, "resistance");
    let ComponentSpec::SwitchSpst { parameters, .. } = &mut circuit.schematic.components[1] else {
        panic!("the contact is the second component");
    };
    parameters.closed_resistance_ohms = 0.0;
    let CompileResult::Failure { diagnostics } = Engine::default().compile(circuit) else {
        panic!("a zero closed resistance should be refused");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
}

#[test]
fn closing_a_switch_across_an_ideal_source_is_accepted() {
    // A closed contact has a finite resistance: a large current, not a singular circuit.
    let ground = point(0, 4);
    let mut run = Run::new(request(
        vec![
            source(10.0, point(0, 0), ground),
            switch(None, point(0, 0), ground),
        ],
        ground,
        "short",
    ));
    run.advance_to(2);
    run.operate(OperatorPosition::Closed);
    close_to(
        run.value(&format!("{CONTACT}:current")),
        1000.0,
        1e-6,
        "current through the switch",
    );
}

#[test]
fn a_fault_on_a_contact_is_refused() {
    for kind in [FaultKind::Open, FaultKind::Short] {
        let mut circuit = divider_switch(None, "fault");
        circuit.faults = Some(vec![FaultSpec {
            id: id("00000000-0000-4000-8000-0000000000f1"),
            kind,
            component_id: id(CONTACT),
            parameter_id: None,
            value: None,
        }]);
        let CompileResult::Failure { diagnostics } = Engine::default().compile(circuit) else {
            panic!("a {kind:?} fault on a switch should be refused");
        };
        assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidFault);
    }
}

#[test]
fn a_relay_coil_behind_a_button_picks_and_drops_out() {
    let ground = point(0, 4);
    let relay = Part {
        spec: ComponentSpec::RelaySpdt {
            id: id(R1),
            position: point(4, 0),
            rotation_deg: 0,
            parameters: RelayParameters {
                coil_resistance_ohms: 100.0,
                coil_inductance_henries: 0.01,
                pickup_current_amps: 0.04,
                dropout_current_amps: 0.02,
                closed_contact_resistance_ohms: 0.1,
            },
        },
        pin1: point(4, 0),
        pin2: ground,
    };
    let mut circuit = request(
        vec![
            source(5.0, point(0, 0), ground),
            button(false, point(0, 0), point(4, 0)),
            relay,
        ],
        ground,
        "relay",
    );
    // The relay port pins are not the two pins of a plain part: add the contact pins, parked
    // on nodes of their own.
    let contacts = [
        ("common", point(10, 0)),
        ("normally_open", point(12, 0)),
        ("normally_closed", point(14, 0)),
    ];
    let geometry = circuit
        .geometry
        .components
        .iter_mut()
        .find(|geometry| geometry.component_id == id(R1))
        .unwrap();
    geometry.pins = vec![
        pin("coil_positive", point(4, 0)),
        pin("coil_negative", ground),
    ];
    geometry
        .pins
        .extend(contacts.iter().map(|(name, at)| pin(name, *at)));

    let mut run = Run::new(circuit);
    let relay_state = |snapshot: &ResultSnapshot| {
        snapshot
            .states
            .iter()
            .find_map(|state| match state {
                DeviceState::Relay {
                    state,
                    last_transition_time_seconds,
                    ..
                } => Some((*state, *last_transition_time_seconds)),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(relay_state(&run.snapshot).0, RelayContactState::Released);
    run.advance_to(3);
    run.operate(OperatorPosition::Pressed);
    let (state, transition) = relay_state(&run.snapshot);
    assert_eq!(state, RelayContactState::Picked);
    assert!(transition.unwrap() >= 0.002 - 1e-12, "{transition:?}");
    // 5 V over 100.01 Ohm of coil and closed button.
    close_to(
        run.value(&format!("{R1}/coil/current")),
        5.0 / 100.01,
        1e-4,
        "coil current",
    );

    run.advance_to(6);
    run.operate(OperatorPosition::Released);
    assert_eq!(relay_state(&run.snapshot).0, RelayContactState::Released);
    close_to(
        run.value(&format!("{R1}/coil/current")),
        0.0,
        1e-4,
        "coil current",
    );
}

#[test]
fn the_catalog_describes_the_contacts() {
    let catalog = component_catalog();
    let entry = |name: &str| {
        catalog
            .component_types
            .iter()
            .find(|entry| serde_json::to_value(entry.type_id).unwrap() == name)
            .unwrap_or_else(|| panic!("{name} is in the catalog"))
    };
    for name in ["switch_spst", "push_button_no", "push_button_nc"] {
        let entry = entry(name);
        let pins = entry
            .pins
            .iter()
            .map(|pin| pin.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(pins, ["pin1", "pin2"], "{name}");
        assert!(entry.fault_kinds.is_empty(), "{name} has no fault kinds");
        let resistance = &entry.parameters[0];
        assert_eq!(resistance.default_value, 0.01);
        assert!(!resistance.optional && !resistance.initial_condition);
    }
    let initial = &entry("switch_spst").parameters[1];
    assert_eq!(serde_json::to_value(initial.id).unwrap(), "initialClosed");
    assert_eq!(serde_json::to_value(initial.unit).unwrap(), "dimensionless");
    assert_eq!(serde_json::to_value(initial.domain).unwrap(), "zero_or_one");
    assert!(initial.optional && initial.initial_condition);
    assert_eq!(entry("push_button_no").parameters.len(), 1);
    assert_eq!(entry("push_button_nc").parameters.len(), 1);
}

#[test]
fn the_saved_form_has_only_the_initial_position() {
    let saved = serde_json::to_value(&divider_switch(Some(1.0), "saved").schematic).unwrap();
    let switch = &saved["components"][1];
    assert_eq!(switch["typeId"], "switch_spst");
    assert_eq!(
        switch["parameters"],
        serde_json::json!({"closedResistanceOhms": 0.01, "initialClosed": 1.0})
    );
    let open = serde_json::to_value(&divider_switch(None, "saved").schematic).unwrap();
    assert_eq!(
        open["components"][1]["parameters"],
        serde_json::json!({"closedResistanceOhms": 0.01})
    );
    let button =
        serde_json::to_value(&divider(button(true, point(0, 0), point(4, 0)), "saved").schematic)
            .unwrap();
    assert_eq!(button["components"][1]["typeId"], "push_button_nc");
    assert_eq!(
        button["components"][1]["parameters"],
        serde_json::json!({"closedResistanceOhms": 0.01})
    );
    // A position reached while running is not a field of the saved form.
    let with_position = r#"{"closedResistanceOhms": 0.01, "closed": true}"#;
    assert!(serde_json::from_str::<SwitchParameters>(with_position).is_err());
    let null_initial = r#"{"closedResistanceOhms": 0.01, "initialClosed": null}"#;
    assert!(serde_json::from_str::<SwitchParameters>(null_initial).is_err());
}
