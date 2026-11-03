//! Stage 6 contactor (docs/contracts/stage-6-control-circuits.md sections 3.2, 5, and 8).

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentGeometry, ComponentSpec,
    DcVoltageSourceParameters, DeviceState, DiagnosticCode, Engine, FaultKind, FaultSpec,
    GeometryEnvelope, GridPoint, OperatorPosition, ParameterId, PushButtonParameters,
    ReferenceMarkerSpec, RelayContactState, RelayParameters, ResistorParameters,
    ResolvedPinPosition, ResultLayout, ResultSnapshot, SCHEMA_VERSION, Schematic,
    SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest, SetParameterResult,
    StableId, component_catalog,
};
use serde_json::Value;

const LATCHING: &str =
    include_str!("../../../tests/golden/stage-6-start-stop-latching.expected.json");

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const STOP: &str = "00000000-0000-4000-8000-000000000002";
const START: &str = "00000000-0000-4000-8000-000000000003";
const QA: &str = "00000000-0000-4000-8000-000000000004";
const LOAD_SOURCE: &str = "00000000-0000-4000-8000-000000000005";
const LOAD: &str = "00000000-0000-4000-8000-000000000006";
const MARKER: &str = "00000000-0000-4000-8000-000000000009";
const SECOND_MARKER: &str = "00000000-0000-4000-8000-00000000000a";

fn id(value: &str) -> StableId {
    StableId::parse(value).unwrap()
}

fn point(x: i32, y: i32) -> GridPoint {
    GridPoint { x, y }
}

/// A component and the named grid points its pins sit on; equal points are one node.
struct Part {
    spec: ComponentSpec,
    pins: Vec<(&'static str, GridPoint)>,
}

fn two_pins(pin1: GridPoint, pin2: GridPoint) -> Vec<(&'static str, GridPoint)> {
    vec![("pin1", pin1), ("pin2", pin2)]
}

fn source(component_id: &str, volts: f64, pin1: GridPoint, pin2: GridPoint) -> Part {
    Part {
        spec: ComponentSpec::DcVoltageSource {
            id: id(component_id),
            position: pin1,
            rotation_deg: 0,
            parameters: DcVoltageSourceParameters {
                voltage_volts: volts,
                internal_resistance_ohms: 0.0,
            },
        },
        pins: two_pins(pin1, pin2),
    }
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
        pins: two_pins(pin1, pin2),
    }
}

fn button(component_id: &str, normally_closed: bool, pin1: GridPoint, pin2: GridPoint) -> Part {
    let parameters = PushButtonParameters {
        closed_resistance_ohms: 0.01,
    };
    let (id, position, rotation_deg) = (id(component_id), pin1, 0);
    let spec = if normally_closed {
        ComponentSpec::PushButtonNc {
            id,
            position,
            rotation_deg,
            parameters,
        }
    } else {
        ComponentSpec::PushButtonNo {
            id,
            position,
            rotation_deg,
            parameters,
        }
    };
    Part {
        spec,
        pins: two_pins(pin1, pin2),
    }
}

/// A contactor with the catalog defaults, except for the coil inductance. Its pins are the
/// coil on `a1` and `a2`, then the normally open contact, then the normally closed one.
fn contactor(
    coil_inductance_henries: f64,
    coil: (GridPoint, GridPoint),
    normally_open: (GridPoint, GridPoint),
    normally_closed: (GridPoint, GridPoint),
) -> Part {
    Part {
        spec: ComponentSpec::Contactor {
            id: id(QA),
            position: coil.0,
            rotation_deg: 0,
            parameters: RelayParameters {
                coil_resistance_ohms: 100.0,
                coil_inductance_henries,
                pickup_current_amps: 0.04,
                dropout_current_amps: 0.02,
                closed_contact_resistance_ohms: 0.1,
            },
        },
        pins: vec![
            ("a1", coil.0),
            ("a2", coil.1),
            ("no_1", normally_open.0),
            ("no_2", normally_open.1),
            ("nc_1", normally_closed.0),
            ("nc_2", normally_closed.1),
        ],
    }
}

fn request(parts: Vec<Part>, grounds: &[GridPoint], revision: &str) -> CompileRequest {
    let geometry = GeometryEnvelope {
        components: parts
            .iter()
            .map(|part| ComponentGeometry {
                component_id: part.spec.id().clone(),
                pins: part
                    .pins
                    .iter()
                    .map(|(pin_id, position)| ResolvedPinPosition {
                        pin_id: (*pin_id).to_owned(),
                        position: *position,
                    })
                    .collect(),
            })
            .collect(),
    };
    CompileRequest {
        schematic: Schematic {
            schema_version: SCHEMA_VERSION,
            components: parts.into_iter().map(|part| part.spec).collect(),
            wires: Vec::new(),
            junctions: Vec::new(),
            reference_markers: grounds
                .iter()
                .zip([MARKER, SECOND_MARKER])
                .map(|(position, marker)| ReferenceMarkerSpec {
                    id: id(marker),
                    position: *position,
                })
                .collect(),
        },
        geometry,
        document_revision: revision.to_owned(),
        faults: None,
    }
}

/// A 5 V source, a normally closed STOP button, START in parallel with the contactor's normally
/// open contact, and the contactor coil, in series (stage-6-start-stop-latching).
fn start_stop(seal_in: bool, revision: &str) -> CompileRequest {
    let ground = point(0, 10);
    let (p0, p1, p2) = (point(0, 0), point(4, 0), point(8, 0));
    let normally_open = if seal_in {
        (p1, p2)
    } else {
        (point(30, 0), point(32, 0))
    };
    request(
        vec![
            source(SOURCE, 5.0, p0, ground),
            button(STOP, true, p0, p1),
            button(START, false, p1, p2),
            contactor(
                0.01,
                (p2, ground),
                normally_open,
                (point(40, 0), point(42, 0)),
            ),
        ],
        &[ground],
        revision,
    )
}

struct Run {
    engine: Engine,
    run_id: StableId,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
    internal_substeps: u32,
    revisions: u32,
}

impl Run {
    fn new(request: CompileRequest) -> Self {
        let mut engine = Engine::default();
        let CompileResult::Success {
            run_id,
            layout,
            internal_substeps,
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
            internal_substeps,
            revisions: 0,
        }
    }

    fn step(&self) -> u64 {
        self.snapshot.metadata.simulation_step
    }

    fn advance_to(&mut self, step: u64) {
        let count = step - self.step();
        if count > 0 {
            self.snapshot = self
                .engine
                .advance(AdvanceRequest {
                    run_id: self.run_id.clone(),
                    step_count: u32::try_from(count).unwrap(),
                })
                .expect("the run should advance");
        }
    }

    fn operate_request(
        &mut self,
        component_id: &str,
        position: OperatorPosition,
    ) -> SetOperatorStateRequest {
        self.revisions += 1;
        SetOperatorStateRequest {
            run_id: self.run_id.clone(),
            expected_document_revision: self.snapshot.metadata.document_revision.clone(),
            document_revision: format!("operator-{}", self.revisions),
            component_id: id(component_id),
            position,
        }
    }

    fn operate(&mut self, component_id: &str, position: OperatorPosition) {
        let request = self.operate_request(component_id, position);
        match self.engine.set_operator_state(request) {
            SetOperatorStateResult::Success { snapshot, .. } => self.snapshot = snapshot,
            SetOperatorStateResult::Failure { diagnostics } => {
                panic!("the position change should succeed: {diagnostics:?}")
            }
        }
    }

    fn set_parameter(&mut self, component_id: &str, parameter_id: ParameterId, value: f64) {
        self.revisions += 1;
        let SetParameterResult::Success { snapshot, .. } =
            self.engine.set_parameter(SetParameterRequest {
                run_id: self.run_id.clone(),
                expected_document_revision: self.snapshot.metadata.document_revision.clone(),
                document_revision: format!("parameter-{}", self.revisions),
                component_id: id(component_id),
                parameter_id,
                value,
            })
        else {
            panic!("the parameter update should succeed");
        };
        self.snapshot = snapshot;
    }

    fn value(&self, key: &str) -> f64 {
        let slot = self
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == key)
            .unwrap();
        self.snapshot.values[slot.offset as usize]
    }

    fn coil_current(&self) -> f64 {
        self.value(&format!("{QA}/coil/current"))
    }

    fn contactor(&self) -> (RelayContactState, Option<f64>) {
        self.snapshot
            .states
            .iter()
            .find_map(|state| match state {
                DeviceState::Contactor {
                    state,
                    last_transition_time_seconds,
                    ..
                } => Some((*state, *last_transition_time_seconds)),
                _ => None,
            })
            .expect("the snapshot has a contactor state")
    }
}

fn state_name(state: RelayContactState) -> &'static str {
    match state {
        RelayContactState::Released => "released",
        RelayContactState::Picked => "picked",
    }
}

/// Replays the operator events of the latching fixture and checks `expected` at its steps.
fn replay_latching(run: &mut Run, events: &[Value], expected: &[Value], tolerance: f64) {
    let last_step = expected
        .iter()
        .map(|entry| entry["step"].as_u64().unwrap())
        .max()
        .unwrap();
    for step in 0..=last_step {
        run.advance_to(step);
        for event in events {
            if event["sentWhenNewestStep"].as_u64().unwrap() == step {
                let component = match event["component"].as_str().unwrap() {
                    "START" => START,
                    "STOP" => STOP,
                    other => panic!("unknown component {other}"),
                };
                let position = match event["position"].as_str().unwrap() {
                    "pressed" => OperatorPosition::Pressed,
                    "released" => OperatorPosition::Released,
                    other => panic!("unknown position {other}"),
                };
                run.operate(component, position);
            }
        }
        for entry in expected {
            if entry["step"].as_u64().unwrap() == step {
                assert_eq!(
                    state_name(run.contactor().0),
                    entry["contactor"].as_str().unwrap(),
                    "contactor at step {step}"
                );
                let current = entry["coilCurrentAmps"].as_f64().unwrap();
                assert!(
                    (run.coil_current() - current).abs() <= tolerance,
                    "coil current at step {step}: {} against {current}",
                    run.coil_current()
                );
            }
        }
    }
}

#[test]
fn a_start_stop_circuit_latches_through_the_contactor() {
    let fixture: Value = serde_json::from_str(LATCHING).unwrap();
    replay_latching(
        &mut Run::new(start_stop(true, "latching")),
        fixture["operatorEvents"].as_array().unwrap(),
        fixture["expected"].as_array().unwrap(),
        fixture["coilCurrentToleranceAmps"].as_f64().unwrap(),
    );
}

#[test]
fn without_a_seal_in_contact_the_circuit_does_not_latch() {
    let fixture: Value = serde_json::from_str(LATCHING).unwrap();
    let variant = &fixture["wrongVariants"][0];
    assert_eq!(variant["name"], "no seal-in contact");
    replay_latching(
        &mut Run::new(start_stop(false, "no-seal-in")),
        fixture["operatorEvents"].as_array().unwrap(),
        variant["expected"].as_array().unwrap(),
        fixture["coilCurrentToleranceAmps"].as_f64().unwrap(),
    );
}

#[test]
fn the_latching_run_is_bit_identical_when_repeated() {
    let play = || {
        let mut run = Run::new(start_stop(true, "determinism"));
        let mut trace = Vec::new();
        for step in 1..=30_u64 {
            run.advance_to(step);
            match step {
                5 => run.operate(START, OperatorPosition::Pressed),
                10 => run.operate(START, OperatorPosition::Released),
                15 => run.operate(STOP, OperatorPosition::Pressed),
                20 => run.operate(STOP, OperatorPosition::Released),
                _ => {}
            }
            trace.push((
                run.snapshot
                    .values
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>(),
                run.snapshot.states.clone(),
            ));
        }
        trace
    };
    assert_eq!(play(), play());
}

/// A 5 V source across the coil alone: a coil that rises and decays over many public steps.
fn coil_on_a_source(revision: &str) -> CompileRequest {
    let ground = point(0, 10);
    request(
        vec![
            source(SOURCE, 0.0, point(0, 0), ground),
            contactor(
                1.0,
                (point(0, 0), ground),
                (point(20, 0), point(22, 0)),
                (point(24, 0), point(26, 0)),
            ),
        ],
        &[ground],
        revision,
    )
}

#[test]
fn the_coil_picks_and_drops_out_at_the_analytical_times() {
    const R: f64 = 100.0;
    const L: f64 = 1.0;
    const TAU: f64 = L / R;
    const STEP: f64 = 0.001;
    let mut run = Run::new(coil_on_a_source("coil-response"));
    assert_eq!(run.contactor(), (RelayContactState::Released, None));
    let tolerance = STEP / f64::from(run.internal_substeps);

    // 5 V from the start of step 2 (t = 1 ms): i = 50 mA (1 - exp(-(t - 1 ms) / tau)).
    run.advance_to(2);
    run.set_parameter(SOURCE, ParameterId::VoltageVolts, 5.0);
    let source_start = STEP;
    let pickup_time = source_start + TAU * (1.0_f64 / (1.0 - 0.04 / 0.05)).ln();
    run.advance_to(30);
    let (state, transition) = run.contactor();
    assert_eq!(state, RelayContactState::Picked);
    let picked_at = transition.unwrap();
    assert!(
        (picked_at - pickup_time).abs() <= 2.0 * tolerance,
        "picked at {picked_at}, expected {pickup_time} within {}",
        2.0 * tolerance
    );

    // 0 V from the start of step 40 (t = 39 ms): the coil current decays from its value there.
    run.advance_to(40);
    let decay_start = 39.0 * STEP;
    let start_current = 0.05 * (1.0 - (-(decay_start - source_start) / TAU).exp());
    run.set_parameter(SOURCE, ParameterId::VoltageVolts, 0.0);
    run.advance_to(80);
    let (state, transition) = run.contactor();
    assert_eq!(state, RelayContactState::Released);
    let dropout_time = decay_start + TAU * (start_current / 0.02).ln();
    let dropped_at = transition.unwrap();
    assert!(
        (dropped_at - dropout_time).abs() <= 2.0 * tolerance,
        "dropped out at {dropped_at}, expected {dropout_time} within {}",
        2.0 * tolerance
    );
}

#[test]
fn the_contacts_are_isolated_from_the_coil_and_from_each_other() {
    // The coil is fed by a 5 V source. The normally open contact switches a 12 V source onto a
    // 100 Ohm load, and the normally closed contact connects a second 100 Ohm load: three
    // separate circuits that share no node.
    let (ground_a, ground_b, ground_c) = (point(0, 10), point(20, 10), point(40, 10));
    let mut circuit = request(
        vec![
            source(SOURCE, 5.0, point(0, 0), ground_a),
            contactor(
                0.01,
                (point(0, 0), ground_a),
                (point(20, 2), point(20, 4)),
                (point(40, 2), point(40, 4)),
            ),
            source(LOAD_SOURCE, 12.0, point(20, 0), ground_b),
            resistor(LOAD, 100.0, point(20, 4), ground_b),
            resistor(STOP, 100.0, point(40, 4), ground_c),
            source(START, 12.0, point(40, 2), ground_c),
        ],
        &[ground_a, ground_b],
        "isolation",
    );
    // The normally open contact sits between the 12 V source and its load.
    circuit.geometry.components[1].pins[2].position = point(20, 0);
    circuit.geometry.components[1].pins[3].position = point(20, 4);
    circuit.geometry.components[1].pins[4].position = point(40, 2);
    circuit.geometry.components[1].pins[5].position = point(40, 4);
    circuit
        .schematic
        .reference_markers
        .push(ReferenceMarkerSpec {
            id: id("00000000-0000-4000-8000-00000000000b"),
            position: ground_c,
        });
    let run = Run::new(circuit);

    // 5 V over the 100 Ohm coil is 50 mA, above the 40 mA pickup: picked at initialization.
    assert_eq!(run.contactor().0, RelayContactState::Picked);
    let close = |actual: f64, expected: f64, what: &str| {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{what}: {actual} against {expected}"
        );
    };
    close(run.coil_current(), 0.05, "coil current");
    close(
        run.value(&format!("{QA}/no/current")),
        12.0 / 100.1,
        "normally open current",
    );
    close(
        run.value(&format!("{QA}/no/voltage")),
        12.0 / 100.1 * 0.1,
        "normally open voltage",
    );
    close(
        run.value(&format!("{QA}/nc/current")),
        0.0,
        "normally closed current",
    );
    close(
        run.value(&format!("{LOAD}:current")),
        12.0 / 100.1,
        "load current",
    );
}

#[test]
fn a_contactor_that_is_not_energized_conducts_through_the_normally_closed_contact() {
    let ground = point(0, 10);
    let mut circuit = request(
        vec![
            source(SOURCE, 0.0, point(0, 0), ground),
            contactor(
                0.01,
                (point(0, 0), ground),
                (point(20, 0), point(20, 4)),
                (point(40, 0), point(40, 4)),
            ),
            source(LOAD_SOURCE, 12.0, point(40, 0), point(40, 10)),
            resistor(LOAD, 100.0, point(40, 4), point(40, 10)),
        ],
        &[ground, point(40, 10)],
        "released",
    );
    circuit.geometry.components[1].pins[2].position = point(20, 0);
    let run = Run::new(circuit);
    assert_eq!(run.contactor(), (RelayContactState::Released, None));
    assert!((run.value(&format!("{QA}/nc/current")) - 12.0 / 100.1).abs() < 1e-9);
    assert_eq!(run.value(&format!("{QA}/no/current")), 0.0);
}

#[test]
fn a_contactor_cannot_be_operated() {
    let mut run = Run::new(start_stop(true, "operate"));
    let request = run.operate_request(QA, OperatorPosition::Closed);
    let SetOperatorStateResult::Failure { diagnostics } = run.engine.set_operator_state(request)
    else {
        panic!("a contactor has no operator position");
    };
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
}

#[test]
fn the_cross_parameter_rule_and_domains_are_checked() {
    for (pickup, dropout) in [(0.04, 0.04), (0.04, 0.05), (0.04, -0.01)] {
        let mut circuit = start_stop(true, "hysteresis");
        let ComponentSpec::Contactor { parameters, .. } = &mut circuit.schematic.components[3]
        else {
            panic!("the contactor is the fourth component");
        };
        parameters.pickup_current_amps = pickup;
        parameters.dropout_current_amps = dropout;
        let CompileResult::Failure { diagnostics } = Engine::default().compile(circuit) else {
            panic!("pickup {pickup} and dropout {dropout} should be refused");
        };
        assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidParameter);
    }
    let mut circuit = start_stop(true, "coil");
    let ComponentSpec::Contactor { parameters, .. } = &mut circuit.schematic.components[3] else {
        panic!("the contactor is the fourth component");
    };
    parameters.coil_resistance_ohms = 0.0;
    assert!(matches!(
        Engine::default().compile(circuit),
        CompileResult::Failure { .. }
    ));
}

#[test]
fn a_fault_on_a_contactor_is_refused() {
    for kind in [FaultKind::Open, FaultKind::Short] {
        let mut circuit = start_stop(true, "fault");
        circuit.faults = Some(vec![FaultSpec {
            id: id("00000000-0000-4000-8000-0000000000f1"),
            kind,
            component_id: id(QA),
            parameter_id: None,
            value: None,
        }]);
        let CompileResult::Failure { diagnostics } = Engine::default().compile(circuit) else {
            panic!("a {kind:?} fault on a contactor should be refused");
        };
        assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidFault);
    }
}

#[test]
fn the_closed_contact_resistance_is_a_live_parameter() {
    let mut run = Run::new(start_stop(true, "live"));
    run.advance_to(5);
    run.operate(START, OperatorPosition::Pressed);
    run.advance_to(12);
    run.operate(START, OperatorPosition::Released);
    assert_eq!(run.contactor().0, RelayContactState::Picked);
    run.set_parameter(QA, ParameterId::ClosedContactResistanceOhms, 1.0);
    // 5 V over the STOP button, the 1 Ohm seal-in contact, and the 100 Ohm coil.
    assert!((run.coil_current() - 5.0 / 101.01).abs() < 1e-6);
    assert_eq!(run.contactor().0, RelayContactState::Picked);
}

#[test]
fn a_coil_that_switches_itself_is_deterministic_and_never_panics() {
    // The contactor's own normally closed contact feeds its coil, which the contract leaves
    // undefined. Whatever the Engine does, it must do the same thing every time and stay finite.
    let ground = point(0, 10);
    let (p0, p1) = (point(0, 0), point(4, 0));
    let build = || {
        request(
            vec![
                source(SOURCE, 5.0, p0, ground),
                contactor(0.01, (p1, ground), (point(30, 0), point(32, 0)), (p0, p1)),
            ],
            &[ground],
            "self-switching",
        )
    };
    let play = || {
        let mut engine = Engine::default();
        match engine.compile(build()) {
            CompileResult::Failure { diagnostics } => Err(diagnostics),
            CompileResult::Success {
                run_id,
                initial_snapshot,
                ..
            } => {
                let mut snapshots = vec![initial_snapshot];
                for _ in 0..40 {
                    match engine.advance(AdvanceRequest {
                        run_id: run_id.clone(),
                        step_count: 1,
                    }) {
                        Ok(snapshot) => snapshots.push(snapshot),
                        Err(diagnostics) => return Err(diagnostics),
                    }
                }
                for snapshot in &snapshots {
                    assert!(snapshot.values.iter().all(|value| value.is_finite()));
                }
                // The run ID is random; everything else must repeat.
                Ok(snapshots
                    .into_iter()
                    .map(|snapshot| {
                        (
                            snapshot.metadata.simulation_step,
                            snapshot
                                .values
                                .iter()
                                .map(|value| value.to_bits())
                                .collect::<Vec<_>>(),
                            snapshot.states,
                        )
                    })
                    .collect::<Vec<_>>())
            }
        }
    };
    assert_eq!(play(), play());
}

#[test]
fn the_catalog_describes_the_contactor() {
    let catalog = component_catalog();
    let entry = catalog
        .component_types
        .iter()
        .find(|entry| serde_json::to_value(entry.type_id).unwrap() == "contactor")
        .expect("contactor is in the catalog");
    let pins = entry
        .pins
        .iter()
        .map(|pin| pin.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(pins, ["a1", "a2", "no_1", "no_2", "nc_1", "nc_2"]);
    let roles = entry
        .pins
        .iter()
        .map(|pin| serde_json::to_value(pin.role).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        [
            "positive",
            "negative",
            "unpolarized",
            "unpolarized",
            "unpolarized",
            "unpolarized"
        ]
    );
    let parameters = entry
        .parameters
        .iter()
        .map(|parameter| {
            (
                serde_json::to_value(parameter.id)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
                parameter.default_value,
                parameter.description_key.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        parameters,
        [
            (
                "coilResistanceOhms".to_owned(),
                100.0,
                "component.contactor.coilResistance".to_owned()
            ),
            (
                "coilInductanceHenries".to_owned(),
                0.01,
                "component.contactor.coilInductance".to_owned()
            ),
            (
                "pickupCurrentAmps".to_owned(),
                0.04,
                "component.contactor.pickupCurrent".to_owned()
            ),
            (
                "dropoutCurrentAmps".to_owned(),
                0.02,
                "component.contactor.dropoutCurrent".to_owned()
            ),
            (
                "closedContactResistanceOhms".to_owned(),
                0.1,
                "component.contactor.closedContactResistance".to_owned()
            ),
        ]
    );
    assert_eq!(entry.display_name_key, "component.contactor.name");
    assert!(entry.fault_kinds.is_empty());
    assert!(entry.parameters.iter().all(|parameter| !parameter.optional));
}

#[test]
fn the_saved_form_is_a_typed_component_with_the_relay_parameters() {
    let circuit = start_stop(true, "saved");
    let saved = serde_json::to_value(&circuit.schematic).unwrap();
    let contactor = &saved["components"][3];
    assert_eq!(contactor["typeId"], "contactor");
    assert_eq!(
        contactor["parameters"],
        serde_json::json!({
            "coilResistanceOhms": 100.0,
            "coilInductanceHenries": 0.01,
            "pickupCurrentAmps": 0.04,
            "dropoutCurrentAmps": 0.02,
            "closedContactResistanceOhms": 0.1,
        })
    );
    let parsed: Schematic = serde_json::from_value(saved).unwrap();
    assert_eq!(parsed, circuit.schematic);
}

#[test]
fn the_snapshot_orders_every_kind_of_state_by_component_id() {
    let run = Run::new(start_stop(true, "order"));
    let ids = run
        .snapshot
        .states
        .iter()
        .map(|state| state.component_id().as_str().to_owned())
        .collect::<Vec<_>>();
    // STOP, START, then the contactor: a push button, a push button, and a contactor.
    assert_eq!(ids, [STOP, START, QA]);
    assert!(matches!(
        run.snapshot.states[2],
        DeviceState::Contactor { .. }
    ));
}
