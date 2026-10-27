//! Faults (docs/contracts/stage-5-assignments.md section 5): an overlay of the compile request
//! that is never part of the schematic. The known answers are direct circuit equations, in
//! `tests/golden/stage-5-fault-finding.expected.json` and below.

use open_circuit_engine::{
    AdvanceRequest, CompileRequest, CompileResult, DiagnosticCode, DiagnosticReference, Engine,
    FaultKind, ResultLayout, ResultSnapshot, SetParameterRequest, SetParameterResult, StableId,
    component_catalog, supported_fault_kinds,
};
use serde_json::{Value, json};

const EXPECTED: &str = include_str!("../../../tests/golden/stage-5-fault-finding.expected.json");

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const R1: &str = "00000000-0000-4000-8000-000000000002";
const R2: &str = "00000000-0000-4000-8000-000000000003";
const METER: &str = "00000000-0000-4000-8000-000000000004";

fn id(value: &str) -> StableId {
    StableId::parse(value).unwrap()
}

fn expected() -> Value {
    serde_json::from_str(EXPECTED).unwrap()
}

fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap()
}

fn measurement<'a>(expected: &'a Value, name: &str) -> &'a Value {
    expected["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"].as_str().unwrap().starts_with(name))
        .unwrap_or_else(|| panic!("no measurement {name}"))
}

fn two_pin(
    id: &str,
    type_id: &str,
    parameters: Value,
    pin1: (i32, i32),
    pin2: (i32, i32),
) -> (Value, Value) {
    (
        json!({
            "typeId": type_id,
            "id": id,
            "position": { "x": 0, "y": 0 },
            "rotationDeg": 0,
            "parameters": parameters,
        }),
        json!({
            "componentId": id,
            "pins": [
                { "pinId": "pin1", "position": { "x": pin1.0, "y": pin1.1 } },
                { "pinId": "pin2", "position": { "x": pin2.0, "y": pin2.1 } },
            ],
        }),
    )
}

fn request(parts: Vec<(Value, Value)>, markers: &[(i32, i32)], faults: Value) -> CompileRequest {
    let components = parts
        .iter()
        .map(|(component, _)| component.clone())
        .collect::<Vec<_>>();
    let geometry = parts
        .iter()
        .map(|(_, pins)| pins.clone())
        .collect::<Vec<_>>();
    let reference_markers = markers
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            json!({ "id": format!("00000000-0000-4000-8000-0000000000f{index}"), "position": { "x": x, "y": y } })
        })
        .collect::<Vec<_>>();
    let mut value = json!({
        "schematic": {
            "schemaVersion": 1,
            "components": components,
            "wires": [],
            "junctions": [],
            "referenceMarkers": reference_markers,
        },
        "geometry": { "components": geometry },
        "documentRevision": "rev-1",
    });
    if !faults.is_null() {
        value["faults"] = faults;
    }
    serde_json::from_value(value).unwrap()
}

/// The fault-finding fixture: 10 V ideal source, R1 = R2 = 1 kOhm in series, a 10 MOhm
/// voltmeter across R2, and a reference marker on the source's negative pin.
fn divider(faults: Value) -> CompileRequest {
    // IN (0,0), OUT (4,0), GND (2,2).
    request(
        vec![
            two_pin(
                SOURCE,
                "dc_voltage_source",
                json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                (0, 0),
                (2, 2),
            ),
            two_pin(
                R1,
                "resistor",
                json!({ "resistanceOhms": 1000.0 }),
                (0, 0),
                (4, 0),
            ),
            two_pin(
                R2,
                "resistor",
                json!({ "resistanceOhms": 1000.0 }),
                (4, 0),
                (2, 2),
            ),
            two_pin(
                METER,
                "voltmeter",
                json!({ "inputResistanceOhms": 1e7 }),
                (4, 0),
                (2, 2),
            ),
        ],
        &[(2, 2)],
        faults,
    )
}

fn fault(id_suffix: u32, kind: &str, component: &str) -> Value {
    json!({
        "id": format!("00000000-0000-4000-8000-0000000000a{id_suffix}"),
        "kind": kind,
        "componentId": component,
    })
}

fn parameter_fault(id_suffix: u32, component: &str, parameter: &str, value: f64) -> Value {
    json!({
        "id": format!("00000000-0000-4000-8000-0000000000a{id_suffix}"),
        "kind": "parameter",
        "componentId": component,
        "parameterId": parameter,
        "value": value,
    })
}

struct Compiled {
    engine: Engine,
    run_id: StableId,
    layout: ResultLayout,
    snapshot: ResultSnapshot,
}

fn compile(request: CompileRequest) -> Compiled {
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

    Compiled {
        engine,
        run_id,
        layout,
        snapshot: initial_snapshot,
    }
}

fn failure_codes(request: CompileRequest) -> Vec<(DiagnosticCode, Vec<String>)> {
    let CompileResult::Failure { diagnostics } = Engine::default().compile(request) else {
        panic!("the circuit should not compile");
    };

    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let components = diagnostic
                .references
                .into_iter()
                .filter_map(|reference| match reference {
                    DiagnosticReference::Component { component_id } => {
                        Some(component_id.as_str().to_owned())
                    }
                    _ => None,
                })
                .collect();
            (diagnostic.code, components)
        })
        .collect()
}

impl Compiled {
    fn value(&self, key: &str) -> f64 {
        let slot = self
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == key)
            .unwrap_or_else(|| panic!("no slot {key}"));
        self.snapshot.values[slot.offset as usize]
    }

    fn voltage(&self, component: &str) -> f64 {
        self.value(&format!("{component}:voltage"))
    }

    fn current(&self, component: &str) -> f64 {
        self.value(&format!("{component}:current"))
    }

    fn advance(&mut self, steps: u32) {
        self.snapshot = self
            .engine
            .advance(AdvanceRequest {
                run_id: self.run_id.clone(),
                step_count: steps,
            })
            .unwrap();
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} is not within {tolerance} of {expected}"
    );
}

#[test]
fn a_circuit_without_faults_is_the_healthy_circuit() {
    let expected = expected();
    let healthy = measurement(&expected, "healthy");

    for request in [divider(Value::Null), divider(json!([]))] {
        let run = compile(request);

        assert_close(
            run.voltage(METER),
            number(healthy, "voltmeterReadingVolts"),
            number(healthy, "toleranceVolts"),
            "healthy reading",
        );
    }
}

#[test]
fn an_open_resistor_carries_no_current_and_the_meter_across_it_shows_the_supply() {
    let expected = expected();
    let open = measurement(&expected, "the fault of the assignment");
    let run = compile(divider(json!([fault(1, "open", R2)])));

    assert_close(
        run.voltage(METER),
        number(open, "voltmeterReadingVolts"),
        number(open, "toleranceVolts"),
        "voltmeter across the open R2",
    );
    assert_eq!(run.current(R2), 0.0, "an open branch carries no current");
    assert_close(
        run.voltage(R2),
        number(open, "r2VoltageVolts"),
        number(open, "toleranceVolts"),
        "voltage across the open R2",
    );
    assert_close(
        run.current(R1),
        number(open, "r1CurrentAmps"),
        number(open, "currentToleranceAmps"),
        "R1 current: only the meter loads the open branch",
    );
}

#[test]
fn a_shorted_resistor_is_one_milliohm_and_the_other_hypotheses_match_the_fixture() {
    let expected = expected();

    for (name, faults) in [
        ("hypothesis: R2 short", json!([fault(1, "short", R2)])),
        ("hypothesis: R1 open", json!([fault(1, "open", R1)])),
        ("hypothesis: R1 short", json!([fault(1, "short", R1)])),
    ] {
        let reference = measurement(&expected, name);
        let run = compile(divider(faults));

        assert_close(
            run.voltage(METER),
            number(reference, "voltmeterReadingVolts"),
            number(reference, "toleranceVolts"),
            name,
        );
    }

    // The shorted part reports the voltage and current of its 1 mOhm, in Ohm's-law agreement.
    let shorted = compile(divider(json!([fault(1, "short", R2)])));
    assert_close(
        shorted.voltage(R2),
        shorted.current(R2) * open_circuit_engine::SHORT_FAULT_RESISTANCE_OHMS,
        1e-15,
        "R2 = 1 mOhm",
    );
}

#[test]
fn a_parameter_fault_replaces_the_value_without_touching_the_schematic() {
    let request = divider(json!([parameter_fault(1, R2, "resistanceOhms", 3000.0)]));
    let run = compile(request.clone());
    let parallel = 3000.0 * 1e7 / (3000.0 + 1e7);
    let reading = 10.0 * parallel / (1000.0 + parallel);

    assert_close(
        run.voltage(METER),
        reading,
        1e-9,
        "reading with R2 = 3 kOhm",
    );

    // The request still says 1 kOhm and compiles to the healthy circuit once the fault is
    // gone: the fault was an input of the first compile, not an edit of the document.
    assert_eq!(
        serde_json::to_value(&request.schematic).unwrap()["components"][2]["parameters"]["resistanceOhms"],
        json!(1000.0)
    );
    let mut without = request;
    without.faults = None;
    assert_close(
        compile(without).voltage(METER),
        4.999750012499375,
        1e-9,
        "the same schematic without the fault",
    );
}

#[test]
fn the_result_layout_is_the_layout_of_the_unfaulted_circuit() {
    let healthy = compile(divider(Value::Null));

    for faults in [
        json!([fault(1, "open", R2)]),
        json!([
            fault(1, "short", R1),
            parameter_fault(2, R2, "resistanceOhms", 220.0)
        ]),
        json!([fault(1, "open", SOURCE)]),
    ] {
        let faulted = compile(divider(faults));

        assert_eq!(faulted.layout, healthy.layout);
        assert_eq!(faulted.snapshot.values.len(), healthy.snapshot.values.len());
        assert!(
            faulted
                .snapshot
                .values
                .iter()
                .all(|value| value.is_finite())
        );
    }
}

#[test]
fn an_open_source_leaves_a_dead_circuit_that_still_reports_the_supply_across_it() {
    let run = compile(divider(json!([fault(1, "open", SOURCE)])));

    assert_eq!(run.current(SOURCE), 0.0);
    assert_close(run.voltage(METER), 0.0, 1e-12, "no supply, no reading");
    assert_close(run.current(R1), 0.0, 1e-12, "no current");
}

#[test]
fn invalid_faults_are_refused_with_invalid_fault_and_the_component() {
    let cases: Vec<(&str, Value, &str)> = vec![
        (
            "a component that is not in the schematic",
            json!([fault(1, "open", "00000000-0000-4000-8000-0000000000ee")]),
            "00000000-0000-4000-8000-0000000000ee",
        ),
        (
            "open on a voltmeter",
            json!([fault(1, "open", METER)]),
            METER,
        ),
        (
            "short on a source",
            json!([fault(1, "short", SOURCE)]),
            SOURCE,
        ),
        (
            "two faults on one component",
            json!([fault(1, "open", R1), fault(2, "short", R1)]),
            R1,
        ),
        (
            "a repeated fault ID",
            json!([fault(1, "open", R1), fault(1, "open", R2)]),
            R2,
        ),
        (
            "a parameter fault with no value",
            json!([{ "id": "00000000-0000-4000-8000-0000000000a1", "kind": "parameter", "componentId": R1, "parameterId": "resistanceOhms" }]),
            R1,
        ),
        (
            "an open fault that carries a value",
            json!([{ "id": "00000000-0000-4000-8000-0000000000a1", "kind": "open", "componentId": R1, "value": 1.0 }]),
            R1,
        ),
        (
            "a parameter the type does not have",
            json!([parameter_fault(1, R1, "capacitanceFarads", 1e-6)]),
            R1,
        ),
        (
            "a value outside the domain",
            json!([parameter_fault(1, R1, "resistanceOhms", 0.0)]),
            R1,
        ),
        (
            "a negative resistance",
            json!([parameter_fault(1, R1, "resistanceOhms", -5.0)]),
            R1,
        ),
    ];

    for (name, faults, component) in cases {
        let codes = failure_codes(divider(faults));

        assert!(
            codes
                .iter()
                .any(|(code, components)| *code == DiagnosticCode::InvalidFault
                    && components.iter().any(|candidate| candidate == component)),
            "{name}: {codes:?}"
        );
        assert!(
            codes
                .iter()
                .all(|(code, _)| *code == DiagnosticCode::InvalidFault),
            "{name} must be a fault error and nothing else: {codes:?}"
        );
    }
}

#[test]
fn an_initial_condition_cannot_be_overridden() {
    let capacitor = "00000000-0000-4000-8000-0000000000c1";
    let request = request(
        vec![
            two_pin(
                SOURCE,
                "dc_voltage_source",
                json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                (0, 0),
                (2, 2),
            ),
            two_pin(
                R1,
                "resistor",
                json!({ "resistanceOhms": 1000.0 }),
                (0, 0),
                (4, 0),
            ),
            two_pin(
                capacitor,
                "capacitor",
                json!({ "capacitanceFarads": 1e-5 }),
                (4, 0),
                (2, 2),
            ),
        ],
        &[(2, 2)],
        json!([parameter_fault(1, capacitor, "initialVoltageVolts", 3.0)]),
    );

    let codes = failure_codes(request);

    assert!(
        codes
            .iter()
            .any(|(code, components)| *code == DiagnosticCode::InvalidFault
                && components.iter().any(|component| component == capacitor))
    );
}

#[test]
fn a_relay_fault_that_breaks_the_dropout_rule_is_a_fault_error() {
    let relay = "00000000-0000-4000-8000-0000000000d1";
    let make = |dropout: f64| {
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 5.0, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (2, 2),
                ),
                (
                    json!({
                        "typeId": "relay_spdt", "id": relay,
                        "position": { "x": 0, "y": 0 }, "rotationDeg": 0,
                        "parameters": {
                            "coilResistanceOhms": 100.0, "coilInductanceHenries": 0.01,
                            "pickupCurrentAmps": 0.04, "dropoutCurrentAmps": 0.02,
                            "closedContactResistanceOhms": 0.1
                        }
                    }),
                    json!({
                        "componentId": relay,
                        "pins": [
                            { "pinId": "coil_positive", "position": { "x": 0, "y": 0 } },
                            { "pinId": "coil_negative", "position": { "x": 2, "y": 2 } },
                            { "pinId": "common", "position": { "x": 6, "y": 0 } },
                            { "pinId": "normally_open", "position": { "x": 8, "y": 0 } },
                            { "pinId": "normally_closed", "position": { "x": 10, "y": 0 } },
                        ],
                    }),
                ),
            ],
            &[(2, 2)],
            json!([parameter_fault(1, relay, "dropoutCurrentAmps", dropout)]),
        )
    };

    // A dropout below the pickup is allowed, and it changes the relay's hysteresis.
    let run = compile(make(0.01));
    assert!(run.snapshot.states.len() == 1);

    // A dropout at or above the pickup breaks the relay's own rule.
    let codes = failure_codes(make(0.05));
    assert!(
        codes
            .iter()
            .any(|(code, components)| *code == DiagnosticCode::InvalidFault
                && components.iter().any(|component| component == relay)),
        "{codes:?}"
    );
}

#[test]
fn a_fuse_parameter_fault_changes_when_it_trips() {
    let fuse = "00000000-0000-4000-8000-0000000000e2";
    let make = |faults: Value| {
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 2.02, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (2, 2),
                ),
                two_pin(
                    fuse,
                    "fuse",
                    json!({ "intactResistanceOhms": 0.01, "tripI2tAmpereSquaredSeconds": 0.004 }),
                    (0, 0),
                    (4, 0),
                ),
                two_pin(
                    R1,
                    "resistor",
                    json!({ "resistanceOhms": 10.0 }),
                    (4, 0),
                    (2, 2),
                ),
            ],
            &[(2, 2)],
            faults,
        )
    };
    // 0.2 A through the fuse: 0.004 A^2*s would take about 0.1 s.
    let mut healthy = compile(make(Value::Null));
    healthy.advance(2);
    assert_eq!(fuse_state(&healthy), "intact");

    let mut weakened = compile(make(json!([parameter_fault(
        1,
        fuse,
        "tripI2tAmpereSquaredSeconds",
        1e-5
    )])));
    weakened.advance(2);
    assert_eq!(fuse_state(&weakened), "tripped");
}

fn fuse_state(run: &Compiled) -> String {
    match run.snapshot.states.as_slice() {
        [open_circuit_engine::DeviceState::Fuse { state, .. }] => {
            format!("{state:?}").to_lowercase()
        }
        other => panic!("one fuse state expected, found {other:?}"),
    }
}

#[test]
fn open_and_shorted_capacitors_and_inductors_follow_the_network_they_leave() {
    let capacitor = "00000000-0000-4000-8000-0000000000c1";
    let inductor = "00000000-0000-4000-8000-0000000000c2";
    let rc = |faults: Value, initial: Option<f64>| {
        let mut parameters = json!({ "capacitanceFarads": 1e-5 });
        if let Some(volts) = initial {
            parameters["initialVoltageVolts"] = json!(volts);
        }
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (2, 2),
                ),
                two_pin(
                    R1,
                    "resistor",
                    json!({ "resistanceOhms": 1000.0 }),
                    (0, 0),
                    (4, 0),
                ),
                two_pin(capacitor, "capacitor", parameters, (4, 0), (2, 2)),
            ],
            &[(2, 2)],
            faults,
        )
    };

    // An open capacitor: no current, the whole supply across it, whatever its initial
    // condition said and however long it runs.
    let mut open = compile(rc(json!([fault(1, "open", capacitor)]), Some(3.0)));
    for steps in [0, 5, 50] {
        if steps > 0 {
            open.advance(steps);
        }
        assert_eq!(open.current(capacitor), 0.0);
        assert_close(
            open.voltage(capacitor),
            10.0,
            1e-9,
            "voltage across the open capacitor",
        );
        assert_close(open.current(R1), 0.0, 1e-12, "no current in R1");
    }

    // A shorted capacitor: a resistor of 1 mOhm, so a steady current at once.
    let mut shorted = compile(rc(json!([fault(1, "short", capacitor)]), Some(3.0)));
    for steps in [0, 5, 50] {
        if steps > 0 {
            shorted.advance(steps);
        }
        let expected = 10.0 / (1000.0 + open_circuit_engine::SHORT_FAULT_RESISTANCE_OHMS);
        assert_close(shorted.current(capacitor), expected, 1e-12, "short current");
        assert_close(
            shorted.voltage(capacitor),
            expected * open_circuit_engine::SHORT_FAULT_RESISTANCE_OHMS,
            1e-12,
            "short voltage",
        );
    }

    // The healthy circuit still charges: faults do not leak into an unfaulted compile.
    let mut healthy = compile(rc(Value::Null, Some(0.0)));
    healthy.advance(10);
    assert!(healthy.voltage(capacitor) > 0.5 && healthy.voltage(capacitor) < 9.9);

    // An open inductor in series with a resistor.
    let rl = request(
        vec![
            two_pin(
                SOURCE,
                "dc_voltage_source",
                json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                (0, 0),
                (2, 2),
            ),
            two_pin(
                R1,
                "resistor",
                json!({ "resistanceOhms": 10.0 }),
                (0, 0),
                (4, 0),
            ),
            two_pin(
                inductor,
                "inductor",
                json!({ "inductanceHenries": 0.1 }),
                (4, 0),
                (2, 2),
            ),
        ],
        &[(2, 2)],
        json!([fault(1, "open", inductor)]),
    );
    let mut run = compile(rl);
    run.advance(20);
    assert_eq!(run.current(inductor), 0.0);
    assert_close(
        run.voltage(inductor),
        10.0,
        1e-9,
        "supply across the open inductor",
    );
}

#[test]
fn an_open_part_that_was_the_only_link_between_two_circuits_reports_zero_volts() {
    // Two supplies with no common node, joined only by a link resistor.
    let link = "00000000-0000-4000-8000-0000000000b1";
    let second = "00000000-0000-4000-8000-0000000000b2";
    let load_one = "00000000-0000-4000-8000-0000000000b3";
    let load_two = "00000000-0000-4000-8000-0000000000b4";
    let make = |faults: Value| {
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 5.0, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (0, 2),
                ),
                two_pin(
                    load_one,
                    "resistor",
                    json!({ "resistanceOhms": 100.0 }),
                    (0, 0),
                    (0, 2),
                ),
                two_pin(
                    second,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 3.0, "internalResistanceOhms": 0.0 }),
                    (10, 0),
                    (10, 2),
                ),
                two_pin(
                    load_two,
                    "resistor",
                    json!({ "resistanceOhms": 100.0 }),
                    (10, 0),
                    (10, 2),
                ),
                two_pin(
                    link,
                    "resistor",
                    json!({ "resistanceOhms": 1000.0 }),
                    (0, 0),
                    (10, 0),
                ),
            ],
            &[],
            faults,
        )
    };

    let open = compile(make(json!([fault(1, "open", link)])));

    assert_eq!(open.current(link), 0.0);
    assert_eq!(
        open.voltage(link),
        0.0,
        "different islands: nothing to compare"
    );
    assert_close(
        open.current(load_one),
        0.05,
        1e-12,
        "first circuit unaffected",
    );
    assert_close(
        open.current(load_two),
        0.03,
        1e-12,
        "second circuit unaffected",
    );
}

#[test]
fn a_pin_that_touches_nothing_else_reports_zero_volts_across_an_open_part() {
    let dangling = "00000000-0000-4000-8000-0000000000b5";
    let request = request(
        vec![
            two_pin(
                SOURCE,
                "dc_voltage_source",
                json!({ "voltageVolts": 5.0, "internalResistanceOhms": 0.0 }),
                (0, 0),
                (2, 2),
            ),
            two_pin(
                R1,
                "resistor",
                json!({ "resistanceOhms": 100.0 }),
                (0, 0),
                (2, 2),
            ),
            two_pin(
                dangling,
                "resistor",
                json!({ "resistanceOhms": 100.0 }),
                (0, 0),
                (20, 20),
            ),
        ],
        &[(2, 2)],
        json!([fault(1, "open", dangling)]),
    );
    let run = compile(request);

    assert_eq!(run.current(dangling), 0.0);
    assert_eq!(run.voltage(dangling), 0.0);
}

#[test]
fn a_live_update_of_an_overridden_parameter_changes_nothing_and_other_updates_keep_the_fault() {
    let mut run = compile(divider(json!([parameter_fault(
        1,
        R2,
        "resistanceOhms",
        3000.0
    )])));
    let before = run.snapshot.values.clone();

    let update =
        |run: &mut Compiled, component: &str, value: f64, revision: &str, expected: &str| {
            run.engine.set_parameter(SetParameterRequest {
                run_id: run.run_id.clone(),
                expected_document_revision: expected.to_owned(),
                document_revision: revision.to_owned(),
                component_id: id(component),
                parameter_id: open_circuit_engine::ParameterId::ResistanceOhms,
                value,
            })
        };

    // R2's value is the fault's: editing it succeeds and the results do not move.
    let SetParameterResult::Success { snapshot, .. } =
        update(&mut run, R2, 500.0, "rev-2", "rev-1")
    else {
        panic!("an update of an overridden parameter is accepted");
    };
    assert_eq!(snapshot.values, before);

    // R1 is not overridden: its edit works, with R2 still at the fault's 3 kOhm.
    let SetParameterResult::Success { snapshot, .. } =
        update(&mut run, R1, 2000.0, "rev-3", "rev-2")
    else {
        panic!("an update of another parameter works");
    };
    run.snapshot = snapshot;
    let parallel = 3000.0 * 1e7 / (3000.0 + 1e7);
    let reading = 10.0 * parallel / (2000.0 + parallel);
    assert_close(
        run.voltage(METER),
        reading,
        1e-9,
        "R1 = 2 kOhm with R2 faulted at 3 kOhm",
    );
}

#[test]
fn faulted_runs_are_deterministic_and_a_recompile_without_faults_is_healthy() {
    let faults = json!([
        fault(1, "open", R2),
        parameter_fault(2, R1, "resistanceOhms", 470.0)
    ]);
    let mut first = compile(divider(faults.clone()));
    let mut second = compile(divider(faults));
    first.advance(10);
    second.advance(10);

    assert_eq!(first.snapshot.values, second.snapshot.values);

    // The same document without faults is a different run with healthy values.
    let healthy = compile(divider(Value::Null));
    assert_ne!(healthy.run_id, first.run_id);
    assert_close(
        healthy.voltage(METER),
        4.999750012499375,
        1e-9,
        "healthy again",
    );
}

#[test]
fn the_catalog_lists_the_fault_kinds_of_every_type() {
    let catalog = component_catalog();
    let kinds = |type_id: &str| {
        catalog
            .component_types
            .iter()
            .find(|entry| serde_json::to_value(entry.type_id).unwrap() == json!(type_id))
            .unwrap_or_else(|| panic!("no type {type_id}"))
            .fault_kinds
            .clone()
    };
    let all = vec![FaultKind::Open, FaultKind::Short, FaultKind::Parameter];

    for passive in ["resistor", "capacitor", "inductor"] {
        assert_eq!(kinds(passive), all);
    }
    assert_eq!(
        kinds("dc_voltage_source"),
        vec![FaultKind::Open, FaultKind::Parameter]
    );
    for device in ["relay_spdt", "fuse", "pmdc_motor", "voltmeter", "ammeter"] {
        assert_eq!(kinds(device), vec![FaultKind::Parameter]);
    }
    for entry in &catalog.component_types {
        assert_eq!(entry.fault_kinds, supported_fault_kinds(entry.type_id));
    }
}

#[test]
fn the_compile_request_reads_and_writes_faults_as_the_contract_spells_them() {
    let faults = json!([
        fault(1, "open", R2),
        parameter_fault(2, R1, "resistanceOhms", 470.0)
    ]);
    let request = divider(faults.clone());
    let written = serde_json::to_value(&request).unwrap();

    assert_eq!(written["faults"], faults);
    // An absent list stays absent, so every earlier request keeps its exact meaning.
    assert!(
        serde_json::to_value(divider(Value::Null))
            .unwrap()
            .get("faults")
            .is_none()
    );

    // Unknown fields and unknown kinds are refused, never read as something else.
    let mut with_unknown = written.clone();
    with_unknown["faults"][0]["pin"] = json!("pin1");
    assert!(serde_json::from_value::<CompileRequest>(with_unknown).is_err());
    let mut with_kind = written;
    with_kind["faults"][0]["kind"] = json!("intermittent");
    assert!(serde_json::from_value::<CompileRequest>(with_kind).is_err());
}

fn relay_state(run: &Compiled) -> String {
    match run.snapshot.states.as_slice() {
        [open_circuit_engine::DeviceState::Relay { state, .. }] => {
            format!("{state:?}").to_lowercase()
        }
        other => panic!("one relay state expected, found {other:?}"),
    }
}

#[test]
fn faults_change_the_discrete_state_of_a_relay_through_the_circuit_they_leave() {
    let relay = "00000000-0000-4000-8000-0000000000d1";
    // 5 V, a 10 Ohm series resistor, and a 100 Ohm coil: 45.5 mA against a 40 mA pickup.
    let make = |faults: Value| {
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 5.0, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (2, 2),
                ),
                two_pin(
                    R1,
                    "resistor",
                    json!({ "resistanceOhms": 10.0 }),
                    (0, 0),
                    (4, 0),
                ),
                (
                    json!({
                        "typeId": "relay_spdt", "id": relay,
                        "position": { "x": 0, "y": 0 }, "rotationDeg": 0,
                        "parameters": {
                            "coilResistanceOhms": 100.0, "coilInductanceHenries": 0.01,
                            "pickupCurrentAmps": 0.04, "dropoutCurrentAmps": 0.02,
                            "closedContactResistanceOhms": 0.1
                        }
                    }),
                    json!({
                        "componentId": relay,
                        "pins": [
                            { "pinId": "coil_positive", "position": { "x": 4, "y": 0 } },
                            { "pinId": "coil_negative", "position": { "x": 2, "y": 2 } },
                            { "pinId": "common", "position": { "x": 20, "y": 0 } },
                            { "pinId": "normally_open", "position": { "x": 22, "y": 0 } },
                            { "pinId": "normally_closed", "position": { "x": 24, "y": 0 } },
                        ],
                    }),
                ),
            ],
            &[(2, 2)],
            faults,
        )
    };
    let state_after = |faults: Value| {
        let mut run = compile(make(faults));
        run.advance(5);
        (
            relay_state(&run),
            run.value(&format!("{relay}/coil/current")),
        )
    };

    let (healthy, current) = state_after(Value::Null);
    assert_eq!(healthy, "picked");
    assert_close(current, 5.0 / 110.0, 1e-9, "healthy coil current");

    // The series resistor open: no coil current at all, so the relay never picks.
    let (open, current) = state_after(json!([fault(1, "open", R1)]));
    assert_eq!(open, "released");
    assert_eq!(current, 0.0);

    // The series resistor shorted: the coil sees (almost) the full 5 V, 49.99 mA.
    let (shorted, current) = state_after(json!([fault(1, "short", R1)]));
    assert_eq!(shorted, "picked");
    assert_close(current, 5.0 / 100.001, 1e-9, "coil current with R1 shorted");

    // A weakened coil, as a parameter fault: the pickup is above what the coil carries.
    let (weak, _) = state_after(json!([parameter_fault(
        1,
        relay,
        "pickupCurrentAmps",
        0.05
    )]));
    assert_eq!(weak, "released");

    // A lower coil resistance picks it up with the series resistor in the way anyway.
    let (strong, _) = state_after(json!([parameter_fault(
        1,
        relay,
        "coilResistanceOhms",
        50.0
    )]));
    assert_eq!(strong, "picked");
}

#[test]
fn a_motor_parameter_fault_changes_the_operating_point() {
    let motor = "00000000-0000-4000-8000-0000000000e3";
    let make = |faults: Value| {
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (2, 2),
                ),
                two_pin(
                    motor,
                    "pmdc_motor",
                    json!({
                        "armatureResistanceOhms": 2.0, "armatureInductanceHenries": 0.1,
                        "motorConstantNewtonMetersPerAmp": 0.1,
                        "rotorInertiaKilogramMetersSquared": 0.01,
                        "viscousFrictionNewtonMeterSecondsPerRadian": 0.01,
                        "loadTorqueNewtonMeters": 0.0
                    }),
                    (0, 0),
                    (2, 2),
                ),
            ],
            &[(2, 2)],
            faults,
        )
    };
    let speed_after_eight_seconds = |faults: Value| {
        let mut run = compile(make(faults));
        run.advance(8000);
        run.value(&format!("{motor}/angular_velocity"))
    };

    // No load: about 33.33 rad/s (stage-4 fixture). A -0.05 N*m shaft torque: 30 rad/s.
    assert_close(
        speed_after_eight_seconds(Value::Null),
        33.33,
        0.01,
        "speed without a fault",
    );
    assert_close(
        speed_after_eight_seconds(json!([parameter_fault(
            1,
            motor,
            "loadTorqueNewtonMeters",
            -0.05
        )])),
        30.0,
        0.01,
        "speed with the load-torque fault",
    );
}

/// `a || b`, for the closed forms below.
fn parallel(a: f64, b: f64) -> f64 {
    a * b / (a + b)
}

/// The divider voltmeter reading with R1 and R2 given: `Vs * (R2 || Rm) / (R1 + (R2 || Rm))`.
fn reading(source: f64, r1: f64, r2: f64, meter: f64) -> f64 {
    let lower = parallel(r2, meter);
    source * lower / (r1 + lower)
}

#[test]
fn combined_faults_of_different_kinds_compose_into_the_closed_form_answer() {
    let rf = open_circuit_engine::SHORT_FAULT_RESISTANCE_OHMS;
    let cases: Vec<(&str, Value, f64)> = vec![
        (
            "R1 shorted and R2 drifted to 3 kOhm",
            json!([
                fault(1, "short", R1),
                parameter_fault(2, R2, "resistanceOhms", 3000.0)
            ]),
            reading(10.0, rf, 3000.0, 1e7),
        ),
        (
            "R1 drifted to 470 Ohm and R2 open",
            json!([
                parameter_fault(1, R1, "resistanceOhms", 470.0),
                fault(2, "open", R2)
            ]),
            10.0 * 1e7 / (470.0 + 1e7),
        ),
        (
            "R1 open and R2 shorted: no current can flow",
            json!([fault(1, "open", R1), fault(2, "short", R2)]),
            0.0,
        ),
        (
            "R1 shorted and R2 shorted",
            json!([fault(1, "short", R1), fault(2, "short", R2)]),
            reading(10.0, rf, rf, 1e7),
        ),
        (
            "the supply raised to 12 V with both resistors drifted",
            json!([
                parameter_fault(1, SOURCE, "voltageVolts", 12.0),
                parameter_fault(2, R1, "resistanceOhms", 2000.0),
                parameter_fault(3, R2, "resistanceOhms", 3000.0),
            ]),
            reading(12.0, 2000.0, 3000.0, 1e7),
        ),
        (
            "the meter input resistance drifted to 1 kOhm and R1 shorted",
            json!([
                parameter_fault(1, METER, "inputResistanceOhms", 1000.0),
                fault(2, "short", R1),
            ]),
            reading(10.0, rf, 1000.0, 1000.0),
        ),
    ];

    for (name, faults, expected) in cases {
        let run = compile(divider(faults));

        assert_close(run.voltage(METER), expected, 1e-9, name);
    }
}

#[test]
fn parameter_faults_of_dynamic_parts_and_the_source_follow_their_equations() {
    let capacitor = "00000000-0000-4000-8000-0000000000c1";
    let inductor = "00000000-0000-4000-8000-0000000000c2";
    let ammeter = "00000000-0000-4000-8000-0000000000c3";

    // RC: C drifted from 10 uF to 100 uF makes tau 100 ms: v(0.1 s) = 10 (1 - e^-1).
    let rc = request(
        vec![
            two_pin(
                SOURCE,
                "dc_voltage_source",
                json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                (0, 0),
                (2, 2),
            ),
            two_pin(
                R1,
                "resistor",
                json!({ "resistanceOhms": 1000.0 }),
                (0, 0),
                (4, 0),
            ),
            two_pin(
                capacitor,
                "capacitor",
                json!({ "capacitanceFarads": 1e-5, "initialVoltageVolts": 0.0 }),
                (4, 0),
                (2, 2),
            ),
        ],
        &[(2, 2)],
        json!([parameter_fault(1, capacitor, "capacitanceFarads", 1e-4)]),
    );
    let mut run = compile(rc);
    run.advance(100);
    assert_close(
        run.voltage(capacitor),
        10.0 * (1.0 - (-1.0_f64).exp()),
        0.01,
        "RC with the drifted C",
    );

    // RL: a shorted inductor is a 1 mOhm resistor, a drifted one changes the time constant.
    let rl = |faults: Value| {
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (2, 2),
                ),
                two_pin(
                    R1,
                    "resistor",
                    json!({ "resistanceOhms": 10.0 }),
                    (0, 0),
                    (4, 0),
                ),
                two_pin(
                    inductor,
                    "inductor",
                    json!({ "inductanceHenries": 0.1, "initialCurrentAmps": 0.0 }),
                    (4, 0),
                    (2, 2),
                ),
            ],
            &[(2, 2)],
            faults,
        )
    };
    let mut shorted = compile(rl(json!([fault(1, "short", inductor)])));
    shorted.advance(10);
    let rf = open_circuit_engine::SHORT_FAULT_RESISTANCE_OHMS;
    assert_close(
        shorted.current(inductor),
        10.0 / (10.0 + rf),
        1e-9,
        "shorted inductor",
    );
    let mut drifted = compile(rl(json!([parameter_fault(
        2,
        inductor,
        "inductanceHenries",
        1.0
    )])));
    drifted.advance(1000);
    // tau = L / R = 100 ms: i(1 s) = 1 A (1 - e^-10).
    assert_close(
        drifted.current(inductor),
        1.0 - (-10.0_f64).exp(),
        0.001,
        "RL with the drifted L",
    );

    // An ammeter in series with 1 kOhm: its burden drifted from 0.1 Ohm to 100 Ohm.
    let series = request(
        vec![
            two_pin(
                SOURCE,
                "dc_voltage_source",
                json!({ "voltageVolts": 10.0, "internalResistanceOhms": 0.0 }),
                (0, 0),
                (2, 2),
            ),
            two_pin(
                ammeter,
                "ammeter",
                json!({ "burdenResistanceOhms": 0.1 }),
                (0, 0),
                (4, 0),
            ),
            two_pin(
                R1,
                "resistor",
                json!({ "resistanceOhms": 1000.0 }),
                (4, 0),
                (2, 2),
            ),
        ],
        &[(2, 2)],
        json!([parameter_fault(1, ammeter, "burdenResistanceOhms", 100.0)]),
    );
    let run = compile(series);
    assert_close(
        run.current(ammeter),
        10.0 / 1100.0,
        1e-12,
        "ammeter reading",
    );
    assert_close(
        run.voltage(ammeter),
        100.0 * 10.0 / 1100.0,
        1e-9,
        "ammeter burden voltage",
    );
}

#[test]
fn a_fuse_trips_when_an_open_or_shorted_part_changes_the_current_through_it() {
    let fuse = "00000000-0000-4000-8000-0000000000e2";
    // 2.02 V, the 0.01 Ohm fuse, a 5 Ohm load (0.4 A: no trip within 2 ms), and a 1 kOhm
    // resistor in parallel with it.
    let make = |faults: Value| {
        request(
            vec![
                two_pin(
                    SOURCE,
                    "dc_voltage_source",
                    json!({ "voltageVolts": 2.02, "internalResistanceOhms": 0.0 }),
                    (0, 0),
                    (2, 2),
                ),
                two_pin(
                    fuse,
                    "fuse",
                    json!({ "intactResistanceOhms": 0.01, "tripI2tAmpereSquaredSeconds": 0.004 }),
                    (0, 0),
                    (4, 0),
                ),
                two_pin(
                    R1,
                    "resistor",
                    json!({ "resistanceOhms": 5.0 }),
                    (4, 0),
                    (2, 2),
                ),
                two_pin(
                    R2,
                    "resistor",
                    json!({ "resistanceOhms": 1000.0 }),
                    (4, 0),
                    (2, 2),
                ),
            ],
            &[(2, 2)],
            faults,
        )
    };
    let after = |faults: Value| {
        let mut run = compile(make(faults));
        run.advance(2);
        fuse_state(&run)
    };

    assert_eq!(after(Value::Null), "intact");
    // The 5 Ohm load shorted: about 2 A through the fuse, which trips in one step.
    assert_eq!(after(json!([fault(1, "short", R1)])), "tripped");
    // The 5 Ohm load open: only 2 mA flows, so the fuse stays intact.
    assert_eq!(after(json!([fault(1, "open", R1)])), "intact");
}
