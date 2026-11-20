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
        "schematic": {"schemaVersion": 3, "documentation": {"designators":[],"wireNumbers":[],"terminals":[],"notes":[],"titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},"sheets":[]}, "components": components, "wires": [],
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

fn overload_request(source_type: &str, phases: [Value; 3]) -> CompileRequest {
    request(&[
        (
            "thermal_overload_three_pole",
            json!({"closedContactResistanceOhms":0.01, "senseResistanceOhms":1,
            "ratedCurrentAmps":10, "thermalSenseSeconds":0.02, "tripScaleSeconds":0.1,
            "coolSeconds":0.1, "resetDose":0.25}),
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
        (source_type, phases[0].clone(), &[("pin1", 1), ("pin2", 0)]),
        (source_type, phases[1].clone(), &[("pin1", 2), ("pin2", 0)]),
        (source_type, phases[2].clone(), &[("pin1", 3), ("pin2", 0)]),
        (
            "dc_voltage_source",
            json!({"voltageVolts":12, "internalResistanceOhms":0}),
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
    ])
}

fn assert_overload_auxiliaries(run: &Run, tripped: bool) {
    assert_eq!(run.value(0, "no/current") > 0.0, tripped);
    assert_eq!(run.value(0, "nc/current") > 0.0, !tripped);
}

#[test]
fn overload_reset_dose_accepts_both_clamped_boundaries() {
    let fixture = overload_request(
        "dc_voltage_source",
        std::array::from_fn(|_| json!({"voltageVolts":0, "internalResistanceOhms":0})),
    );
    for (dose, valid) in [(0.0, true), (0.99, true), (-0.01, false), (1.0, false)] {
        let mut candidate = fixture.clone();
        let open_circuit_engine::ComponentSpec::ThermalOverloadThreePole { parameters, .. } =
            &mut candidate.schematic.components[0]
        else {
            unreachable!();
        };
        parameters.reset_dose = dose;
        match Engine::default().compile(candidate) {
            CompileResult::Success { .. } if valid => {}
            CompileResult::Failure { diagnostics } if !valid => {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidParameter)
                );
            }
            result => panic!("reset dose {dose}: {result:?}"),
        }
    }
}

#[test]
fn dc_overload_trips_at_the_detector_dose_reference_and_resets_after_cooling() {
    for multiple in [1.0_f64, 1.5, 2.0, 3.0] {
        let fixture = overload_request(
            "dc_voltage_source",
            [2.0, -10.0 * multiple, 3.0]
                .map(|voltage| json!({"voltageVolts":voltage, "internalResistanceOhms":0})),
        );
        let mut run = Run::compile(fixture.clone());
        let q_final = multiple.powi(2);
        let threshold = -0.02 * (1.0 - 1.0 / q_final).ln();
        // Integral of q(t)-1 from the instant the detector exceeds rated current.
        let dose = |t: f64| {
            ((q_final - 1.0) * (t - threshold)
                + q_final * 0.02 * ((-t / 0.02).exp() - (-threshold / 0.02).exp()))
                / 0.1
        };
        let mut trip = None;
        for step in 1..=200 {
            run.advance(1);
            if run.value(0, "thermal_dose") == 1.0 {
                trip = Some(f64::from(step) * 0.001);
                break;
            }
            assert_overload_auxiliaries(&run, false);
        }
        if multiple == 1.0 {
            assert!(trip.is_none());
            assert_eq!(run.value(0, "thermal_dose"), 0.0);
            continue;
        }
        let trip = trip.unwrap();
        let (mut below, mut above) = (threshold, 0.2);
        for _ in 0..40 {
            let middle = (below + above) / 2.0;
            if dose(middle) < 1.0 {
                below = middle;
            } else {
                above = middle;
            }
        }
        assert!(
            (trip - above).abs() <= 0.002_f64.max(0.01 * above),
            "{multiple}×: {trip}s vs {above}s"
        );
        assert_overload_auxiliaries(&run, true);
        assert!((run.value(0, "pole2/current") + 10.0 * multiple).abs() <= 1e-10);
        if multiple != 2.0 {
            continue;
        }
        let before = run.snapshot.clone();
        assert!(matches!(
            run.operate(0, OperatorPosition::Pressed),
            SetOperatorStateResult::Failure { .. }
        ));
        assert_eq!(run.snapshot, before);
        let refused = run.engine.set_parameter(SetParameterRequest {
            run_id: before.metadata.run_id.clone(),
            expected_document_revision: before.metadata.document_revision.clone(),
            document_revision: "refused".to_owned(),
            component_id: StableId::parse(id(0)).unwrap(),
            parameter_id: ParameterId::RatedCurrentAmps,
            value: 50.0,
        });
        assert!(matches!(refused, SetParameterResult::Failure { .. }));
        let mut control = Run::compile(fixture);
        control.advance(before.metadata.simulation_step.try_into().unwrap());
        run.set(2, ParameterId::VoltageVolts, 0.0);
        control.set(2, ParameterId::VoltageVolts, 0.0);
        run.advance(250);
        control.advance(250);
        assert_eq!(run.snapshot.values, control.snapshot.values);
        assert_eq!(run.snapshot.states, control.snapshot.states);
        let cooled = run.value(0, "thermal_dose");
        assert!(cooled > 0.0 && cooled <= 0.25);
        run.advance(1);
        let after_cooling = run.value(0, "thermal_dose");
        assert!((after_cooling - cooled * (-0.001_f64 / 0.1).exp()).abs() < 1e-12);
        let previous_step = run.snapshot.metadata.simulation_step;
        assert!(matches!(
            run.operate(0, OperatorPosition::Pressed),
            SetOperatorStateResult::Success { .. }
        ));
        assert_eq!(run.snapshot.metadata.simulation_step, previous_step);
        assert_eq!(run.value(0, "thermal_dose"), after_cooling);
        assert_overload_auxiliaries(&run, false);
        run.set(4, ParameterId::VoltageVolts, 11.0);
        assert_eq!(run.snapshot.metadata.simulation_step, previous_step);
        assert_eq!(run.value(0, "thermal_dose"), after_cooling);
        assert_overload_auxiliaries(&run, false);
    }
}

#[test]
fn balanced_ac_overload_uses_the_maximum_phase_before_filtering() {
    let phases = [0.0_f64, 120.0, 240.0];
    let mut run = Run::compile(overload_request("ac_voltage_source", phases.map(|phase| {
        json!({"voltageRmsVolts":15,"frequencyHz":50,"phaseOffsetDegrees":phase,"internalResistanceOhms":0})
    })));
    let dt = 1e-5_f64;
    let decay = (-dt / 0.02).exp();
    let mut detector = 0.0;
    let mut dose = 0.0;
    let mut expected = None;
    for tick in 0..20_000 {
        let time = (f64::from(tick) + 0.5) * dt;
        let q = phases
            .into_iter()
            .map(|phase| {
                (std::f64::consts::TAU * 50.0 * time + phase.to_radians())
                    .sin()
                    .powi(2)
                    * 4.5
            })
            .fold(0.0_f64, f64::max);
        detector = decay * detector + (1.0 - decay) * q;
        dose += dt * (detector - 1.0).max(0.0) / 0.1;
        if dose >= 1.0 {
            expected = Some(time);
            break;
        }
    }
    for step in 1..=200 {
        run.advance(1);
        if run.value(0, "thermal_dose") == 1.0 {
            let expected = expected.unwrap();
            let actual = f64::from(step) * 0.001;
            assert!(
                (actual - expected).abs() <= 0.002_f64.max(0.01 * expected),
                "{actual}s vs {expected}s"
            );
            assert_overload_auxiliaries(&run, true);
            return;
        }
    }
    panic!("balanced AC did not trip");
}

fn induction_request(delta: bool) -> CompileRequest {
    let windings = if delta {
        [
            ("u1", 1),
            ("u2", 2),
            ("v1", 2),
            ("v2", 3),
            ("w1", 3),
            ("w2", 1),
        ]
    } else {
        [
            ("u1", 1),
            ("u2", 0),
            ("v1", 2),
            ("v2", 0),
            ("w1", 3),
            ("w2", 0),
        ]
    };
    request(&[
        (
            "induction_motor_three_phase",
            json!({"statorResistanceOhms":1,"rotorResistanceOhms":1,
              "statorLeakageHenries":0.005,"rotorLeakageHenries":0.005,"magnetizingHenries":0.1,
              "polePairs":2,"rotorInertiaKilogramMetersSquared":0.002,
              "viscousFrictionNewtonMeterSecondsPerRadian":0.01,"loadTorqueNewtonMeters":0.1}),
            &windings,
        ),
        (
            "three_phase_source_star",
            json!({"phaseVoltageRmsVolts":48,"frequencyHz":50,"phaseOffsetDegrees":0,"internalResistanceOhms":0,"reversedSequence":0}),
            &[("l1", 1), ("l2", 2), ("l3", 3), ("n", 0)],
        ),
    ])
}

#[test]
fn induction_motor_starts_from_zero_and_accelerates_in_star_and_delta() {
    for delta in [false, true] {
        let mut run = Run::compile(induction_request(delta));
        for key in [
            "u/current",
            "v/current",
            "w/current",
            "angular_velocity",
            "electromagnetic_torque",
        ] {
            assert_eq!(run.value(0, key), 0.0);
        }
        let mut reference = MotorReference {
            state: [0.0; 5],
            delta,
        };
        let mut samples = Vec::new();
        for step in 1..=180 {
            reference.advance(step);
            run.advance(1);
            let speed = run.value(0, "angular_velocity");
            let torque = run.value(0, "electromagnetic_torque");
            let current = run.value(0, "u/current");
            let [is_alpha, _, _, _, expected_speed] = reference.state;
            let expected_torque = MotorReference::torque(reference.state);
            assert!((speed - expected_speed).abs() <= 0.5_f64.max(0.02 * expected_speed.abs()));
            assert!((torque - expected_torque).abs() <= 0.05_f64.max(0.03 * expected_torque.abs()));
            samples.push((speed, expected_speed, current, is_alpha));
        }
        for cycle in samples.as_chunks::<20>().0 {
            let actual = cycle
                .iter()
                .map(|sample| sample.2.powi(2))
                .sum::<f64>()
                .sqrt();
            let expected = cycle
                .iter()
                .map(|sample| sample.3.powi(2))
                .sum::<f64>()
                .sqrt();
            assert!((actual - expected).abs() <= 0.02 * expected);
        }
        let [settled, expected_settled] = [samples.last().unwrap().0, samples.last().unwrap().1];
        assert!(
            settled > 100.0 && settled < 157.1,
            "delta={delta}, speed={settled}"
        );
        let actual_90 = samples
            .iter()
            .position(|sample| sample.0 >= 0.9 * expected_settled)
            .unwrap();
        let expected_90 = samples
            .iter()
            .position(|sample| sample.1 >= 0.9 * expected_settled)
            .unwrap();
        assert!(actual_90.abs_diff(expected_90) as f64 <= 5.0_f64.max(0.05 * expected_90 as f64));
        let before = run.value(0, "angular_velocity");
        run.set(0, ParameterId::LoadTorqueNewtonMeters, 0.2);
        assert!(run.value(0, "angular_velocity") < before);
    }
}

#[test]
fn induction_motor_validates_integer_poles_and_refuses_compile_only_edits() {
    for (pole_pairs, valid) in [
        (1.0, true),
        (8.0, true),
        (0.0, false),
        (1.5, false),
        (9.0, false),
    ] {
        let mut fixture = induction_request(false);
        let open_circuit_engine::ComponentSpec::InductionMotorThreePhase { parameters, .. } =
            &mut fixture.schematic.components[0]
        else {
            unreachable!()
        };
        parameters.pole_pairs = pole_pairs;
        match Engine::default().compile(fixture) {
            CompileResult::Success { .. } if valid => {}
            CompileResult::Failure { diagnostics } if !valid => {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidParameter)
                );
            }
            result => panic!("pole pairs {pole_pairs}: {result:?}"),
        }
    }

    let mut actual = Run::compile(induction_request(false));
    let mut control = Run::compile(induction_request(false));
    actual.advance(1);
    control.advance(1);
    let refused = actual.engine.set_parameter(SetParameterRequest {
        run_id: actual.snapshot.metadata.run_id.clone(),
        expected_document_revision: actual.snapshot.metadata.document_revision.clone(),
        document_revision: "refused".to_owned(),
        component_id: StableId::parse(id(0)).unwrap(),
        parameter_id: ParameterId::PolePairs,
        value: 3.0,
    });
    assert!(matches!(refused, SetParameterResult::Failure { .. }));
    actual.set(0, ParameterId::LoadTorqueNewtonMeters, 0.2);
    control.set(0, ParameterId::LoadTorqueNewtonMeters, 0.2);
    actual.advance(2);
    control.advance(2);
    assert_eq!(actual.snapshot.values, control.snapshot.values);
}

fn switched_motor_request(shunt: bool) -> CompileRequest {
    let mut fixture = induction_request(false);
    for (pin, node) in [(0, 4), (2, 5), (4, 6)] {
        fixture.geometry.components[0].pins[pin].position.x = node;
    }
    let switchgear = request(&[
        (
            "breaker_three_pole",
            json!({"closedContactResistanceOhms":0.01,
            "instantaneousTripAmps":1000,"initialClosed":1}),
            &[
                ("l1", 1),
                ("t1", 4),
                ("l2", 2),
                ("t2", 5),
                ("l3", 3),
                ("t3", 6),
            ],
        ),
        (
            "resistor",
            json!({"resistanceOhms":5}),
            &[("pin1", 4), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":5}),
            &[("pin1", 5), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":5}),
            &[("pin1", 6), ("pin2", 0)],
        ),
    ]);
    for (index, (component, mut geometry)) in switchgear
        .schematic
        .components
        .into_iter()
        .zip(switchgear.geometry.components)
        .enumerate()
    {
        if index > 0 && !shunt {
            break;
        }
        let new_id = id(index + 3);
        let mut value = serde_json::to_value(component).unwrap();
        value["id"] = json!(new_id);
        geometry.component_id = StableId::parse(new_id).unwrap();
        fixture
            .schematic
            .components
            .push(serde_json::from_value(value).unwrap());
        fixture.geometry.components.push(geometry);
    }
    fixture
}

#[test]
fn induction_motor_switching_preserves_currents_and_rejects_an_open_cut() {
    for shunt in [false, true] {
        let fixture = switched_motor_request(shunt);
        let CompileResult::Success {
            diagnostics,
            internal_substeps,
            ..
        } = Engine::default().compile(fixture.clone())
        else {
            panic!("motor and breaker must compile");
        };
        assert_eq!(internal_substeps, 64);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::StepAccuracyLimited)
        );
        let mut run = Run::compile(fixture.clone());
        run.advance(12);
        let before = run.snapshot.clone();
        let paths = [
            "u/current",
            "v/current",
            "w/current",
            "angular_velocity",
            "electromagnetic_torque",
        ];
        let before_state = paths.map(|path| run.value(0, path));
        assert!(before_state[0].abs() > 0.1);
        let action = run.operate(3, OperatorPosition::Open);
        if !shunt {
            let SetOperatorStateResult::Failure { diagnostics } = action else {
                panic!("opening an energized winding without a return path must fail");
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == DiagnosticCode::InconsistentState)
            );
            let mut control = Run::compile(fixture);
            control.advance(13);
            run.advance(1);
            assert_eq!(run.snapshot.values, control.snapshot.values);
            assert_eq!(run.snapshot.states, control.snapshot.states);
            assert_eq!(
                run.snapshot.metadata.document_revision,
                before.metadata.document_revision
            );
            continue;
        }
        assert!(
            matches!(action, SetOperatorStateResult::Success { .. }),
            "{action:?}"
        );
        assert_eq!(
            run.snapshot.metadata.simulation_time_seconds,
            before.metadata.simulation_time_seconds
        );
        assert_eq!(paths.map(|path| run.value(0, path)), before_state);
        for (current, voltage) in
            before_state[..3]
                .iter()
                .zip(["u/voltage", "v/voltage", "w/voltage"])
        {
            assert!((run.value(0, voltage) + 5.0 * current).abs() < 1e-9);
        }
        run.advance(1);
        assert!(run.value(0, "u/current").abs() < before_state[0].abs());
        assert_eq!(run.value(3, "pole1/current"), 0.0);
    }
}

// Independent current-coordinate, amplitude-invariant model. Its fixed
// 10 µs RK4 grid is separate from the Engine's implicit flux equations.
struct MotorReference {
    state: [f64; 5],
    delta: bool,
}

impl MotorReference {
    fn torque([sa, sb, ra, rb, _]: [f64; 5]) -> f64 {
        0.3 * (ra * sb - rb * sa)
    }

    fn derivative(&self, time: f64, state: [f64; 5]) -> [f64; 5] {
        let [sa, sb, ra, rb, speed] = state;
        let angle = std::f64::consts::TAU * 50.0 * time;
        let line = |thirds: f64| {
            48.0 * 2.0_f64.sqrt() * (angle + thirds * std::f64::consts::TAU / 3.0).sin()
        };
        let (u, v, w) = (line(0.0), line(-1.0), line(1.0));
        let phase = if self.delta {
            [u - v, v - w, w - u]
        } else {
            [u, v, w]
        };
        let voltage = [phase[0], (phase[1] - phase[2]) / 3.0_f64.sqrt()];
        let electrical_speed = 2.0 * speed;
        let stator = [voltage[0] - sa, voltage[1] - sb];
        let rotor = [
            -ra - electrical_speed * (0.1 * sb + 0.105 * rb),
            -rb + electrical_speed * (0.1 * sa + 0.105 * ra),
        ];
        let det = 0.105_f64.powi(2) - 0.1_f64.powi(2);
        [
            (0.105 * stator[0] - 0.1 * rotor[0]) / det,
            (0.105 * stator[1] - 0.1 * rotor[1]) / det,
            (0.105 * rotor[0] - 0.1 * stator[0]) / det,
            (0.105 * rotor[1] - 0.1 * stator[1]) / det,
            (Self::torque(state) - 0.01 * speed - 0.1) / 0.002,
        ]
    }

    fn advance(&mut self, public_step: u32) {
        let dt = 1e-5;
        let shifted = |state: [f64; 5], slope: [f64; 5], duration| {
            std::array::from_fn(|i| state[i] + slope[i] * duration)
        };
        for tick in (public_step - 1) * 100..public_step * 100 {
            let t = f64::from(tick) * dt;
            let k1 = self.derivative(t, self.state);
            let k2 = self.derivative(t + dt / 2.0, shifted(self.state, k1, dt / 2.0));
            let k3 = self.derivative(t + dt / 2.0, shifted(self.state, k2, dt / 2.0));
            let k4 = self.derivative(t + dt, shifted(self.state, k3, dt));
            for i in 0..5 {
                self.state[i] += dt * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]) / 6.0;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Phasor(f64, f64);

impl Phasor {
    fn plus(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1)
    }
    fn minus(self, other: Self) -> Self {
        Self(self.0 - other.0, self.1 - other.1)
    }
    fn times(self, other: Self) -> Self {
        Self(
            self.0 * other.0 - self.1 * other.1,
            self.0 * other.1 + self.1 * other.0,
        )
    }
    fn over(self, other: Self) -> Self {
        let norm = other.0.powi(2) + other.1.powi(2);
        self.times(Self(other.0 / norm, -other.1 / norm))
    }
}

fn ac_source(frequency: f64) -> Value {
    json!({"phaseVoltageRmsVolts":48,"frequencyHz":frequency,"phaseOffsetDegrees":0,
        "internalResistanceOhms":0,"reversedSequence":0})
}

fn phase_phasors() -> [Phasor; 3] {
    [0.0, -1.0, 1.0].map(|thirds| {
        let theta = thirds * std::f64::consts::TAU / 3.0;
        Phasor(48.0 * theta.cos(), 48.0 * theta.sin())
    })
}

fn load_values(inductive: bool) -> Value {
    let scale = f64::from(inductive);
    json!({
        "phase1ResistanceOhms":10, "phase2ResistanceOhms":16, "phase3ResistanceOhms":24,
        "phase1InductanceHenries":0.005 * scale, "phase2InductanceHenries":0.009 * scale,
        "phase3InductanceHenries":0.015 * scale
    })
}

fn transformer_values() -> Value {
    json!({"turnsRatio":2,"primaryResistanceOhms":2,"secondaryResistanceOhms":1,
        "primaryLeakageHenries":0.004,"secondaryLeakageHenries":0.006,"magnetizingHenries":0.05})
}

fn compare_phasors(run: &mut Run, frequency: f64, slots: &[(usize, &str, Phasor)]) {
    // Fit sine and cosine coefficients over 100 ms, an integer number of
    // periods at both reference frequencies.
    let mut fits = vec![[0.0; 2]; slots.len()];
    for _ in 0..100 {
        run.advance(1);
        let angle =
            std::f64::consts::TAU * frequency * run.snapshot.metadata.simulation_time_seconds;
        for ((part, path, _), [re, im]) in slots.iter().zip(&mut fits) {
            let value = run.value(*part, path) / 2.0_f64.sqrt();
            *re += value * angle.sin() / 50.0;
            *im += value * angle.cos() / 50.0;
        }
    }
    for ((_, path, expected), [re, im]) in slots.iter().zip(fits) {
        let expected_rms = expected.0.hypot(expected.1);
        assert!(
            (re.hypot(im) - expected_rms).abs() <= (0.005 * expected_rms).max(1e-8),
            "{frequency} Hz {path}: actual ({re}, {im}), expected ({}, {})",
            expected.0,
            expected.1
        );
        if expected_rms > 1e-8 {
            let dot = re * expected.0 + im * expected.1;
            let cross = im * expected.0 - re * expected.1;
            assert!(
                cross.atan2(dot).abs().to_degrees() <= 0.3,
                "{frequency} Hz {path}: phase"
            );
        }
    }
}

#[test]
fn unbalanced_star_and_delta_loads_match_phasors_and_terminal_kcl() {
    for frequency in [50.0, 60.0] {
        let omega = std::f64::consts::TAU * frequency;
        let phases = phase_phasors();
        for (delta, floating, inductive) in [
            (false, false, true),
            (false, true, true),
            (true, false, false),
            (true, false, true),
        ] {
            let mut pins = vec![("l1", 1), ("l2", 2), ("l3", 3)];
            if !delta {
                pins.push(("n", if floating { 4 } else { 0 }));
            }
            let mut run = Run::compile(request(&[
                (
                    "three_phase_source_star",
                    ac_source(frequency),
                    &[("l1", 1), ("l2", 2), ("l3", 3), ("n", 0)],
                ),
                (
                    if delta {
                        "three_phase_load_delta"
                    } else {
                        "three_phase_load_star"
                    },
                    load_values(inductive),
                    &pins,
                ),
            ]));
            let z = [10.0, 16.0, 24.0]
                .into_iter()
                .zip([0.005, 0.009, 0.015])
                .map(|(r, l)| Phasor(r, omega * l * f64::from(inductive)))
                .collect::<Vec<_>>();
            let sum = |values: [Phasor; 3]| values.into_iter().fold(Phasor(0.0, 0.0), Phasor::plus);
            let neutral = if floating {
                sum(std::array::from_fn(|i| phases[i].over(z[i])))
                    .over(sum(std::array::from_fn(|i| Phasor(1.0, 0.0).over(z[i]))))
            } else {
                Phasor(0.0, 0.0)
            };
            let voltages: [Phasor; 3] = std::array::from_fn(|i| {
                phases[i].minus(if delta { phases[(i + 1) % 3] } else { neutral })
            });
            let currents: [Phasor; 3] = std::array::from_fn(|i| voltages[i].over(z[i]));
            let mut slots = Vec::new();
            let paths = if delta {
                [
                    ("l12/voltage", "l12/current"),
                    ("l23/voltage", "l23/current"),
                    ("l31/voltage", "l31/current"),
                ]
            } else {
                [
                    ("l1/voltage", "l1/current"),
                    ("l2/voltage", "l2/current"),
                    ("l3/voltage", "l3/current"),
                ]
            };
            for (i, (voltage, current)) in paths.into_iter().enumerate() {
                slots.push((1, voltage, voltages[i]));
                slots.push((1, current, currents[i]));
            }
            if delta {
                for (i, path) in ["l1/current", "l2/current", "l3/current"]
                    .into_iter()
                    .enumerate()
                {
                    slots.push((1, path, currents[i].minus(currents[(i + 2) % 3])));
                }
            } else {
                slots.push((1, "n/current", Phasor(0.0, 0.0).minus(sum(currents))));
            }
            run.advance(80);
            compare_phasors(&mut run, frequency, &slots);
        }
    }
}

#[test]
fn transformer_no_load_and_unbalanced_secondary_match_the_coupled_phasors() {
    for frequency in [50.0, 60.0] {
        for (loaded, linked_reference) in [(false, false), (true, false), (true, true)] {
            let mut parts: Vec<Part<'_>> = vec![
                (
                    "three_phase_source_star",
                    ac_source(frequency),
                    &[("l1", 1), ("l2", 2), ("l3", 3), ("n", 0)],
                ),
                (
                    "transformer_three_phase",
                    transformer_values(),
                    &[
                        ("p1a", 1),
                        ("p1b", 0),
                        ("p2a", 2),
                        ("p2b", 0),
                        ("p3a", 3),
                        ("p3b", 0),
                        ("s1a", 4),
                        ("s1b", 7),
                        ("s2a", 5),
                        ("s2b", 7),
                        ("s3a", 6),
                        ("s3b", 7),
                    ],
                ),
            ];
            let tied_secondary = [("l1", 4), ("l2", 5), ("l3", 6), ("n", 0)];
            let floating_secondary = [("l1", 4), ("l2", 5), ("l3", 6), ("n", 7)];
            if loaded {
                parts.push((
                    "three_phase_load_star",
                    load_values(true),
                    if linked_reference {
                        &tied_secondary
                    } else {
                        &floating_secondary
                    },
                ));
            }
            let mut fixture = request(&parts);
            if linked_reference {
                for pin in &mut fixture.geometry.components[1].pins {
                    if pin.position.x == 7 {
                        pin.position.x = 0;
                    }
                }
            }
            let mut run = Run::compile(fixture);
            let omega = std::f64::consts::TAU * frequency;
            let zp = Phasor(2.0, omega * 0.054);
            let zs = Phasor(1.0, omega * 0.0185);
            let zm = Phasor(0.0, omega * 0.025);
            let mut slots = Vec::new();
            let phases = phase_phasors();
            for (i, (primary, secondary, output)) in [
                (
                    "primary1/current",
                    "secondary1/current",
                    "secondary1/voltage",
                ),
                (
                    "primary2/current",
                    "secondary2/current",
                    "secondary2/voltage",
                ),
                (
                    "primary3/current",
                    "secondary3/current",
                    "secondary3/voltage",
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let load = Phasor([10.0, 16.0, 24.0][i], omega * [0.005, 0.009, 0.015][i]);
                let (ip, is, vs) = if loaded {
                    let total = zs.plus(load);
                    let ip = phases[i].over(zp.minus(zm.times(zm).over(total)));
                    let is = Phasor(0.0, 0.0).minus(zm.times(ip).over(total));
                    (ip, is, Phasor(0.0, 0.0).minus(is.times(load)))
                } else {
                    let ip = phases[i].over(zp);
                    (ip, Phasor(0.0, 0.0), zm.times(ip))
                };
                slots.extend([(1, primary, ip), (1, secondary, is), (1, output, vs)]);
            }
            run.advance(400);
            compare_phasors(&mut run, frequency, &slots);
        }
    }
}

#[test]
fn three_phase_load_preserves_the_dc_seeded_current_under_an_explicit_capacitor_voltage() {
    let parts = [
        (
            "three_phase_load_star",
            load_values(true),
            &[("l1", 1), ("l2", 0), ("l3", 0), ("n", 0)][..],
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts":24,"internalResistanceOhms":2}),
            &[("pin1", 1), ("pin2", 0)][..],
        ),
        (
            "capacitor",
            json!({"capacitanceFarads":0.001,"initialVoltageVolts":0}),
            &[("pin1", 1), ("pin2", 0)][..],
        ),
    ];
    let fixture = request(&parts);
    let run = Run::compile(fixture.clone());
    assert_eq!(run.value(0, "l1/voltage"), 0.0);
    assert!((run.value(0, "l1/current") - 2.0).abs() < 1e-12);
    assert!((run.value(0, "n/current") + 2.0).abs() < 1e-12);

    let mut invalid = fixture;
    let open_circuit_engine::ComponentSpec::ThreePhaseLoadStar { parameters, .. } =
        &mut invalid.schematic.components[0]
    else {
        unreachable!()
    };
    parameters.phase1_inductance_henries = -0.1;
    let CompileResult::Failure { diagnostics } = Engine::default().compile(invalid) else {
        panic!("negative winding inductance cannot compile");
    };
    let id = StableId::parse(id(0)).unwrap();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::InvalidParameter
            && diagnostic.references.iter().any(|reference| matches!(reference,
                open_circuit_engine::DiagnosticReference::Component { component_id } if *component_id == id
            ))
    }));
}

const SINGLE_BANK_TERMINALS: &[(&str, i32)] = &[
    ("p1a", 1),
    ("p1b", 0),
    ("p2a", 0),
    ("p2b", 0),
    ("p3a", 0),
    ("p3b", 0),
    ("s1a", 4),
    ("s1b", 5),
    ("s2a", 5),
    ("s2b", 5),
    ("s3a", 5),
    ("s3b", 5),
];

#[test]
fn transformer_preserves_its_dc_seed_during_constrained_initialization() {
    let run = Run::compile(request(&[
        (
            "transformer_three_phase",
            transformer_values(),
            SINGLE_BANK_TERMINALS,
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts":24,"internalResistanceOhms":2}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "capacitor",
            json!({"capacitanceFarads":0.001,"initialVoltageVolts":0}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 4), ("pin2", 5)],
        ),
    ]));
    assert_eq!(run.value(0, "primary1/voltage"), 0.0);
    assert!((run.value(0, "primary1/current") - 6.0).abs() < 1e-12);
    assert_eq!(run.value(0, "secondary1/current"), 0.0);
}

#[test]
fn transformer_opening_preserves_both_currents_and_recovers_with_the_coupled_matrix() {
    let fixture = request(&[
        (
            "transformer_three_phase",
            transformer_values(),
            SINGLE_BANK_TERMINALS,
        ),
        (
            "dc_voltage_source",
            json!({"voltageVolts":24,"internalResistanceOhms":0}),
            &[("pin1", 2), ("pin2", 0)],
        ),
        (
            "breaker_three_pole",
            json!({"closedContactResistanceOhms":0.01,"instantaneousTripAmps":1000,"initialClosed":1}),
            &[
                ("l1", 2),
                ("t1", 1),
                ("l2", 0),
                ("t2", 0),
                ("l3", 0),
                ("t3", 0),
            ],
        ),
        (
            "resistor",
            json!({"resistanceOhms":5}),
            &[("pin1", 1), ("pin2", 0)],
        ),
        (
            "resistor",
            json!({"resistanceOhms":10}),
            &[("pin1", 4), ("pin2", 5)],
        ),
    ]);
    let mut run = Run::compile(fixture);
    run.advance(1);
    let time = run.snapshot.metadata.simulation_time_seconds;
    let mut expected = [
        run.value(0, "primary1/current"),
        run.value(0, "secondary1/current"),
    ];
    assert!(expected[0] > 10.0);
    let SetOperatorStateResult::Success {
        internal_substeps, ..
    } = run.operate(2, OperatorPosition::Open)
    else {
        panic!("shunt supplies a path for the primary current");
    };
    assert_eq!(run.snapshot.metadata.simulation_time_seconds, time);
    assert_eq!(
        [
            run.value(0, "primary1/current"),
            run.value(0, "secondary1/current")
        ],
        expected
    );
    assert!((run.value(0, "primary1/voltage") + 5.0 * expected[0]).abs() < 1e-10);

    // The source is disconnected. With both loads shunting the windings,
    // solve (L+hR)i₁ = (L-(Δt-h)R)i₀ directly for each internal step.
    let dt = 0.001 / f64::from(internal_substeps);
    let (lp, ls, mutual) = (0.054, 0.0185, 0.025);
    for substep in 0..internal_substeps {
        let h = if substep == 0 { dt } else { dt / 2.0 };
        let (p, s) = (lp + h * 7.0, ls + h * 11.0);
        let det = p * s - mutual * mutual;
        let rhs_p = (lp - (dt - h) * 7.0) * expected[0] + mutual * expected[1];
        let rhs_s = mutual * expected[0] + (ls - (dt - h) * 11.0) * expected[1];
        expected = [
            (s * rhs_p - mutual * rhs_s) / det,
            (p * rhs_s - mutual * rhs_p) / det,
        ];
    }
    run.advance(1);
    assert!((run.value(0, "primary1/current") - expected[0]).abs() < 1e-10);
    assert!((run.value(0, "secondary1/current") - expected[1]).abs() < 1e-10);
}
