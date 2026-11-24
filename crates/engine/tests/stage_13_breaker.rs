//! Stage 13 breaker trip classes (issue #199, docs/contracts/stage-13-protection-and-safety.md
//! section 5): the optional `curveClass` makes the three-pole breaker thermal-magnetic. The
//! magnetic threshold is 5, 10 or 20 times the rated current (classes B, C, D) and the thermal part
//! is the overload relay's detector and dose. Class 0 is the Stage 10 breaker, unchanged.

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, Engine, OperatorPosition,
    ParameterId, ResultLayout, ResultSnapshot, SetOperatorStateRequest, SetOperatorStateResult,
    SetParameterRequest, SetParameterResult, StableId, component_catalog,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const LOAD: &str = "00000000-0000-4000-8000-000000000002";
const BREAKER: &str = "00000000-0000-4000-8000-000000000003";

const RATED: f64 = 10.0;
const SENSE: f64 = 0.02;
const SCALE: f64 = 2.0;
const COOL: f64 = 5.0;

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

/// A 100 V ideal source through pole 1 of the breaker into a load that draws `amps`; the other
/// poles are unconnected. Nodes: A (0,0) the source's positive pin and `l1`, B (10,0) `t1` and the
/// load, D (2,2) the source's negative pin with the ground marker.
fn circuit(amps: f64, breaker_parameters: Value) -> CompileRequest {
    let parts = [
        part(
            SOURCE,
            "dc_voltage_source",
            json!({"voltageVolts": 100.0, "internalResistanceOhms": 0.0}),
            &[("pin1", (0, 0)), ("pin2", (2, 2))],
        ),
        part(
            LOAD,
            "resistor",
            json!({"resistanceOhms": 100.0 / amps}),
            &[("pin1", (10, 0)), ("pin2", (2, 2))],
        ),
        part(
            BREAKER,
            "breaker_three_pole",
            breaker_parameters,
            &[
                ("l1", (0, 0)),
                ("t1", (10, 0)),
                ("l2", (30, 30)),
                ("t2", (31, 30)),
                ("l3", (40, 40)),
                ("t3", (41, 40)),
            ],
        ),
    ];
    serde_json::from_value(json!({
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
    }))
    .expect("a compile request")
}

/// A closed breaker of the class with fast thermal constants, so the tests stay short.
fn classed(class: u8) -> Value {
    json!({
        "closedContactResistanceOhms": 0.0001,
        "instantaneousTripAmps": 100000.0,
        "initialClosed": 1.0,
        "curveClass": class,
        "ratedCurrentAmps": RATED,
        "thermalSenseSeconds": SENSE,
        "tripScaleSeconds": SCALE,
        "coolSeconds": COOL,
    })
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

fn refused(request: CompileRequest) -> Vec<DiagnosticCode> {
    let CompileResult::Failure { diagnostics } = Engine::default().compile(request) else {
        panic!("the circuit should be refused");
    };
    diagnostics.into_iter().map(|d| d.code).collect()
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
        self.value(&format!("{BREAKER}/tripped")) == 1.0
    }

    fn dose(&self) -> f64 {
        self.value(&format!("{BREAKER}/thermal_dose"))
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

    fn steps_until_tripped(&mut self, limit: u32) -> Option<u32> {
        for step in 1..=limit {
            self.advance(1);
            if self.tripped() {
                return Some(step);
            }
        }
        None
    }

    fn operate(&mut self, position: OperatorPosition) -> bool {
        let revision = self.snapshot.metadata.document_revision.clone();
        match self.engine.set_operator_state(SetOperatorStateRequest {
            run_id: self.run_id.clone(),
            expected_document_revision: revision.clone(),
            document_revision: format!("{revision}+"),
            component_id: StableId::parse(BREAKER).unwrap(),
            position,
        }) {
            SetOperatorStateResult::Success { snapshot, .. } => {
                self.snapshot = snapshot;
                true
            }
            SetOperatorStateResult::Failure { .. } => false,
        }
    }
}

/// The thermal trip time of a constant current `ratio` times the rated current from a cold start:
/// the detector is `I^2 (1 - exp(-t/tau))` and the dose gains `(q - 1) / K` per second while
/// `q = detector / In^2 > 1`.
fn reference_trip_time(ratio: f64) -> f64 {
    let step = 1.0e-6;
    let (mut time, mut dose) = (0.0_f64, 0.0_f64);
    while dose < 1.0 {
        time += step;
        let q = ratio * ratio * (1.0 - (-time / SENSE).exp());
        dose = if q > 1.0 {
            dose + step * (q - 1.0) / SCALE
        } else {
            dose * (-step / COOL).exp()
        };
        assert!(time < 100.0, "the reference never trips");
    }
    time
}

#[test]
fn the_magnetic_part_trips_at_five_ten_and_twenty_times_the_rated_current() {
    for (class, multiple) in [(1_u8, 5.0), (2, 10.0), (3, 20.0)] {
        // Just above the threshold: tripped at the first sample, the compile itself.
        let above = start(circuit(1.02 * multiple * RATED, classed(class)));
        assert!(above.tripped(), "class {class} above {multiple} x In");

        // Just below it: no magnetic trip; the thermal part has not had time to act.
        let below = start(circuit(0.98 * multiple * RATED, classed(class)));
        assert!(!below.tripped(), "class {class} below {multiple} x In");
    }
}

#[test]
fn the_class_thresholds_replace_the_instantaneous_trip_setting() {
    // A tiny instantaneous setting would trip a class-0 breaker at 50 A; class C ignores it.
    let mut parameters = classed(2);
    parameters["instantaneousTripAmps"] = json!(1.0);
    assert!(!start(circuit(5.0 * RATED, parameters.clone())).tripped());

    parameters.as_object_mut().unwrap().remove("curveClass");
    assert!(
        start(circuit(5.0 * RATED, parameters)).tripped(),
        "class 0 uses the setting"
    );
}

#[test]
fn the_thermal_part_trips_after_the_overload_law_at_several_multiples() {
    for (class, ratio) in [(3_u8, 5.5), (2, 3.0), (1, 2.0), (1, 1.2)] {
        let mut run = start(circuit(ratio * RATED, classed(class)));
        assert!(
            !run.tripped(),
            "class {class} at {ratio} x In starts closed"
        );
        let reference = reference_trip_time(ratio);
        let steps = run
            .steps_until_tripped(20_000)
            .unwrap_or_else(|| panic!("class {class} at {ratio} x In never trips"));
        let measured = f64::from(steps) * 1.0e-3;

        let tolerance = (0.01 * reference).max(2.0e-3);
        assert!(
            (measured - reference).abs() <= tolerance,
            "class {class} at {ratio} x In: tripped at {measured} s, reference {reference} s"
        );
        assert!(run.dose() >= 1.0 - 1e-12, "{}", run.dose());
    }
}

#[test]
fn a_current_at_or_below_the_rated_current_never_trips() {
    let mut run = start(circuit(0.9 * RATED, classed(1)));

    assert_eq!(run.steps_until_tripped(5000), None);
    assert_eq!(run.dose(), 0.0);
}

#[test]
fn a_latched_breaker_refuses_to_close_but_a_dose_does_not_and_the_dose_cools() {
    let mut run = start(circuit(2.0 * RATED, classed(1)));
    let cold = run.steps_until_tripped(20_000).expect("it trips");
    assert!(
        !run.operate(OperatorPosition::Closed),
        "refused while latched"
    );

    // An explicit open clears the latch and keeps the dose (it is the cooling that lowers it).
    assert!(run.operate(OperatorPosition::Open));
    assert!(!run.tripped());
    assert!(run.dose() >= 0.99, "{}", run.dose());

    // There is no dose gate on closing: the breaker closes at once and trips again soon.
    assert!(run.operate(OperatorPosition::Closed));
    let again = run.steps_until_tripped(20_000).expect("it trips again");
    assert!(
        again < cold / 4,
        "re-closed into a hot breaker: {again} steps against {cold}"
    );

    // Open and cool for several time constants: the dose falls away.
    assert!(run.operate(OperatorPosition::Open));
    run.advance(5 * (COOL * 1000.0) as u32);
    assert!(run.dose() < 0.01, "{}", run.dose());
}

#[test]
fn class_zero_keeps_the_stage_ten_slots_and_never_reports_a_dose() {
    let plain = json!({
        "closedContactResistanceOhms": 0.0001,
        "instantaneousTripAmps": 100.0,
        "initialClosed": 1.0,
    });
    let run = start(circuit(RATED, plain));
    let keys: Vec<&str> = run
        .layout
        .slots
        .iter()
        .filter(|slot| slot.key.starts_with(BREAKER))
        .map(|slot| slot.key.as_str())
        .collect();
    let id = |suffix: &str| format!("{BREAKER}{suffix}");
    let expected: Vec<String> = (1..=3)
        .flat_map(|pole| {
            [
                id(&format!("/pole{pole}/voltage")),
                id(&format!("/pole{pole}/current")),
            ]
        })
        .chain([id("/tripped")])
        .collect();
    assert_eq!(
        keys,
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );

    // A class breaker has the dose after `tripped`.
    let classed_run = start(circuit(RATED, classed(2)));
    assert_eq!(
        classed_run.layout.slots.last().map(|slot| slot.key.clone()),
        Some(id("/thermal_dose"))
    );
}

#[test]
fn the_class_and_its_inputs_are_validated_and_compile_only() {
    for (key, value) in [
        ("curveClass", json!(4)),
        ("curveClass", json!(0.5)),
        ("curveClass", json!(-1)),
        ("ratedCurrentAmps", json!(0.01)),
        ("thermalSenseSeconds", json!(5.0)),
        ("tripScaleSeconds", json!(0.0)),
        ("coolSeconds", json!(100000.0)),
    ] {
        let mut parameters = classed(1);
        parameters[key] = value.clone();
        assert_eq!(
            refused(circuit(RATED, parameters)),
            vec![DiagnosticCode::InvalidParameter],
            "{key} = {value}"
        );
    }

    let mut run = start(circuit(2.0 * RATED, classed(1)));
    for compile_only in [
        ParameterId::RatedCurrentAmps,
        ParameterId::ThermalSenseSeconds,
        ParameterId::TripScaleSeconds,
        ParameterId::CoolSeconds,
        ParameterId::CurveClass,
    ] {
        let revision = run.snapshot.metadata.document_revision.clone();
        let result = run.engine.set_parameter(SetParameterRequest {
            run_id: run.run_id.clone(),
            expected_document_revision: revision.clone(),
            document_revision: format!("{revision}+"),
            component_id: StableId::parse(BREAKER).unwrap(),
            parameter_id: compile_only,
            value: 1.0,
        });
        assert!(
            matches!(result, SetParameterResult::Failure { .. }),
            "{compile_only:?}"
        );
    }
}

#[test]
fn the_catalog_lists_the_optional_class_parameters_after_the_stage_ten_ones() {
    let catalog = component_catalog();
    let entry = catalog
        .component_types
        .iter()
        .find(|entry| serde_json::to_value(entry.type_id).unwrap() == json!("breaker_three_pole"))
        .expect("the breaker");
    let ids: Vec<String> = entry
        .parameters
        .iter()
        .map(|p| {
            serde_json::to_value(p.id)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        ids,
        [
            "closedContactResistanceOhms",
            "instantaneousTripAmps",
            "initialClosed",
            "curveClass",
            "ratedCurrentAmps",
            "thermalSenseSeconds",
            "tripScaleSeconds",
            "coolSeconds",
        ]
    );
    let bounds: Vec<_> = entry.parameters[3..]
        .iter()
        .map(|p| {
            (
                p.optional,
                p.default_value,
                p.minimum_value,
                p.maximum_value,
            )
        })
        .collect();
    assert_eq!(
        bounds,
        vec![
            (true, 0.0, Some(0.0), Some(3.0)),
            (true, 10.0, Some(0.1), Some(10_000.0)),
            (true, 0.02, Some(0.005), Some(2.0)),
            (true, 10.0, Some(0.01), Some(10_000.0)),
            (true, 30.0, Some(0.01), Some(10_000.0)),
        ]
    );
}
