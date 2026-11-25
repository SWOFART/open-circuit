//! Stage 14 LED, zener diode, N-channel MOSFET and operational amplifier (docs/contracts/
//! stage-14-more-parts.md, "Semiconductor and controlled sources", issue #211). Each circuit is
//! solved by the Engine and by an independent solution of the same law written here (a bisection
//! of the loop equation), and held to `1e-8 + 1e-3 * |expected|`. States must match exactly.

use open_circuit_engine::{
    CompileRequest, CompileResult, DeviceState, Engine, OperationalAmplifierState, ResultLayout,
    ResultSnapshot,
};
use serde_json::{Value, json};

const SOURCE: &str = "00000000-0000-4000-8000-000000001440";
const SECOND: &str = "00000000-0000-4000-8000-000000001441";
const PART: &str = "00000000-0000-4000-8000-000000001442";
const RESISTOR: &str = "00000000-0000-4000-8000-000000001443";
const FEEDBACK: &str = "00000000-0000-4000-8000-000000001444";
/// A parameter's id, default, minimum and maximum; and a part's pin ids with its parameters.
type ParameterSummary = (String, f64, f64, f64);
type PartSummary = (Vec<String>, Vec<ParameterSummary>);

const MARKER: &str = "00000000-0000-4000-8000-0000000014f0";

/// The thermal voltage of the Stage 7 diode model: 27 degrees Celsius.
const THERMAL_VOLTAGE: f64 = 0.02586492578632875;

fn component(id: &str, type_id: &str, parameters: Value) -> Value {
    json!({"id": id, "position": {"x": 0, "y": 0}, "rotationDeg": 0,
        "typeId": type_id, "parameters": parameters})
}

fn pins(id: &str, pins: &[(&str, (i32, i32))]) -> Value {
    json!({"componentId": id, "pins": pins.iter().map(|(pin, (x, y))| json!({
        "pinId": pin, "position": {"x": x, "y": y}})).collect::<Vec<_>>()})
}

fn request(components: Vec<Value>, geometry: Vec<Value>, ground: (i32, i32)) -> CompileRequest {
    let mut schematic: Value = serde_json::from_str(include_str!(
        "../../../tests/golden/stage-1-divider.schematic.json"
    ))
    .unwrap();
    schematic["components"] = Value::Array(components);
    schematic["wires"] = json!([]);
    schematic["referenceMarkers"] =
        json!([{"id": MARKER, "position": {"x": ground.0, "y": ground.1}}]);
    serde_json::from_value(json!({
        "schematic": schematic,
        "geometry": {"components": geometry},
        "documentRevision": "rev-1"
    }))
    .unwrap()
}

fn dc(id: &str, volts: f64) -> Value {
    component(
        id,
        "dc_voltage_source",
        json!({"voltageVolts": volts, "internalResistanceOhms": 0.0}),
    )
}

fn resistor(id: &str, ohms: f64) -> Value {
    component(id, "resistor", json!({"resistanceOhms": ohms}))
}

fn solve(request: CompileRequest) -> (ResultLayout, ResultSnapshot) {
    let result = Engine::default().compile(request);
    let CompileResult::Success {
        layout,
        initial_snapshot,
        ..
    } = result
    else {
        panic!("the circuit should compile: {result:?}");
    };
    (layout, initial_snapshot)
}

fn value(layout: &ResultLayout, snapshot: &ResultSnapshot, id: &str, suffix: &str) -> f64 {
    let key = format!("{id}/{suffix}");
    let slot = layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot {key}"));
    snapshot.values[slot.offset as usize]
}

fn close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-3 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

fn state_of(snapshot: &ResultSnapshot, id: &str) -> DeviceState {
    snapshot
        .states
        .iter()
        .find(|state| state.component_id().as_str() == id)
        .unwrap_or_else(|| panic!("no state for {id}"))
        .clone()
}

/// Bisection of a monotone increasing function on `[low, high]`.
fn root(mut low: f64, mut high: f64, f: impl Fn(f64) -> f64) -> f64 {
    for _ in 0..200 {
        let middle = 0.5 * (low + high);
        if f(middle) < 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    0.5 * (low + high)
}

/// `source - I R - V(I)` solved for the current of a series junction: `V` from the Shockley law
/// of the Stage 7 diode with `n` times the thermal voltage.
fn diode_current(source: f64, resistance: f64, ideality: f64) -> f64 {
    let thermal = ideality * THERMAL_VOLTAGE;
    root(0.0, source / resistance, |current| {
        // Increasing in I: I R + V(I) - source.
        current * resistance + thermal * (current / 1e-14).ln_1p() - source
    })
}

// ---- the LED ---------------------------------------------------------------------------------

fn led_circuit(source: f64) -> CompileRequest {
    request(
        vec![
            dc(SOURCE, source),
            resistor(RESISTOR, 1000.0),
            component(
                PART,
                "led",
                json!({"saturationCurrentAmps": 1e-14, "idealityFactor": 2.0,
                    "litCurrentAmps": 0.001}),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", (0, 0)), ("pin2", (0, 4))]),
            pins(RESISTOR, &[("pin1", (0, 0)), ("pin2", (4, 0))]),
            pins(PART, &[("p", (4, 0)), ("n", (0, 4))]),
        ],
        (0, 4),
    )
}

#[test]
fn an_led_follows_the_shockley_law_and_is_lit_from_its_lit_current() {
    for source in [3.0, 5.0, 9.0] {
        let (layout, snapshot) = solve(led_circuit(source));
        let current = diode_current(source, 1000.0, 2.0);
        close(
            value(&layout, &snapshot, PART, "currentAmps"),
            current,
            "LED current",
        );
        close(
            value(&layout, &snapshot, PART, "voltageVolts"),
            source - current * 1000.0,
            "LED voltage",
        );
        let expected = current >= 0.001;
        assert!(
            matches!(state_of(&snapshot, PART), DeviceState::Led { lit, .. } if lit == expected),
            "{source} V: current {current}"
        );
    }
    // A reverse source leaves a leakage far below the lit current.
    let (layout, snapshot) = solve(led_circuit(-5.0));
    assert!(value(&layout, &snapshot, PART, "currentAmps").abs() < 1e-9);
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::Led { lit: false, .. }
    ));
}

// ---- the zener diode -------------------------------------------------------------------------

fn zener_circuit(source: f64, breakdown: f64, breakdown_resistance: f64) -> CompileRequest {
    request(
        vec![
            dc(SOURCE, source),
            resistor(RESISTOR, 470.0),
            component(
                PART,
                "zener_diode",
                json!({"saturationCurrentAmps": 1e-14, "idealityFactor": 1.0,
                    "breakdownVolts": breakdown,
                    "breakdownResistanceOhms": breakdown_resistance}),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", (0, 0)), ("pin2", (0, 4))]),
            pins(RESISTOR, &[("pin1", (0, 0)), ("pin2", (4, 0))]),
            // The cathode faces the source: the zener is reverse biased.
            pins(PART, &[("p", (0, 4)), ("n", (4, 0))]),
        ],
        (0, 4),
    )
}

#[test]
fn a_zener_clamps_at_its_breakdown_voltage_with_the_series_resistance() {
    // 12 V through 470 Ohm into 5.1 V and 5 Ohm: I = (12 - 5.1) / (470 + 5).
    let (layout, snapshot) = solve(zener_circuit(12.0, 5.1, 5.0));
    let current = (12.0 - 5.1) / (470.0 + 5.0);
    close(
        value(&layout, &snapshot, PART, "currentAmps"),
        -current,
        "breakdown current",
    );
    close(
        value(&layout, &snapshot, PART, "voltageVolts"),
        -(5.1 + 5.0 * current),
        "breakdown voltage",
    );
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::ZenerDiode {
            in_breakdown: true,
            ..
        }
    ));
}

#[test]
fn a_zener_below_its_knee_blocks_and_forward_it_conducts_like_a_diode() {
    // 4 V is below the 5.1 V knee: only the leakage flows.
    let (layout, snapshot) = solve(zener_circuit(4.0, 5.1, 5.0));
    assert!(value(&layout, &snapshot, PART, "currentAmps").abs() < 1e-8);
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::ZenerDiode {
            in_breakdown: false,
            ..
        }
    ));

    // A reversed source drives the forward branch: 470 Ohm and a 1-ideality junction.
    let (layout, snapshot) = solve(zener_circuit(-5.0, 5.1, 5.0));
    let current = diode_current(5.0, 470.0, 1.0);
    close(
        value(&layout, &snapshot, PART, "currentAmps"),
        current,
        "forward current",
    );
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::ZenerDiode {
            in_breakdown: false,
            ..
        }
    ));
}

// ---- the MOSFET --------------------------------------------------------------------------------

/// The contract's channel for `vds >= 0`, written again here.
fn contract_channel(vgs: f64, vds: f64, threshold: f64, gain: f64) -> f64 {
    let overdrive = (vgs - threshold).max(0.0);
    if vds < overdrive {
        gain * (overdrive * vds - vds * vds / 2.0)
    } else {
        gain * overdrive * overdrive / 2.0
    }
}

fn mosfet_circuit(supply: f64, gate: f64, drain_resistance: f64) -> CompileRequest {
    request(
        vec![
            dc(SOURCE, supply),
            dc(SECOND, gate),
            resistor(RESISTOR, drain_resistance),
            component(
                PART,
                "mosfet_n",
                json!({"thresholdVolts": 2.0, "gainAmpsPerVoltSquared": 0.1}),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", (0, 0)), ("pin2", (0, 8))]),
            pins(SECOND, &[("pin1", (2, 4)), ("pin2", (0, 8))]),
            pins(RESISTOR, &[("pin1", (0, 0)), ("pin2", (6, 0))]),
            // Gate, drain and source: the source at the ground.
            pins(PART, &[("g", (2, 4)), ("d", (6, 0)), ("s", (0, 8))]),
        ],
        (0, 8),
    )
}

#[test]
fn a_mosfet_is_off_in_cutoff_in_the_triode_region_and_in_saturation() {
    // Cutoff: vgs = 1 V is below the 2 V threshold.
    let (layout, snapshot) = solve(mosfet_circuit(12.0, 1.0, 100.0));
    close(
        value(&layout, &snapshot, PART, "drainCurrentAmps"),
        0.0,
        "cutoff current",
    );
    close(
        value(&layout, &snapshot, PART, "drainSourceVoltageVolts"),
        12.0,
        "cutoff drain voltage",
    );
    close(
        value(&layout, &snapshot, PART, "gateSourceVoltageVolts"),
        1.0,
        "gate voltage",
    );
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::MosfetN {
            conducting: false,
            ..
        }
    ));

    // Triode: vgs = 10 V and 100 Ohm in the drain: solve 12 - vds = 100 * I(vds).
    let (layout, snapshot) = solve(mosfet_circuit(12.0, 10.0, 100.0));
    let vds = root(0.0, 12.0, |vds| {
        100.0 * contract_channel(10.0, vds, 2.0, 0.1) - (12.0 - vds)
    });
    assert!(vds < 8.0, "the reference is in the triode region");
    close(
        value(&layout, &snapshot, PART, "drainSourceVoltageVolts"),
        vds,
        "triode vds",
    );
    close(
        value(&layout, &snapshot, PART, "drainCurrentAmps"),
        contract_channel(10.0, vds, 2.0, 0.1),
        "triode current",
    );
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::MosfetN {
            conducting: true,
            ..
        }
    ));

    // Saturation: vgs = 4 V gives 0.1 * 2^2 / 2 = 0.2 A, which drops 2 V in 10 Ohm.
    let (layout, snapshot) = solve(mosfet_circuit(12.0, 4.0, 10.0));
    close(
        value(&layout, &snapshot, PART, "drainCurrentAmps"),
        0.2,
        "saturation current",
    );
    close(
        value(&layout, &snapshot, PART, "drainSourceVoltageVolts"),
        10.0,
        "saturation vds",
    );
}

#[test]
fn a_negative_drain_source_voltage_swaps_the_terminals_and_the_sign_of_the_current() {
    // The drain at -6 V through 10 Ohm: the channel runs from the source to the drain, and the
    // gate (5 V above the source) is 5 + |vds| above the drain.
    let (layout, snapshot) = solve(mosfet_circuit(-6.0, 5.0, 10.0));
    let swapped = |vds_magnitude: f64| -> f64 {
        // The drain is the source of the swapped channel: its gate voltage is 5 + vds_magnitude.
        contract_channel(5.0 + vds_magnitude, vds_magnitude, 2.0, 0.1)
    };
    let magnitude = root(0.0, 6.0, |m| 10.0 * swapped(m) - (6.0 - m));
    close(
        value(&layout, &snapshot, PART, "drainSourceVoltageVolts"),
        -magnitude,
        "reverse vds",
    );
    close(
        value(&layout, &snapshot, PART, "drainCurrentAmps"),
        -swapped(magnitude),
        "reverse current",
    );
}

// ---- the operational amplifier -------------------------------------------------------------------

fn follower(input: f64, load: f64) -> CompileRequest {
    request(
        vec![
            dc(SOURCE, input),
            resistor(RESISTOR, load),
            component(
                PART,
                "operational_amplifier",
                json!({"openLoopGain": 1000.0, "limitVolts": 12.0,
                    "outputResistanceOhms": 1.0}),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", (0, 0)), ("pin2", (0, 8))]),
            // plus at the source, minus and out joined (the follower), ref at the ground.
            pins(
                PART,
                &[
                    ("plus", (0, 0)),
                    ("minus", (6, 0)),
                    ("out", (6, 0)),
                    ("ref", (0, 8)),
                ],
            ),
            pins(RESISTOR, &[("pin1", (6, 0)), ("pin2", (0, 8))]),
        ],
        (0, 8),
    )
}

#[test]
fn an_amplifier_in_a_follower_tracks_its_input_with_the_open_loop_gain_error() {
    // Vout = k / (1 + k) Vin with k = A RL / (RL + Rout).
    let load = 1000.0;
    let gain = 1000.0 * load / (load + 1.0);
    let (layout, snapshot) = solve(follower(3.0, load));
    let expected = gain / (1.0 + gain) * 3.0;
    close(
        value(&layout, &snapshot, PART, "outputVoltageVolts"),
        expected,
        "follower output",
    );
    close(
        value(&layout, &snapshot, PART, "differentialVoltageVolts"),
        3.0 - expected,
        "differential input",
    );
    // A source delivering power has a negative current.
    close(
        value(&layout, &snapshot, PART, "outputCurrentAmps"),
        -expected / load,
        "output current",
    );
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::OperationalAmplifier {
            state: OperationalAmplifierState::Linear,
            ..
        }
    ));
}

#[test]
fn an_amplifier_without_feedback_clips_at_either_limit() {
    // 10 mV across a gain of 1000 asks for 10 V, still linear; 20 mV asks for 20 V and clips.
    for (input, expected_state, rail) in [
        (0.010, OperationalAmplifierState::Linear, 10.0),
        (0.020, OperationalAmplifierState::ClippedHigh, 12.0),
        (-0.020, OperationalAmplifierState::ClippedLow, -12.0),
    ] {
        let (layout, snapshot) = open_loop(input);
        // The minus input is at the ground, so the differential input is the source itself.
        close(
            value(&layout, &snapshot, PART, "differentialVoltageVolts"),
            input,
            "differential input",
        );
        // The 10 kOhm load divides the open-circuit output with the 1 Ohm output resistance.
        close(
            value(&layout, &snapshot, PART, "outputVoltageVolts"),
            rail * 10_000.0 / 10_001.0,
            "output voltage",
        );
        assert!(
            matches!(
                state_of(&snapshot, PART),
                DeviceState::OperationalAmplifier { state, .. } if state == expected_state
            ),
            "{input} V"
        );
    }
}

fn open_loop(input: f64) -> (ResultLayout, ResultSnapshot) {
    solve(request(
        vec![
            dc(SOURCE, input),
            resistor(RESISTOR, 10_000.0),
            component(
                PART,
                "operational_amplifier",
                json!({"openLoopGain": 1000.0, "limitVolts": 12.0,
                    "outputResistanceOhms": 1.0}),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", (0, 0)), ("pin2", (0, 8))]),
            // minus and ref at the ground; the output into the load.
            pins(
                PART,
                &[
                    ("plus", (0, 0)),
                    ("minus", (0, 8)),
                    ("out", (6, 0)),
                    ("ref", (0, 8)),
                ],
            ),
            pins(RESISTOR, &[("pin1", (6, 0)), ("pin2", (0, 8))]),
        ],
        (0, 8),
    ))
}

#[test]
fn an_inverting_amplifier_sets_its_gain_with_the_feedback_resistors() {
    // Vin through 1 kOhm to the minus input, 10 kOhm from the output back to it, plus grounded:
    // the closed-loop gain is -10 (to within 1 / A).
    let (layout, snapshot) = solve(request(
        vec![
            dc(SOURCE, 0.5),
            resistor(RESISTOR, 1_000.0),
            resistor(FEEDBACK, 10_000.0),
            component(
                PART,
                "operational_amplifier",
                json!({"openLoopGain": 100000.0, "limitVolts": 12.0,
                    "outputResistanceOhms": 1.0}),
            ),
        ],
        vec![
            pins(SOURCE, &[("pin1", (0, 0)), ("pin2", (0, 8))]),
            pins(RESISTOR, &[("pin1", (0, 0)), ("pin2", (4, 2))]),
            pins(FEEDBACK, &[("pin1", (4, 2)), ("pin2", (8, 0))]),
            pins(
                PART,
                &[
                    ("plus", (0, 8)),
                    ("minus", (4, 2)),
                    ("out", (8, 0)),
                    ("ref", (0, 8)),
                ],
            ),
        ],
        (0, 8),
    ));
    let output = value(&layout, &snapshot, PART, "outputVoltageVolts");
    assert!((output + 5.0).abs() < 5.0 * 2e-3, "output {output} V");
    assert!(matches!(
        state_of(&snapshot, PART),
        DeviceState::OperationalAmplifier {
            state: OperationalAmplifierState::Linear,
            ..
        }
    ));
}

// ---- catalog and inputs --------------------------------------------------------------------------

#[test]
fn the_catalog_lists_the_four_parts_with_the_contract_pins_defaults_and_ranges() {
    let catalog = open_circuit_engine::component_catalog();
    let entry = |type_id: &str| {
        catalog
            .component_types
            .iter()
            .find(|entry| serde_json::to_value(entry.type_id).unwrap() == json!(type_id))
            .unwrap_or_else(|| panic!("no {type_id}"))
    };
    let summary = |type_id: &str| -> PartSummary {
        let entry = entry(type_id);
        assert!(entry.fault_kinds.is_empty());
        (
            entry
                .pins
                .iter()
                .map(|pin| pin.id.as_str().to_owned())
                .collect(),
            entry
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
                        parameter.minimum_value.unwrap(),
                        parameter.maximum_value.unwrap(),
                    )
                })
                .collect(),
        )
    };
    let named = |pairs: &[(&str, f64, f64, f64)]| -> Vec<ParameterSummary> {
        pairs
            .iter()
            .map(|&(key, default, low, high)| (key.to_owned(), default, low, high))
            .collect()
    };
    let strings = |pins: &[&str]| -> Vec<String> { pins.iter().map(|&p| p.to_owned()).collect() };

    assert_eq!(
        summary("led"),
        (
            strings(&["p", "n"]),
            named(&[
                ("saturationCurrentAmps", 1e-14, 1e-18, 1e-3),
                ("idealityFactor", 2.0, 1.0, 4.0),
                ("litCurrentAmps", 1e-3, 1e-6, 1.0),
            ])
        )
    );
    assert_eq!(
        summary("zener_diode"),
        (
            strings(&["p", "n"]),
            named(&[
                ("saturationCurrentAmps", 1e-14, 1e-18, 1e-3),
                ("idealityFactor", 1.0, 1.0, 4.0),
                ("breakdownVolts", 5.1, 0.1, 1000.0),
                ("breakdownResistanceOhms", 5.0, 0.001, 1e6),
            ])
        )
    );
    assert_eq!(
        summary("mosfet_n"),
        (
            strings(&["g", "d", "s"]),
            named(&[
                ("thresholdVolts", 2.0, 0.1, 20.0),
                ("gainAmpsPerVoltSquared", 0.1, 1e-6, 100.0),
            ])
        )
    );
    assert_eq!(
        summary("operational_amplifier"),
        (
            strings(&["plus", "minus", "out", "ref"]),
            named(&[
                ("openLoopGain", 1000.0, 1.0, 1e6),
                ("limitVolts", 12.0, 0.1, 1000.0),
                ("outputResistanceOhms", 1.0, 1e-6, 1e6),
            ])
        )
    );
}

#[test]
fn out_of_range_inputs_are_refused_before_any_run() {
    let bad: Vec<Value> = vec![
        // The ideality factor of the LED is at most 4.
        json!({"type": "led", "parameters": {"saturationCurrentAmps": 1e-14,
            "idealityFactor": 4.5, "litCurrentAmps": 0.001}}),
        json!({"type": "zener_diode", "parameters": {"saturationCurrentAmps": 1e-14,
            "idealityFactor": 1.0, "breakdownVolts": 0.05, "breakdownResistanceOhms": 5.0}}),
        json!({"type": "mosfet_n", "parameters": {"thresholdVolts": 25.0,
            "gainAmpsPerVoltSquared": 0.1}}),
        json!({"type": "operational_amplifier", "parameters": {"openLoopGain": 0.5,
            "limitVolts": 12.0, "outputResistanceOhms": 1.0}}),
    ];
    for case in bad {
        let type_id = case["type"].as_str().unwrap();
        let pin_names: &[&str] = match type_id {
            "mosfet_n" => &["g", "d", "s"],
            "operational_amplifier" => &["plus", "minus", "out", "ref"],
            _ => &["p", "n"],
        };
        let positions = [(0, 0), (4, 0), (0, 4), (4, 4)];
        let part_pins: Vec<(&str, (i32, i32))> = pin_names
            .iter()
            .zip(positions)
            .map(|(&name, position)| (name, position))
            .collect();
        let result = Engine::default().compile(request(
            vec![component(PART, type_id, case["parameters"].clone())],
            vec![pins(PART, &part_pins)],
            (0, 0),
        ));
        assert!(
            matches!(result, CompileResult::Failure { .. }),
            "{type_id} with {case} should be refused"
        );
    }
}
