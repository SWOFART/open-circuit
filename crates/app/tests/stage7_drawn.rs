//! Issues #114 and #115: the semiconductor circuits that the editor commands and the real symbol
//! geometry draw from blank documents (`src/store/stage7Drawing.test.ts`) run on the real Engine
//! behind `SimulationCore`. Their frames agree with the answers of the model equations in
//! `tests/golden/stage-7-*` (never Engine output) within the manifest's tolerances; repeated
//! runs agree bit for bit; live edits, a Stage 6 switch, a capacitor, refused edits, compile
//! failures and a saved and reopened project behave as the contract says.

mod common;

use common::{Run, assert_close};
use open_circuit_app_lib::project::{read_project_file, write_project_file};
use open_circuit_app_lib::simulation::{SimulationConfig, SimulationCore};
use open_circuit_engine::{
    CompileRequest, CompileResult, DiagnosticCode, DiagnosticReference, OperatorPosition,
    ParameterId, SetOperatorStateResult, SetParameterResult, StableId,
};
use serde_json::{Value, json};

macro_rules! drawn {
    ($name:literal) => {
        include_str!(concat!(
            "../../../tests/golden/stage-7-drawn-",
            $name,
            ".request.json"
        ))
    };
}

const DIODE_FORWARD: &str = drawn!("diode-forward");
const DIODE_REVERSE: &str = drawn!("diode-reverse");
const DIODE_INDUCTOR: &str = drawn!("diode-inductor");
const DIODE_IDEAL_SOURCE: &str = drawn!("diode-ideal-source");
const NPN_CUTOFF: &str = drawn!("npn-cutoff");
const NPN_ACTIVE: &str = drawn!("npn-active");
const NPN_SATURATION: &str = drawn!("npn-saturation");
const NPN_REVERSE_ACTIVE: &str = drawn!("npn-reverse-active");
const NPN_IDEAL_SOURCE: &str = drawn!("npn-ideal-source");
const PNP_CUTOFF: &str = drawn!("pnp-cutoff");
const PNP_ACTIVE: &str = drawn!("pnp-active");
const PNP_SATURATION: &str = drawn!("pnp-saturation");
const RC_CHARGING: &str = drawn!("rc-diode-charging");
const RC_REVERSED: &str = drawn!("rc-diode-reversed");
const ANALYTICAL: &str = include_str!("../../../tests/golden/stage-7-analytical.expected.json");
const MANIFEST: &str = include_str!("../../../tests/golden/stage-7-manifest.json");

fn num(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

fn answer(id: &str) -> Value {
    let all: Value = serde_json::from_str(ANALYTICAL).unwrap();
    all["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))["contract"]
        .clone()
}

fn tolerance(id: &str) -> Value {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    manifest["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fixture| fixture["id"] == id)
        .unwrap_or_else(|| panic!("no fixture {id}"))["engineTolerance"]
        .clone()
}

fn close(actual: f64, expected: f64, limits: &Value, volts: bool, what: &str) {
    if volts {
        assert_close(actual, expected, num(&limits["voltsAbsolute"]), what);
    } else {
        let relative = limits.get("ampsRelative").map_or(0.0, num);
        let absolute = limits
            .get("ampsAbsolute")
            .or_else(|| limits.get("ampsAbsoluteFloor"))
            .map_or(0.0, num);
        assert_close(
            actual,
            expected,
            relative * actual.abs().max(expected.abs()) + absolute,
            what,
        );
    }
}

fn request(text: &str) -> CompileRequest {
    serde_json::from_str(text).expect("a drawn compile request")
}

/// The identifier of the one component of a type in a drawn request.
fn device(request: &CompileRequest, type_id: &str) -> String {
    let json = serde_json::to_value(request).unwrap();
    json["schematic"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| component["typeId"] == type_id)
        .unwrap_or_else(|| panic!("no {type_id}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn value(run: &Run, step: u64, key: &str) -> f64 {
    let (frame, _bytes) = run.frame_at(step);
    let slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("no slot {key}"));
    frame.values[slot.offset as usize]
}

fn diode_case(text: &str, id: &str) {
    let request = request(text);
    let diode = device(&request, "diode");
    let mut run = Run::start_every_step(request);
    let step = run.advance(5);
    let expected = answer(id);
    let limits = tolerance(id);

    close(
        value(&run, step, &format!("{diode}:voltage")),
        num(&expected["diodeVoltageVolts"]),
        &limits,
        true,
        "diode voltage",
    );
    close(
        value(&run, step, &format!("{diode}:current")),
        num(&expected["diodeCurrentAmps"]),
        &limits,
        false,
        "diode current",
    );
}

/// The five terminal quantities of the drawn transistor against the answer of the equations.
/// With `exchanged` the load hangs on the emitter and the collector is the reference, so the
/// junction voltages are measured from the load node, as the reference's conventions say.
fn transistor_case(text: &str, type_id: &str, id: &str, exchanged: bool) -> [f64; 5] {
    let request = request(text);
    let transistor = device(&request, type_id);
    let mut run = Run::start_every_step(request);
    let step = run.advance(5);
    let expected = answer(id);
    let limits = tolerance(id);
    let read = |name: &str| value(&run, step, &format!("{transistor}/{name}"));
    let load = num(&expected["loadNodeVoltageVolts"]);
    let (emitter, collector) = if exchanged { (load, 0.0) } else { (0.0, load) };

    close(
        read("base_emitter/voltage"),
        num(&expected["baseVoltageVolts"]) - emitter,
        &limits,
        true,
        "V_BE",
    );
    close(
        read("collector_emitter/voltage"),
        collector - emitter,
        &limits,
        true,
        "V_CE",
    );
    for (key, name, what) in [
        ("collector/current", "collectorCurrentAmps", "i_C"),
        ("base/current", "baseCurrentAmps", "i_B"),
        ("emitter/current", "emitterCurrentAmps", "i_E"),
    ] {
        close(read(key), num(&expected[name]), &limits, false, what);
    }

    [
        read("base_emitter/voltage"),
        read("collector_emitter/voltage"),
        read("collector/current"),
        read("base/current"),
        read("emitter/current"),
    ]
}

#[test]
fn the_drawn_diodes_agree_with_the_model_equations_forward_and_reverse() {
    diode_case(DIODE_FORWARD, "diode-forward");
    diode_case(DIODE_REVERSE, "diode-reverse");
}

#[test]
fn the_drawn_transistors_agree_with_the_model_equations_in_every_bias_region() {
    let npn_cutoff = transistor_case(NPN_CUTOFF, "bjt_npn", "npn-cutoff", false);
    let npn_active = transistor_case(NPN_ACTIVE, "bjt_npn", "npn-active", false);
    let npn_saturation = transistor_case(NPN_SATURATION, "bjt_npn", "npn-saturation", false);
    let pnp_cutoff = transistor_case(PNP_CUTOFF, "bjt_pnp", "pnp-cutoff", false);
    let pnp_active = transistor_case(PNP_ACTIVE, "bjt_pnp", "pnp-active", false);
    let pnp_saturation = transistor_case(PNP_SATURATION, "bjt_pnp", "pnp-saturation", false);

    // The three regions differ in what the Engine reports (nothing here infers a region): the
    // collector-emitter voltage and the current gain.
    assert!(npn_cutoff[1] > 9.99 && npn_cutoff[2].abs() < 1e-9);
    assert!(npn_active[1] > 5.0 && npn_active[1] < 9.5);
    assert!(npn_saturation[1] < 0.3);
    let gain = |case: &[f64; 5]| case[2] / case[3];
    assert!((gain(&npn_active) - 100.0).abs() < 1.0, "active gain");
    assert!(gain(&npn_saturation) < 20.0, "saturated gain");

    // The PNP is the NPN with every voltage and current negated.
    for (npn, pnp) in [
        (npn_cutoff, pnp_cutoff),
        (npn_active, pnp_active),
        (npn_saturation, pnp_saturation),
    ] {
        for (a, b) in npn.iter().zip(pnp.iter()) {
            assert_close(*a, -*b, 1e-9 * a.abs().max(1e-3), "PNP mirror");
        }
    }
}

#[test]
fn the_wrong_wiring_is_told_apart_from_the_correct_stage() {
    // The collector and emitter exchanged: reverse active, a different answer from the same parts.
    let wrong = transistor_case(NPN_REVERSE_ACTIVE, "bjt_npn", "npn-reverse-active", true);
    let right = transistor_case(NPN_ACTIVE, "bjt_npn", "npn-active", false);

    assert!(
        wrong[2].abs() < right[2].abs() / 20.0,
        "the exchanged stage carries a small reverse collector current: {wrong:?} vs {right:?}"
    );
    assert!(
        wrong[2] < 0.0 && right[2] > 0.0,
        "opposite collector currents"
    );
    assert!((wrong[0] - right[0]).abs() > 0.1, "different V_BE");
}

/// The nonlinear RC of the `rc-diode-*` fixtures: the switch closes after step 4 and the
/// capacitor voltage is read at the contract's times.
fn rc_timeline(text: &str, id: &str, volts_tolerance: f64) -> Vec<(u64, f64)> {
    let request = request(text);
    let switch = device(&request, "switch_spst");
    let capacitor = device(&request, "capacitor");
    let mut run = Run::start_every_step(request);
    let mut observed = Vec::new();
    run.advance(4);
    let (_, result) = run.set_operator_state(&switch, OperatorPosition::Closed);
    assert!(matches!(result, SetOperatorStateResult::Success { .. }));

    let expected = answer(id);
    for (time, volts) in expected["capacitorVoltsAtSeconds"].as_object().unwrap() {
        let step = (time.parse::<f64>().unwrap() * 1000.0).round() as u64;
        run.advance(step as u32 - run.delivered.last().unwrap().0.metadata.simulation_step as u32);
        let actual = value(&run, step, &format!("{capacitor}:voltage"));
        assert_close(
            actual,
            num(volts),
            volts_tolerance,
            &format!("{id} at {time} s"),
        );
        observed.push((step, actual));
    }

    observed
}

#[test]
fn the_drawn_nonlinear_rc_follows_the_reference_and_the_reversed_diode_does_not() {
    let limits = |id: &str| num(&tolerance(id)["voltsAbsolute"]);
    let charging = rc_timeline(
        RC_CHARGING,
        "rc-diode-charging",
        limits("rc-diode-charging"),
    );
    let reversed = rc_timeline(
        RC_REVERSED,
        "rc-diode-reversed",
        limits("rc-diode-reversed"),
    );

    // At 204 ms the correct circuit has charged to about 4.56 V and the reversed one to a
    // microvolt: the two are not close.
    assert!(charging.last().unwrap().1 > 4.5);
    assert!(reversed.last().unwrap().1 < 1e-5);
}

/// Everything a run reports at each step, as bit patterns, so two runs compare exactly.
fn bits(run: &Run) -> Vec<(u64, Vec<u64>, Vec<String>)> {
    run.delivered
        .iter()
        .map(|(frame, _)| {
            (
                frame.metadata.simulation_step,
                frame.values.iter().map(|value| value.to_bits()).collect(),
                frame
                    .states
                    .iter()
                    .map(|state| format!("{state:?}"))
                    .collect(),
            )
        })
        .collect()
}

fn rc_run(text: &str, steps_after_close: u32) -> Run {
    let request = request(text);
    let switch = device(&request, "switch_spst");
    let mut run = Run::start_every_step(request);
    run.advance(4);
    run.set_operator_state(&switch, OperatorPosition::Closed);
    run.advance(steps_after_close);
    run
}

#[test]
fn repeated_runs_of_the_same_events_are_identical_bit_for_bit() {
    for text in [RC_CHARGING, RC_REVERSED] {
        let (first, second) = (bits(&rc_run(text, 80)), bits(&rc_run(text, 80)));
        assert!(first.len() >= 80);
        assert_eq!(first, second);
    }
    for (text, type_id) in [
        (NPN_ACTIVE, "bjt_npn"),
        (PNP_SATURATION, "bjt_pnp"),
        (DIODE_FORWARD, "diode"),
    ] {
        let run = |text: &str| {
            let mut run = Run::start_every_step(request(text));
            run.advance(20);
            bits(&run)
        };
        assert_eq!(run(text), run(text), "{type_id}");
    }
}

#[test]
fn a_parameter_change_during_a_run_replaces_only_the_current_step() {
    let request = request(DIODE_FORWARD);
    let resistor = device(&request, "resistor");
    let diode = device(&request, "diode");
    let mut run = Run::start_every_step(request.clone());
    run.advance(5);
    let keys = vec![format!("{diode}:voltage"), format!("{diode}:current")];
    let before = run.samples(&keys, 0, 5);

    let (_, result) = run.set_parameter(&resistor, ParameterId::ResistanceOhms, 2000.0);
    let SetParameterResult::Success { snapshot, .. } = result else {
        panic!("a valid resistance is accepted");
    };
    assert_eq!(snapshot.metadata.simulation_step, 5);

    // Earlier samples are the old circuit's, the newest one is recomputed.
    let after = run.samples(&keys, 0, 5);
    assert_eq!(before[..5], after[..5]);
    assert_ne!(before[5], after[5]);

    // The recomputed step equals a fresh run of the edited drawing, within the solver tolerance.
    let mut edited = serde_json::to_value(&request).unwrap();
    for component in edited["schematic"]["components"].as_array_mut().unwrap() {
        if component["id"] == resistor.as_str() {
            component["parameters"]["resistanceOhms"] = json!(2000.0);
        }
    }
    let mut fresh = Run::start_every_step(serde_json::from_value(edited).unwrap());
    fresh.advance(5);
    let fresh = fresh.samples(&keys, 5, 5);
    for (live, new) in after[5].1.iter().zip(fresh[0].1.iter()) {
        assert_close(*live, *new, 1e-9 * new.abs().max(1e-6), "live versus fresh");
    }
}

#[test]
fn a_live_edit_keeps_the_capacitor_charge_and_the_switch_position() {
    let request = request(RC_CHARGING);
    let resistor = device(&request, "resistor");
    let capacitor = device(&request, "capacitor");
    let mut run = rc_run(RC_CHARGING, 30);
    let at = run.delivered.last().unwrap().0.metadata.simulation_step;
    let charge = |run: &Run, step: u64| value(run, step, &format!("{capacitor}:voltage"));
    let before = charge(&run, at);

    let (_, result) = run.set_parameter(&resistor, ParameterId::ResistanceOhms, 5000.0);
    assert!(matches!(result, SetParameterResult::Success { .. }));
    run.advance(40);

    // The state at the edit carried over: no jump at the edit, and the switch stayed closed
    // (the capacitor kept charging, faster through the smaller resistance).
    let next = charge(&run, at + 1);
    assert!(
        (next - before).abs() < 0.05,
        "a jump at the edit: {before} -> {next}"
    );
    assert!(charge(&run, at + 40) > before);
}

/// The identifier of the resistor of a drawn request that has this resistance.
fn resistor_of(request: &CompileRequest, ohms: f64) -> String {
    let json = serde_json::to_value(request).unwrap();
    json["schematic"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|component| {
            component["typeId"] == "resistor"
                && component["parameters"]["resistanceOhms"] == json!(ohms)
        })
        .unwrap_or_else(|| panic!("no {ohms} ohm resistor"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A refused edit leaves the run exactly as an untouched twin: same values and states at every
/// later step, and the next valid edit is still accepted.
fn refused_edit_is_atomic(
    start: impl Fn() -> Run,
    component: &str,
    value: f64,
    expected: &[DiagnosticCode],
) {
    let (mut touched, mut control) = (start(), start());
    let (_, result) = touched.set_parameter(component, ParameterId::ResistanceOhms, value);
    let SetParameterResult::Failure { diagnostics } = result else {
        panic!("the edit must be refused");
    };
    assert!(
        expected.contains(&diagnostics[0].code),
        "unexpected refusal {:?}",
        diagnostics[0].code
    );
    touched.advance(50);
    control.advance(50);

    assert_eq!(bits(&touched), bits(&control));
    let (_, ok) = touched.set_parameter(component, ParameterId::ResistanceOhms, 20_000.0);
    assert!(matches!(ok, SetParameterResult::Success { .. }));
}

#[test]
fn a_refused_edit_commits_nothing_and_the_run_stays_bit_identical() {
    // Nonlinear solves that run out of range while the edit is tried: nothing of the trial
    // reaches the run. Diode and transistor, direct current.
    for (text, ohms) in [(DIODE_FORWARD, 1000.0), (NPN_ACTIVE, 430_000.0)] {
        let request = request(text);
        let resistor = resistor_of(&request, ohms);
        refused_edit_is_atomic(
            || {
                let mut run = Run::start_every_step(request.clone());
                run.advance(5);
                run
            },
            &resistor,
            1e-21,
            &[DiagnosticCode::ModelRangeExceeded],
        );
    }

    // The same through a running switched capacitor circuit: the edit is refused (an ideal
    // short through the closed switch), and the capacitor's history is untouched.
    let request = request(RC_CHARGING);
    let resistor = device(&request, "resistor");
    refused_edit_is_atomic(
        || rc_run(RC_CHARGING, 30),
        &resistor,
        1e-21,
        &[
            DiagnosticCode::ModelRangeExceeded,
            DiagnosticCode::SingularSystem,
            DiagnosticCode::InvalidParameter,
        ],
    );
}

#[test]
fn the_compile_failures_name_the_device_and_the_document_stays_usable() {
    for (text, type_id) in [(DIODE_IDEAL_SOURCE, "diode"), (NPN_IDEAL_SOURCE, "bjt_npn")] {
        let request = request(text);
        let id = device(&request, type_id);
        let mut core = SimulationCore::new(SimulationConfig::default());
        let result = core.compile(request);
        let CompileResult::Failure { diagnostics } = &result else {
            panic!("an ideal source across a forward junction does not compile");
        };
        assert_eq!(diagnostics[0].code, DiagnosticCode::ModelRangeExceeded);
        assert_eq!(
            diagnostics[0].references,
            [DiagnosticReference::Component {
                component_id: StableId::parse(&id).unwrap(),
            }]
        );
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["status"], "failure");
        assert_eq!(
            json["diagnostics"][0]["localizationKey"],
            "diagnostic.modelRangeExceeded"
        );
    }

    // The same editor can then compile the corrected drawing on the same core.
    let mut core = SimulationCore::new(SimulationConfig::default());
    assert!(matches!(
        core.compile(request(DIODE_IDEAL_SOURCE)),
        CompileResult::Failure { .. }
    ));
    assert!(matches!(
        core.compile(request(DIODE_FORWARD)),
        CompileResult::Success { .. }
    ));
}

#[test]
fn a_saved_and_reopened_project_keeps_inputs_and_recalculates_everything_else() {
    let directory = tempfile::tempdir().expect("a temporary directory");

    for (text, name) in [
        (RC_CHARGING, "rc"),
        (NPN_ACTIVE, "npn"),
        (DIODE_FORWARD, "diode"),
    ] {
        let original = request(text);
        let path = directory.path().join(format!("{name}.ocircuit"));

        // Save after the circuit has run: nothing a run computed reaches the file.
        let mut run = Run::start_every_step(original.clone());
        run.advance(20);
        write_project_file(&path, &original.schematic).expect("the project is saved");
        let file = std::fs::read_to_string(&path).unwrap();
        for computed in [
            "region",
            "conducting",
            "iteration",
            "converged",
            "solver",
            "operating",
            "snapshot",
            "trace",
            "voltage\":",
            "runId",
        ] {
            assert!(
                !file.contains(computed),
                "{name}: the file holds `{computed}`"
            );
        }
        let reopened = read_project_file(&path).expect("the project reopens");
        assert_eq!(
            reopened, original.schematic,
            "{name}: inputs and geometry round-trip"
        );

        // Compiling the reopened document starts from step 0 and recomputes the same values.
        let mut document = serde_json::to_value(&original).unwrap();
        document["schematic"] = serde_json::to_value(&reopened).unwrap();
        let again: CompileRequest = serde_json::from_value(document).unwrap();
        let mut second = Run::start_every_step(again);
        assert_eq!(second.initial_snapshot.metadata.simulation_step, 0);
        second.advance(20);
        assert_eq!(
            bits(&run),
            bits(&second),
            "{name}: recalculated, not restored"
        );
    }
}

#[test]
fn a_diode_in_series_with_an_inductor_keeps_the_current_continuous_and_settles_on_the_dc_answer() {
    let request = request(DIODE_INDUCTOR);
    let inductor = device(&request, "inductor");
    let diode = device(&request, "diode");
    let mut run = Run::start_every_step(request);
    run.advance(5);
    let current = |run: &Run, step: u64| value(run, step, &format!("{inductor}:current"));

    // The inductor starts at zero current and the diode carries the same current as the
    // inductor at every step, rising without a jump.
    assert!(
        current(&run, 1).abs() < 1e-3,
        "the current starts from zero: about 0.43 mA per step at most"
    );
    let (mut previous, mut worst_jump) = (0.0_f64, 0.0_f64);
    for step in 1..=5 {
        worst_jump = worst_jump.max((current(&run, step) - previous).abs());
        previous = current(&run, step);
        assert_close(
            value(&run, step, &format!("{diode}:current")),
            current(&run, step),
            1e-9 * current(&run, step).abs() + 1e-12,
            "series current",
        );
    }
    assert!(worst_jump < 1e-3, "no jump: {worst_jump}");

    // Changing the inductance during the run keeps the current of the step it was changed at.
    let before = current(&run, 5);
    let (_, result) = run.set_parameter(&inductor, ParameterId::InductanceHenries, 5.0);
    assert!(matches!(result, SetParameterResult::Success { .. }));
    run.advance(400);
    assert_close(current(&run, 6), before, 1.5e-3, "no jump at the edit");

    // Long after the transient the current is the direct-current answer of the same diode.
    let last = run.delivered.last().unwrap().0.metadata.simulation_step;
    let expected = answer("diode-forward");
    let limits = tolerance("diode-forward");
    close(
        value(&run, last, &format!("{diode}:current")),
        num(&expected["diodeCurrentAmps"]),
        &limits,
        false,
        "settled current",
    );
    close(
        value(&run, last, &format!("{diode}:voltage")),
        num(&expected["diodeVoltageVolts"]),
        &limits,
        true,
        "settled voltage",
    );
}
