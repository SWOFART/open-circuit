use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, Engine, OperatorPosition,
    ParameterId, ResultLayout, ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult,
    SetParameterRequest, SetParameterResult, StableId,
};
use serde_json::{Value, json};

fn id(n: usize) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

type Part<'a> = (&'a str, Value, &'a [(&'a str, i32)]);

fn request(parts: &[Part<'_>]) -> CompileRequest {
    let components: Vec<_> = parts
        .iter()
        .enumerate()
        .map(|(index, (kind, parameters, _))| {
            json!({"id": id(index), "typeId": kind, "position": {"x": index, "y": 2},
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
            json!({"componentId": id(index), "pins": pins})
        })
        .collect();
    serde_json::from_value(json!({
        "documentRevision": "base", "geometry": {"components": geometry},
        "schematic": {"schemaVersion": 2, "components": components, "wires": [],
            "rails": [], "junctions": [], "referenceMarkers": [
                {"id": id(parts.len()), "position": {"x": 0, "y": 0}}
            ]}
    }))
    .unwrap()
}

struct Run {
    engine: Engine,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
    rev: usize,
}

impl Run {
    fn compile(request: CompileRequest) -> Self {
        let mut engine = Engine::default();
        let result = engine.compile(request);
        let CompileResult::Success {
            layout,
            initial_snapshot,
            ..
        } = result
        else {
            panic!("{result:?}");
        };
        Self {
            engine,
            layout,
            snapshot: initial_snapshot,
            rev: 0,
        }
    }

    fn advance(&mut self, steps: u32) {
        self.snapshot = self
            .engine
            .advance(AdvanceRequest {
                run_id: self.snapshot.metadata.run_id.clone(),
                step_count: steps,
            })
            .unwrap();
    }

    fn value(&self, part: usize, path: &str) -> f64 {
        let key = format!("{}/{path}", id(part));
        let offset = self
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == key)
            .unwrap()
            .offset;
        self.snapshot.values[offset as usize]
    }

    fn set(&mut self, part: usize, parameter_id: ParameterId, value: f64) {
        self.rev += 1;
        let result = self.engine.set_parameter(SetParameterRequest {
            run_id: self.snapshot.metadata.run_id.clone(),
            expected_document_revision: self.snapshot.metadata.document_revision.clone(),
            document_revision: format!("rev-{}", self.rev),
            component_id: StableId::parse(id(part)).unwrap(),
            parameter_id,
            value,
        });
        let SetParameterResult::Success { snapshot, .. } = result else {
            panic!("{result:?}");
        };
        self.snapshot = snapshot;
    }

    fn operate(&mut self, part: usize, position: OperatorPosition) -> SetOperatorStateResult {
        self.rev += 1;
        let result = self.engine.set_operator_state(SetOperatorStateRequest {
            run_id: self.snapshot.metadata.run_id.clone(),
            expected_document_revision: self.snapshot.metadata.document_revision.clone(),
            document_revision: format!("rev-{}", self.rev),
            component_id: StableId::parse(id(part)).unwrap(),
            position,
        });
        if let SetOperatorStateResult::Success { snapshot, .. } = &result {
            self.snapshot = snapshot.clone();
        }
        result
    }
}

#[test]
fn ac_coil_changes_all_three_poles_and_auxiliaries_together() {
    for frequency in [50.0, 60.0] {
        check_ac_coil(frequency);
    }
}

fn check_ac_coil(frequency: f64) {
    let mut run = Run::compile(request(&[
        (
            "contactor_three_pole",
            json!({"coilResistanceOhms":10, "coilInductanceHenries":0.001,
            "closedContactResistanceOhms":0.01, "pickupCurrentAmps":0.7, "dropoutCurrentAmps":0.4,
            "coilSenseSeconds":0.02}),
            &[
                ("a1", 1),
                ("a2", 0),
                ("l1", 2),
                ("t1", 3),
                ("l2", 2),
                ("t2", 4),
                ("l3", 2),
                ("t3", 5),
                ("no_1", 2),
                ("no_2", 6),
                ("nc_1", 2),
                ("nc_2", 7),
            ],
        ),
        (
            "ac_voltage_source",
            json!({"voltageRmsVolts":10, "frequencyHz":frequency, "phaseOffsetDegrees":0,
            "internalResistanceOhms":0}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts":12, "internalResistanceOhms":0}),
            &[("pin1", 2), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 3), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 4), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 5), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 6), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 7), ("pin2", 0)],
        ),
    ]));
    let assert_poles = |run: &Run, engaged: bool| {
        let expected = 12.0 / 10.01;
        for path in [
            "pole1/current",
            "pole2/current",
            "pole3/current",
            "no/current",
        ] {
            assert!(
                (run.value(0, path) - if engaged { expected } else { 0.0 }).abs() < 1e-10,
                "{path}"
            );
        }
        assert!((run.value(0, "nc/current") - if engaged { 0.0 } else { expected }).abs() < 1e-10);
    };
    assert_poles(&run, false);
    let mut detector = 0.0;
    let mut reference_transition = [None; 2];
    let mut actual_transition = [None; 2];
    // Use the closed-form RL current and integrate the detector with
    // midpoint quadrature on a 10 µs grid independent of the Engine.
    let omega = std::f64::consts::TAU * frequency;
    let impedance = 10.0_f64.hypot(omega * 0.001);
    let phase = (omega * 0.001 / 10.0).atan();
    let sinusoid = |time: f64| (omega * time - phase).sin() * 2.0_f64.sqrt() / impedance;
    let dt = 1e-5_f64;
    let decay = (-dt / 0.02).exp();
    for step in 1..=750 {
        if step == 501 {
            run.set(1, ParameterId::VoltageRmsVolts, 1.0);
        }
        let dropping = step > 500;
        let transition = usize::from(dropping);
        let (start, rms, transient) = if dropping {
            (0.5, 1.0, 9.0 * sinusoid(0.5))
        } else {
            (0.0, 10.0, -10.0 * sinusoid(0.0))
        };
        for tick in (step - 1) * 100..step * 100 {
            let time = (f64::from(tick) + 0.5) * dt;
            let current = rms * sinusoid(time) + transient * (-(time - start) / 0.0001).exp();
            detector = decay * detector + (1.0 - decay) * current.powi(2);
            if (!dropping && detector >= 0.7_f64.powi(2))
                || (dropping && detector <= 0.4_f64.powi(2))
            {
                reference_transition[transition].get_or_insert(time + dt / 2.0);
            }
        }
        run.advance(1);
        let picked = run.value(0, "pole1/current") > 0.0;
        let time = f64::from(step) * 0.001;
        if picked != dropping {
            actual_transition[transition].get_or_insert(time);
        }
        assert_poles(
            &run,
            actual_transition[0].is_some() && actual_transition[1].is_none(),
        );
        if (401..=500).contains(&step) || (651..=750).contains(&step) {
            let actual = run.value(0, "coil/current_squared");
            assert!(
                (actual - detector).abs() <= 0.005 * detector,
                "{frequency} Hz at {time}s: expected {detector}, got {actual}"
            );
        }
    }
    for (index, name) in ["pickup", "dropout"].into_iter().enumerate() {
        let actual = actual_transition[index].unwrap();
        let expected = reference_transition[index].unwrap();
        assert!(
            (actual - expected).abs() <= 0.002,
            "{frequency} Hz {name}: expected {expected}s, got {actual}s"
        );
    }
}

#[test]
fn breaker_closes_trips_and_requires_explicit_open_to_reset() {
    let mut run = Run::compile(request(&[
        (
            "breaker_three_pole",
            json!({"closedContactResistanceOhms":0.01,
            "instantaneousTripAmps":2}),
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
            json!({"voltageVolts":6, "internalResistanceOhms":0}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":20}),
            &[("pin1", 2), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 3), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":5}),
            &[("pin1", 4), ("pin2", 0)],
        ),
    ]));
    run.advance(5);
    assert!(matches!(
        run.operate(0, OperatorPosition::Closed),
        SetOperatorStateResult::Success { .. }
    ));
    assert_eq!(run.snapshot.metadata.simulation_step, 5);
    for (index, resistance) in [20.0, 10.0, 5.0].into_iter().enumerate() {
        assert!(
            (run.value(0, &format!("pole{}/current", index + 1)) - 6.0 / (resistance + 0.01)).abs()
                < 1e-10
        );
    }
    run.set(1, ParameterId::VoltageVolts, 12.0);
    assert_eq!(run.value(0, "tripped"), 1.0);
    for pole in 1..=3 {
        assert_eq!(run.value(0, &format!("pole{pole}/current")), 0.0);
    }
    let rev = run.snapshot.metadata.document_revision.clone();
    assert!(matches!(run.operate(0, OperatorPosition::Closed),
        SetOperatorStateResult::Failure { diagnostics } if diagnostics[0].code == DiagnosticCode::InvalidParameter));
    run.advance(1);
    assert_eq!(run.snapshot.metadata.document_revision, rev);
    assert_eq!(run.value(0, "tripped"), 1.0);
    assert!(matches!(
        run.operate(0, OperatorPosition::Open),
        SetOperatorStateResult::Success { .. }
    ));
    assert_eq!(run.value(0, "tripped"), 0.0);
    assert!(matches!(
        run.operate(0, OperatorPosition::Closed),
        SetOperatorStateResult::Success { .. }
    ));
    assert_eq!(run.value(0, "tripped"), 1.0);
    run.set(0, ParameterId::InstantaneousTripAmps, 3.0);
    assert_eq!(run.value(0, "tripped"), 1.0);
}

#[test]
fn simultaneous_initial_trips_publish_only_the_settled_poles() {
    let breaker =
        json!({"closedContactResistanceOhms":0.01, "instantaneousTripAmps":1, "initialClosed":1});
    let circuit = request(&[
        (
            "breaker_three_pole",
            breaker.clone(),
            &[
                ("l1", 1),
                ("t1", 2),
                ("l2", 1),
                ("t2", 2),
                ("l3", 1),
                ("t3", 2),
            ],
        ),
        (
            "breaker_three_pole",
            breaker,
            &[
                ("l1", 1),
                ("t1", 3),
                ("l2", 1),
                ("t2", 3),
                ("l3", 1),
                ("t3", 3),
            ],
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts":12, "internalResistanceOhms":0}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":1}),
            &[("pin1", 2), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":1}),
            &[("pin1", 3), ("pin2", 0)],
        ),
    ]);
    let run = Run::compile(circuit.clone());
    for part in 0..=1 {
        assert_eq!(run.value(part, "tripped"), 1.0);
        for pole in 1..=3 {
            assert_eq!(run.value(part, &format!("pole{pole}/current")), 0.0);
        }
    }
    assert!(run.snapshot.states.iter().all(|state| matches!(
        state,
        open_circuit_engine::DeviceState::Switch {
            conducting: false,
            last_transition_time_seconds: Some(0.0),
            ..
        }
    )));
    let replay = Run::compile(circuit.clone());
    assert_eq!(replay.snapshot.values, run.snapshot.values);
    assert_eq!(replay.snapshot.states, run.snapshot.states);

    let mut invalid = serde_json::to_value(circuit).unwrap();
    invalid["schematic"]["components"][0]["parameters"]["closedContactResistanceOhms"] = json!(0);
    let invalid = serde_json::from_value(invalid).unwrap();
    let CompileResult::Failure { diagnostics } = Engine::default().compile(invalid) else {
        panic!("zero closed resistance must be refused");
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidParameter
            && diagnostic.references.contains(
                &open_circuit_engine::DiagnosticReference::Component {
                    component_id: StableId::parse(id(0)).unwrap(),
                },
            )
    }));
}

#[test]
fn an_explicit_capacitor_voltage_does_not_reinitialize_the_coil_current() {
    let circuit = request(&[
        (
            "contactor_three_pole",
            json!({"coilResistanceOhms":10, "coilInductanceHenries":0.1,
            "closedContactResistanceOhms":0.01, "pickupCurrentAmps":0.7, "dropoutCurrentAmps":0.4,
            "coilSenseSeconds":0.02}),
            &[
                ("a1", 2),
                ("a2", 0),
                ("l1", 0),
                ("t1", 0),
                ("l2", 0),
                ("t2", 0),
                ("l3", 0),
                ("t3", 0),
                ("no_1", 0),
                ("no_2", 0),
                ("nc_1", 0),
                ("nc_2", 0),
            ],
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts":20, "internalResistanceOhms":0}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 1), ("pin2", 2)],
        ),
        (
            "capacitor",
            json!({"capacitanceFarads":0.001,"initialVoltageVolts":0}),
            &[("pin1", 2), ("pin2", 0)],
        ),
    ]);
    let run = Run::compile(circuit.clone());
    assert_eq!(run.value(0, "coil/voltage"), 0.0);
    assert!((run.value(0, "coil/current") - 1.0).abs() < 1e-12);
    assert!((run.value(0, "coil/current_squared") - 1.0).abs() < 1e-12);
    assert!(matches!(
        run.snapshot.states.as_slice(),
        [open_circuit_engine::DeviceState::Contactor {
            state: open_circuit_engine::RelayContactState::Picked,
            ..
        }]
    ));

    let mut incompatible = circuit;
    incompatible.geometry.components[3].pins[0].position.x = 1;
    let CompileResult::Failure { diagnostics } = Engine::default().compile(incompatible) else {
        panic!("an explicit zero volt capacitor cannot share the ideal 20 V source");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InconsistentState)
    );
}
