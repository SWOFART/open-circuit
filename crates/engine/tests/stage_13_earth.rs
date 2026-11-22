//! Stage 13 protective earth and earth faults (issue #197, docs/contracts/
//! stage-13-protection-and-safety.md sections 2, 3, 7.1 and 8): the earth electrode, the exposed
//! part, the `leak` fault, and the two compile-time warnings. The known answers are the loop
//! equations of the contract's section 11.

use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, DiagnosticReference, DiagnosticSeverity, Engine,
    FaultKind, ResultLayout, ResultSnapshot, StableId, component_catalog,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const ELECTRODE: &str = "00000000-0000-4000-8000-000000000002";
const FRAME: &str = "00000000-0000-4000-8000-000000000003";
const METER: &str = "00000000-0000-4000-8000-000000000004";
const LEAK: &str = "00000000-0000-4000-8000-0000000000a1";

const EMPTY_DOCUMENTATION: &str = r#"{"designators":[],"wireNumbers":[],"terminals":[],"notes":[],
    "titleBlock":{"title":"","drawingNumber":"","revision":"","author":"","company":"","date":""},
    "sheets":[]}"#;

fn part(id: &str, type_id: &str, parameters: Value, pins: &[(&str, (i32, i32))]) -> (Value, Value) {
    (
        json!({
            "typeId": type_id, "id": id, "position": {"x": 0, "y": 0},
            "rotationDeg": 0, "parameters": parameters,
        }),
        json!({
            "componentId": id,
            "pins": pins.iter().map(|(pin, (x, y))| json!({
                "pinId": pin, "position": {"x": x, "y": y},
            })).collect::<Vec<_>>(),
        }),
    )
}

/// The source `E` with internal resistance `rs`, positive pin at (0, 0) and negative pin at
/// (2, 2). A voltmeter reads the frame (10, 0) against the negative pin.
fn source(volts: f64, rs: f64) -> (Value, Value) {
    part(
        SOURCE,
        "dc_voltage_source",
        json!({"voltageVolts": volts, "internalResistanceOhms": rs}),
        &[("pin1", (0, 0)), ("pin2", (2, 2))],
    )
}

fn electrode(re: f64, at: (i32, i32)) -> (Value, Value) {
    part(
        ELECTRODE,
        "earth_electrode",
        json!({"earthResistanceOhms": re}),
        &[("pe", at)],
    )
}

fn exposed(at: (i32, i32)) -> (Value, Value) {
    part(
        FRAME,
        "exposed_part",
        json!({"frequencyHz": 50.0, "touchVoltageLimitVolts": 50.0}),
        &[("ep", at)],
    )
}

fn voltmeter() -> (Value, Value) {
    part(
        METER,
        "voltmeter",
        json!({"inputResistanceOhms": 1e12}),
        &[("pin1", (10, 0)), ("pin2", (2, 2))],
    )
}

fn request(parts: Vec<(Value, Value)>, markers: &[(i32, i32)], faults: Value) -> CompileRequest {
    let components: Vec<Value> = parts
        .iter()
        .map(|(component, _)| component.clone())
        .collect();
    let geometry: Vec<Value> = parts.iter().map(|(_, pins)| pins.clone()).collect();
    let reference_markers: Vec<Value> = markers
        .iter()
        .enumerate()
        .map(|(n, (x, y))| {
            json!({"id": format!("00000000-0000-4000-8000-0000000000f{n}"),
                "position": {"x": x, "y": y}})
        })
        .collect();
    let mut value = json!({
        "schematic": {
            "schemaVersion": 3,
            "components": components, "wires": [], "junctions": [],
            "referenceMarkers": reference_markers, "rails": [],
            "documentation": serde_json::from_str::<Value>(EMPTY_DOCUMENTATION).unwrap(),
        },
        "geometry": {"components": geometry},
        "documentRevision": "rev-1",
    });
    if !faults.is_null() {
        value["faults"] = faults;
    }
    serde_json::from_value(value).expect("a compile request")
}

fn leak(value: f64) -> Value {
    json!([{"id": LEAK, "kind": "leak", "componentId": SOURCE,
        "pinId": "pin1", "frameId": FRAME, "value": value}])
}

struct Run {
    layout: ResultLayout,
    snapshot: ResultSnapshot,
    diagnostics: Vec<(DiagnosticCode, DiagnosticSeverity, Vec<String>)>,
}

fn compile(request: CompileRequest) -> Run {
    let CompileResult::Success {
        layout,
        initial_snapshot,
        diagnostics,
        ..
    } = Engine::default().compile(request)
    else {
        panic!("the circuit should compile");
    };
    Run {
        layout,
        snapshot: initial_snapshot,
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| {
                let components = diagnostic
                    .references
                    .iter()
                    .filter_map(|reference| match reference {
                        DiagnosticReference::Component { component_id } => {
                            Some(component_id.as_str().to_owned())
                        }
                        _ => None,
                    })
                    .collect();
                (diagnostic.code, diagnostic.severity, components)
            })
            .collect(),
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

    fn frame_volts(&self) -> f64 {
        self.value(&format!("{METER}:voltage"))
    }

    fn source_amps(&self) -> f64 {
        self.value(&format!("{SOURCE}:current")).abs()
    }

    fn has(&self, code: DiagnosticCode) -> bool {
        self.diagnostics.iter().any(|(found, _, _)| *found == code)
    }
}

fn assert_close(actual: f64, expected: f64, relative: f64, absolute: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= absolute + relative * expected.abs(),
        "{what}: {actual} is not within {absolute} + {relative}*|x| of {expected}"
    );
}

fn failure(request: CompileRequest) -> Vec<DiagnosticCode> {
    let CompileResult::Failure { diagnostics } = Engine::default().compile(request) else {
        panic!("the circuit should not compile");
    };
    diagnostics.into_iter().map(|d| d.code).collect()
}

/// A 100 V source with 1 ohm internal resistance, a 9 ohm electrode bonded to the frame (their
/// pins share the point (10, 0)), the ground marker on the source's negative pin.
fn bonded(faults: Value) -> CompileRequest {
    request(
        vec![
            source(100.0, 1.0),
            electrode(9.0, (10, 0)),
            exposed((10, 0)),
            voltmeter(),
        ],
        &[(2, 2)],
        faults,
    )
}

#[test]
fn a_leak_to_a_bonded_frame_drives_the_loop_current_through_the_electrode() {
    let run = compile(bonded(leak(10.0)));

    // I = E / (Rs + Rf + Re) = 100 / 20 = 5 A; the frame sits at Re * I = 45 V.
    assert_close(run.source_amps(), 5.0, 1e-6, 1e-9, "the leakage current");
    assert_close(run.frame_volts(), 45.0, 1e-6, 1e-9, "the frame voltage");
}

#[test]
fn the_loop_follows_the_resistances() {
    for (rf, re) in [(0.001, 0.001), (100.0, 1.0), (1000.0, 100.0), (5.0, 50.0)] {
        let request = request(
            vec![
                source(230.0, 0.5),
                electrode(re, (10, 0)),
                exposed((10, 0)),
                voltmeter(),
            ],
            &[(2, 2)],
            leak(rf),
        );
        let run = compile(request);
        let current = 230.0 / (0.5 + rf + re);

        assert_close(run.source_amps(), current, 1e-6, 1e-9, "current");
        assert_close(run.frame_volts(), re * current, 1e-6, 1e-9, "frame voltage");
    }
}

#[test]
fn an_unbonded_frame_floats_to_the_leaking_pin_and_carries_no_current() {
    let request = request(
        vec![
            source(100.0, 1.0),
            electrode(9.0, (20, 0)),
            exposed((10, 0)),
            voltmeter(),
        ],
        &[(2, 2)],
        leak(10.0),
    );
    let run = compile(request);

    // The frame is tied to the world only by the exposed part's 1 gigaohm: a defined voltage, not
    // a floating net (contract 7.1).
    let expected = 100.0 * (1.0 - (1.0 + 10.0) / 1.0e9);
    assert_close(run.frame_volts(), expected, 1e-6, 0.0, "the unbonded frame");
    assert!(run.source_amps() < 1e-6, "{}", run.source_amps());
}

#[test]
fn without_a_fault_the_frame_stays_at_earth_and_nothing_flows() {
    let run = compile(bonded(Value::Null));

    assert_close(run.frame_volts(), 0.0, 0.0, 1e-9, "the frame voltage");
    assert_close(run.source_amps(), 0.0, 0.0, 1e-9, "the source current");
    assert!(!run.has(DiagnosticCode::NoEarthPath));
    assert!(!run.has(DiagnosticCode::ExposedPartNotBonded));
}

#[test]
fn an_electrode_alone_gives_a_drawing_its_earth_node_without_a_ground_marker() {
    // The source's negative pin is bonded to the electrode: the island's reference is the earth
    // node, so the circuit solves with no marker and no NoGroundReference warning.
    let run = compile(request(
        vec![source(10.0, 1.0), electrode(5.0, (2, 2))],
        &[],
        Value::Null,
    ));

    assert!(!run.has(DiagnosticCode::NoGroundReference));
    assert_close(
        run.source_amps(),
        0.0,
        0.0,
        1e-9,
        "no closed loop, no current",
    );
}

#[test]
fn an_exposed_part_with_no_earth_or_no_bond_is_advised_not_refused() {
    // No marker and no electrode: the island has no earth node, and the frame is not bonded.
    let alone = compile(request(vec![exposed((10, 0))], &[], Value::Null));
    assert!(alone.has(DiagnosticCode::NoEarthPath));
    assert!(alone.has(DiagnosticCode::ExposedPartNotBonded));
    assert!(
        alone
            .diagnostics
            .iter()
            .all(|(_, severity, _)| *severity == DiagnosticSeverity::Warning
                || *severity == DiagnosticSeverity::Info),
        "{:?}",
        alone.diagnostics
    );

    // A leak joins the frame to the earthed island, so only the missing bond remains.
    let leaking = compile(request(
        vec![source(100.0, 1.0), exposed((10, 0)), voltmeter()],
        &[(2, 2)],
        leak(10.0),
    ));
    assert!(!leaking.has(DiagnosticCode::NoEarthPath));
    assert!(leaking.has(DiagnosticCode::ExposedPartNotBonded));

    // Bonded to an electrode in an earthed island: neither warning.
    let bonded = compile(bonded(Value::Null));
    assert!(!bonded.has(DiagnosticCode::NoEarthPath));
    assert!(!bonded.has(DiagnosticCode::ExposedPartNotBonded));
}

#[test]
fn a_malformed_leak_is_an_invalid_fault() {
    let base = |fault: Value| bonded(json!([fault]));
    let good = || {
        json!({"id": LEAK, "kind": "leak", "componentId": SOURCE,
            "pinId": "pin1", "frameId": FRAME, "value": 10.0})
    };
    let with = |key: &str, value: Value| {
        let mut fault = good();
        fault[key] = value;
        fault
    };
    let without = |key: &str| {
        let mut fault = good();
        fault.as_object_mut().unwrap().remove(key);
        fault
    };

    assert!(matches!(
        Engine::default().compile(base(good())),
        CompileResult::Success { .. }
    ));
    for (what, fault) in [
        ("a pin the part does not have", with("pinId", json!("l1"))),
        (
            "a frame that is not an exposed part",
            with("frameId", json!(ELECTRODE)),
        ),
        (
            "an unknown frame",
            with("frameId", json!("00000000-0000-4000-8000-0000000000ee")),
        ),
        ("a value below the floor", with("value", json!(0.0001))),
        ("a value above the ceiling", with("value", json!(2.0e9))),
        ("a missing value", without("value")),
        ("a missing pin", without("pinId")),
        ("a missing frame", without("frameId")),
        (
            "a parameter id",
            with("parameterId", json!("resistanceOhms")),
        ),
        ("the ID of a component", with("id", json!(SOURCE))),
    ] {
        assert_eq!(
            failure(base(fault)),
            vec![DiagnosticCode::InvalidFault],
            "{what}"
        );
    }

    // The other kinds refuse the leak's fields.
    let open_with_pin = json!({"id": LEAK, "kind": "open", "componentId": SOURCE, "pinId": "pin1"});
    assert_eq!(
        failure(base(open_with_pin)),
        vec![DiagnosticCode::InvalidFault]
    );
    // One fault per component, as in Stage 5.
    let twice = json!([good(), {"id": "00000000-0000-4000-8000-0000000000a2", "kind": "open",
        "componentId": SOURCE}]);
    assert_eq!(failure(bonded(twice)), vec![DiagnosticCode::InvalidFault]);
}

#[test]
fn a_leak_is_listed_only_for_the_types_the_contract_names() {
    let allowed = [
        "resistor",
        "inductor",
        "capacitor",
        "dc_voltage_source",
        "ac_voltage_source",
        "three_phase_source_star",
        "three_phase_source_delta",
        "three_phase_load_star",
        "three_phase_load_delta",
        "transformer_three_phase",
        "induction_motor_three_phase",
        "pmdc_motor",
        "contactor_three_pole",
        "breaker_three_pole",
        "thermal_overload_three_pole",
    ];
    for entry in &component_catalog().component_types {
        let name = serde_json::to_value(entry.type_id).unwrap();
        assert_eq!(
            entry.fault_kinds.contains(&FaultKind::Leak),
            allowed.contains(&name.as_str().unwrap()),
            "{name}"
        );
    }
}

#[test]
fn the_earth_devices_are_in_the_catalog_with_the_contract_pins_and_bounds() {
    let catalog = component_catalog();
    let entry = |name: &str| {
        catalog
            .component_types
            .iter()
            .find(|entry| serde_json::to_value(entry.type_id).unwrap() == json!(name))
            .unwrap_or_else(|| panic!("no type {name}"))
    };

    let electrode = entry("earth_electrode");
    assert_eq!(
        serde_json::to_value(&electrode.pins).unwrap()[0]["id"],
        "pe"
    );
    let resistance = &electrode.parameters[0];
    assert_eq!(
        (
            resistance.default_value,
            resistance.minimum_value,
            resistance.maximum_value
        ),
        (10.0, Some(0.001), Some(1_000_000.0))
    );

    let part = entry("exposed_part");
    assert_eq!(serde_json::to_value(&part.pins).unwrap()[0]["id"], "ep");
    let bounds: Vec<_> = part
        .parameters
        .iter()
        .map(|p| (p.default_value, p.minimum_value, p.maximum_value))
        .collect();
    assert_eq!(
        bounds,
        vec![
            (50.0, Some(1.0), Some(400.0)),
            (50.0, Some(1.0), Some(1000.0))
        ]
    );
    assert!(StableId::parse(LEAK).is_ok());
}
