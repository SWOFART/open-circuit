//! Stage 13 residual-current device (issue #198, docs/contracts/stage-13-protection-and-safety.md
//! section 4): the residual current is the sum of the pole currents, a first-order mean-square
//! detector feeds a trip dose, and the device latches at a dose of 1. The reference trip time is an
//! independent numerical integration of the same law over the analytic detector waveform.

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, Engine, OperatorPosition,
    ParameterId, ResultLayout, ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult,
    SetParameterRequest, SetParameterResult, StableId, component_catalog,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const LOAD: &str = "00000000-0000-4000-8000-000000000002";
const RCD: &str = "00000000-0000-4000-8000-000000000003";

const RATED: f64 = 0.03;
const TRIP_TIME: f64 = 0.1;
const SENSE: f64 = 0.02;

const DOCUMENTATION: &str = r#"{"designators":[],"wireNumbers":[],"terminals":[],"notes":[],
    "titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},
    "sheets":[]}"#;

fn part(id: &str, type_id: &str, parameters: Value, pins: &[(&str, (i32, i32))]) -> (Value, Value) {
    (
        json!({"typeId": type_id, "id": id, "position": {"x": 0, "y": 0},
            "rotationDeg": 0, "parameters": parameters}),
        json!({"componentId": id, "pins": pins.iter().map(|(pin, (x, y))| json!({
            "pinId": pin, "position": {"x": x, "y": y}})).collect::<Vec<_>>()}),
    )
}

/// Nodes: A (0,0) the source's positive pin and `l1`, B (10,0) `t1` and the load's first pin,
/// D (2,2) the source's negative pin, with the ground marker. With `returns_through_rcd` the load
/// returns through pole 2 (node C at (20,0), `t2`, then `l2` at D): a balanced loop. Otherwise it
/// returns straight to D: the whole load current is a residual current.
fn circuit(
    residual_amps: f64,
    returns_through_rcd: bool,
    four_pole: bool,
    rcd_parameters: Value,
) -> CompileRequest {
    let volts = 100.0;
    let load = volts / residual_amps;
    let (load_pin2, rcd_pole_two) = if returns_through_rcd {
        ((20, 0), vec![("l2", (2, 2)), ("t2", (20, 0))])
    } else {
        ((2, 2), vec![("l2", (30, 30)), ("t2", (31, 30))])
    };
    let mut rcd_pins = vec![("l1", (0, 0)), ("t1", (10, 0))];
    rcd_pins.extend(rcd_pole_two);
    if four_pole {
        rcd_pins.extend([
            ("l3", (40, 40)),
            ("t3", (41, 40)),
            ("l4", (50, 50)),
            ("t4", (51, 50)),
        ]);
    }
    let parts = [
        part(
            SOURCE,
            "dc_voltage_source",
            json!({"voltageVolts": volts, "internalResistanceOhms": 0.0}),
            &[("pin1", (0, 0)), ("pin2", (2, 2))],
        ),
        part(
            LOAD,
            "resistor",
            json!({"resistanceOhms": load}),
            &[("pin1", (10, 0)), ("pin2", load_pin2)],
        ),
        part(
            RCD,
            if four_pole {
                "rcd_four_pole"
            } else {
                "rcd_two_pole"
            },
            rcd_parameters,
            &rcd_pins,
        ),
    ];
    let value = json!({
        "schematic": {
            "schemaVersion": 3,
            "components": parts.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
            "wires": [], "junctions": [], "rails": [],
            "referenceMarkers": [{"id": "00000000-0000-4000-8000-0000000000f0",
                "position": {"x": 2, "y": 2}}],
            "documentation": serde_json::from_str::<Value>(DOCUMENTATION).unwrap(),
        },
        "geometry": {"components": parts.iter().map(|(_, g)| g.clone()).collect::<Vec<_>>()},
        "documentRevision": "rev-1",
    });
    serde_json::from_value(value).expect("a compile request")
}

fn parameters(initially_closed: bool) -> Value {
    let mut value = json!({
        "ratedResidualCurrentAmps": RATED,
        "tripTimeAtRatedSeconds": TRIP_TIME,
        "residualSenseSeconds": SENSE,
        "closedContactResistanceOhms": 0.0001,
    });
    if initially_closed {
        value["initialClosed"] = json!(1.0);
    }
    value
}

struct Run {
    engine: Engine,
    run_id: StableId,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
}

fn start(request: CompileRequest) -> Run {
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
    Run {
        engine,
        run_id,
        layout,
        snapshot: initial_snapshot,
    }
}

impl Run {
    fn value(&self, key: &str) -> f64 {
        let slot = self
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == key)
            .unwrap_or_else(|| panic!("no slot {key}"));
        self.snapshot.values[slot.offset as usize]
    }

    fn tripped(&self) -> bool {
        self.value(&format!("{RCD}/tripped")) == 1.0
    }

    fn residual_rms(&self) -> f64 {
        self.value(&format!("{RCD}/residual_rms"))
    }

    fn dose(&self) -> f64 {
        self.value(&format!("{RCD}/trip_dose"))
    }

    fn advance(&mut self, steps: u32) {
        self.snapshot = self
            .engine
            .advance(AdvanceRequest {
                run_id: self.run_id.clone(),
                step_count: steps,
            })
            .expect("the run advances");
    }

    /// The first public step (1 ms each) at which the device is latched, within `limit` steps.
    fn steps_until_tripped(&mut self, limit: u32) -> Option<u32> {
        for step in 1..=limit {
            self.advance(1);
            if self.tripped() {
                return Some(step);
            }
        }
        None
    }

    fn revision(&self) -> String {
        self.snapshot.metadata.document_revision.clone()
    }

    fn operate(&mut self, position: OperatorPosition) -> Result<(), Vec<DiagnosticCode>> {
        let revision = self.revision();
        let result = self.engine.set_operator_state(SetOperatorStateRequest {
            run_id: self.run_id.clone(),
            expected_document_revision: revision.clone(),
            document_revision: format!("{revision}+"),
            component_id: StableId::parse(RCD).unwrap(),
            position,
        });
        match result {
            SetOperatorStateResult::Success { snapshot, .. } => {
                self.snapshot = snapshot;
                Ok(())
            }
            SetOperatorStateResult::Failure { diagnostics } => {
                Err(diagnostics.into_iter().map(|d| d.code).collect())
            }
        }
    }

    fn set(&mut self, parameter: ParameterId, value: f64) -> SetParameterResult {
        let revision = self.revision();
        let result = self.engine.set_parameter(SetParameterRequest {
            run_id: self.run_id.clone(),
            expected_document_revision: revision.clone(),
            document_revision: format!("{revision}+"),
            component_id: StableId::parse(RCD).unwrap(),
            parameter_id: parameter,
            value,
        });
        if let SetParameterResult::Success { snapshot, .. } = &result {
            self.snapshot = snapshot.clone();
        }
        result
    }
}

/// The trip time, in seconds, of a constant residual `ratio` times the rated current: the
/// detector starts from rest, `I(t) = I sqrt(1 - exp(-t / tau))`, and the dose gains
/// `(I(t) / In)^2 / T` per second while `I(t) >= In`.
fn reference_trip_time(ratio: f64) -> f64 {
    let step = 1.0e-6;
    let (mut time, mut dose) = (0.0_f64, 0.0_f64);
    while dose < 1.0 {
        time += step;
        let level = ratio * (1.0 - (-time / SENSE).exp()).sqrt();
        dose = if level >= 1.0 {
            dose + step * level * level / TRIP_TIME
        } else {
            0.0
        };
        assert!(time < 10.0, "the reference never trips");
    }
    time
}

#[test]
fn a_balanced_load_leaves_no_residual_and_never_trips() {
    let mut run = start(circuit(1.0, true, false, parameters(true)));
    run.advance(500);

    assert!(!run.tripped());
    assert!(run.residual_rms() <= 1e-9, "{}", run.residual_rms());
    assert_eq!(run.dose(), 0.0);
}

#[test]
fn a_residual_below_the_rated_current_never_trips() {
    let mut run = start(circuit(0.5 * RATED, false, false, parameters(true)));

    assert_eq!(run.steps_until_tripped(1500), None);
    assert_eq!(run.dose(), 0.0);
    assert!(run.residual_rms() < RATED);
}

#[test]
fn a_residual_above_the_rated_current_trips_after_the_law_of_the_contract() {
    for ratio in [1.05, 2.0, 5.0] {
        let mut run = start(circuit(ratio * RATED, false, false, parameters(true)));
        let reference = reference_trip_time(ratio);
        let steps = run
            .steps_until_tripped(3000)
            .unwrap_or_else(|| panic!("{ratio} x In never trips"));
        let measured = f64::from(steps) * 1.0e-3;

        let tolerance = (0.01 * reference).max(2.0e-3);
        assert!(
            (measured - reference).abs() <= tolerance,
            "{ratio} x In: tripped at {measured} s, reference {reference} s"
        );
        // Latched: the poles are open, so the residual current is gone and the dose is held.
        run.advance(50);
        assert!(run.tripped());
        assert_eq!(run.dose(), 1.0);
    }
}

#[test]
fn a_close_never_trips_in_its_own_reply_and_a_latched_device_refuses_to_close() {
    let mut run = start(circuit(5.0 * RATED, false, false, parameters(false)));
    assert!(!run.tripped());
    assert_eq!(run.residual_rms(), 0.0);

    run.operate(OperatorPosition::Closed)
        .expect("a close succeeds");
    assert!(
        !run.tripped(),
        "the device cannot trip in the reply of its close"
    );

    let steps = run
        .steps_until_tripped(1000)
        .expect("it trips after the delay");
    assert!(steps > 1, "{steps}");
    assert!(
        run.operate(OperatorPosition::Closed).is_err(),
        "closed while latched"
    );
    assert!(
        run.operate(OperatorPosition::Pressed).is_err(),
        "a press is not an operation"
    );

    // An open clears the latch, the dose and the detector exactly.
    run.operate(OperatorPosition::Open)
        .expect("an open succeeds");
    assert!(!run.tripped());
    assert_eq!(run.dose(), 0.0);
    assert_eq!(run.residual_rms(), 0.0);

    run.operate(OperatorPosition::Closed)
        .expect("it closes again");
    assert!(run.steps_until_tripped(1000).is_some());
}

#[test]
fn the_rated_current_and_the_trip_time_are_live_and_the_rest_is_compile_only() {
    let mut run = start(circuit(2.0 * RATED, false, false, parameters(true)));
    run.advance(3);

    // Raising the rated current above the residual keeps the device from tripping.
    assert!(matches!(
        run.set(ParameterId::RatedResidualCurrentAmps, 0.1),
        SetParameterResult::Success { .. }
    ));
    assert_eq!(run.steps_until_tripped(1500), None);

    assert!(matches!(
        run.set(ParameterId::TripTimeAtRatedSeconds, 0.5),
        SetParameterResult::Success { .. }
    ));
    for compile_only in [
        ParameterId::ResidualSenseSeconds,
        ParameterId::ClosedContactResistanceOhms,
    ] {
        assert!(
            matches!(
                run.set(compile_only, 0.05),
                SetParameterResult::Failure { .. }
            ),
            "{compile_only:?}"
        );
    }
}

#[test]
fn the_four_pole_device_sums_all_four_poles_and_lays_out_its_slots_in_order() {
    let run = start(circuit(2.0 * RATED, false, true, parameters(true)));
    let keys: Vec<&str> = run
        .layout
        .slots
        .iter()
        .filter(|slot| slot.key.starts_with(RCD))
        .map(|slot| slot.key.as_str())
        .collect();
    let id = |suffix: &str| format!("{RCD}{suffix}");
    let expected: Vec<String> = (1..=4)
        .flat_map(|pole| {
            [
                id(&format!("/pole{pole}/voltage")),
                id(&format!("/pole{pole}/current")),
            ]
        })
        .chain([id("/tripped"), id("/residual_rms"), id("/trip_dose")])
        .collect();
    assert_eq!(
        keys,
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );

    // Only pole 1 carries current, so the residual is its current.
    let current = run.value(&id("/pole1/current"));
    assert!(
        (current - 2.0 * RATED).abs() < 1e-6 * 2.0 * RATED,
        "{current}"
    );
}

#[test]
fn the_catalog_lists_the_rcd_types_with_the_contract_parameters() {
    let catalog = component_catalog();
    for (name, poles) in [("rcd_two_pole", 2), ("rcd_four_pole", 4)] {
        let entry = catalog
            .component_types
            .iter()
            .find(|entry| serde_json::to_value(entry.type_id).unwrap() == json!(name))
            .unwrap_or_else(|| panic!("no type {name}"));
        assert_eq!(entry.pins.len(), 2 * poles, "{name}");
        assert_eq!(entry.pin_groups.as_ref().map(Vec::len), Some(poles));
        assert_eq!(entry.designator_prefix, "F");
        let bounds: Vec<_> = entry
            .parameters
            .iter()
            .map(|p| (p.default_value, p.minimum_value, p.maximum_value))
            .collect();
        assert_eq!(
            bounds,
            vec![
                (0.03, Some(0.003), Some(1.0)),
                (0.1, Some(0.01), Some(2.0)),
                (0.02, Some(0.001), Some(0.2)),
                (0.01, Some(0.0001), Some(10.0)),
                (0.0, None, None),
            ]
        );
        assert!(entry.parameters[4].optional);
    }
}
