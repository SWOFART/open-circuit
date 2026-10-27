//! The Stage 4 acceptance session (issue #69): the circuits of the Stage 4 fixtures, drawn
//! in the editor, compiled and run by the real Engine, and carried the way the bridge
//! carries them: JSON for compile and update results, binary OCF2 frames with the discrete
//! device states, and OCT1 trace responses.
//!
//! The compile requests are `tests/golden/stage-4-drawn-*.request.json`, produced by
//! `src/store/stage4Drawing.test.ts` from a blank document with the editor's own commands.
//! The expected values are the fixtures' `tests/golden/stage-4-*.expected.json`. The test
//! writes the session to `<target>/tmp/stage4-session.json`, which
//! `src/store/stage4EndToEnd.test.tsx` replays through the TypeScript adapter, decoder,
//! controllers, and panels.

mod common;

use std::fs;
use std::path::PathBuf;

use common::{Run, assert_close, hex, slot_key, value_of};
use open_circuit_engine::{
    CompileRequest, CompileResult, DeviceState, FuseState, ParameterId, RelayContactState,
    ResultQuantity, ResultSnapshot, SetParameterResult, component_catalog,
};
use serde_json::{Value, json};

const RELAY_REQUEST: &str = include_str!("../../../tests/golden/stage-4-drawn-relay.request.json");
const FUSE_REQUEST: &str = include_str!("../../../tests/golden/stage-4-drawn-fuse.request.json");
const MOTOR_REQUEST: &str = include_str!("../../../tests/golden/stage-4-drawn-motor.request.json");
const VOLTMETER_REQUEST: &str =
    include_str!("../../../tests/golden/stage-4-drawn-voltmeter.request.json");
const AMMETER_REQUEST: &str =
    include_str!("../../../tests/golden/stage-4-drawn-ammeter.request.json");

const RELAY_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-4-relay-self-latching.expected.json");
const FUSE_EXPECTED: &str = include_str!("../../../tests/golden/stage-4-fuse-i2t.expected.json");
const MOTOR_EXPECTED: &str = include_str!("../../../tests/golden/stage-4-pmdc-motor.expected.json");
const METERS_EXPECTED: &str =
    include_str!("../../../tests/golden/stage-4-meters-loading.expected.json");

/// The drawn circuits number their components in the order they were placed: the source
/// first, then the device, then the load or start path.
const SOURCE: &str = "00000000-0000-4000-8000-000000000001";
const DEVICE: &str = "00000000-0000-4000-8000-000000000002";
const OTHER: &str = "00000000-0000-4000-8000-000000000003";

fn request(text: &str) -> CompileRequest {
    serde_json::from_str(text).expect("a drawn request")
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("a JSON fixture")
}

fn relay_state(snapshot: &ResultSnapshot) -> (RelayContactState, Option<f64>) {
    match snapshot.states.as_slice() {
        [
            DeviceState::Relay {
                state,
                last_transition_time_seconds,
                ..
            },
        ] => (*state, *last_transition_time_seconds),
        other => panic!("the snapshot holds exactly one relay state, not {other:?}"),
    }
}

fn fuse_state(snapshot: &ResultSnapshot) -> (FuseState, Option<f64>) {
    match snapshot.states.as_slice() {
        [
            DeviceState::Fuse {
                state,
                last_transition_time_seconds,
                ..
            },
        ] => (*state, *last_transition_time_seconds),
        other => panic!("the snapshot holds exactly one fuse state, not {other:?}"),
    }
}

fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number"))
}

/// What a run of the self-latching relay looks like, step by step.
#[derive(Debug, PartialEq)]
struct RelayTimeline {
    /// For steps 0 to 10: the contact state, its last transition, and the coil current.
    steps: Vec<(u64, RelayContactState, Option<f64>, u64)>,
}

struct RelaySession {
    run: Run,
    timeline: RelayTimeline,
    updates: Vec<(u64, Value, Value)>,
}

fn run_relay() -> RelaySession {
    let mut run = Run::start_every_step(request(RELAY_REQUEST));
    let coil = format!("{DEVICE}/coil/current");
    let coil_slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == coil)
        .expect("the relay has a coil current slot")
        .offset as usize;
    let mut steps = vec![(
        0,
        relay_state(&run.initial_snapshot).0,
        relay_state(&run.initial_snapshot).1,
        run.initial_snapshot.values[coil_slot].to_bits(),
    )];
    let mut updates = Vec::new();

    for step in 1..=10_u64 {
        run.advance(1);

        // The fixture's two events happen at 2 ms and 6 ms of model time.
        if step == 2 {
            let (sent, reply) = run.set_parameter(OTHER, ParameterId::ResistanceOhms, 1e12);
            assert!(matches!(reply, SetParameterResult::Success { .. }));
            updates.push((step, json!(sent), json!(reply)));
        }
        if step == 6 {
            let (sent, reply) = run.set_parameter(SOURCE, ParameterId::VoltageVolts, 0.0);
            assert!(matches!(reply, SetParameterResult::Success { .. }));
            updates.push((step, json!(sent), json!(reply)));
        }

        let (snapshot, _) = run.frame_at(step);
        let (state, transition) = relay_state(snapshot);

        steps.push((
            step,
            state,
            transition,
            snapshot.values[coil_slot].to_bits(),
        ));
    }

    RelaySession {
        run,
        timeline: RelayTimeline { steps },
        updates,
    }
}

#[test]
fn the_self_latching_relay_picks_holds_and_releases_deterministically() {
    let expected = json_of(RELAY_EXPECTED);
    let session = run_relay();
    let steps = &session.timeline.steps;
    let at = |step: u64| steps.iter().find(|entry| entry.0 == step).unwrap();
    let current = |step: u64| f64::from_bits(at(step).3);

    // Time zero: the released-state solve exceeds the pickup current, so initialization
    // picks the relay before the first snapshot is returned.
    assert_eq!(expected["expected"][0]["contactState"], "picked");
    assert_eq!(at(0).1, RelayContactState::Picked);
    assert_eq!(at(0).2, Some(0.0));

    // Held through the update at 2 ms, and at 5 ms on the normally open contact alone.
    for step in 0..=5 {
        assert_eq!(at(step).1, RelayContactState::Picked, "step {step}");
        assert_eq!(at(step).2, Some(0.0), "step {step}: no new transition");
    }
    assert_close(
        current(5),
        number(&expected["expected"][1], "coilCurrentAmps"),
        number(&expected["expected"][1], "coilCurrentToleranceAmps"),
        "coil current at 5 ms",
    );

    // The supply is removed when the newest step is 6 ms. A live update takes effect from the
    // start of the newest step (docs/contracts/stage-3-dynamics.md section 3.4), so the
    // supply is gone from 5 ms; the sample of step 6 is replaced by the update's reply, and
    // the relay lets go shortly after 5 ms, well before the fixture's 7 ms.
    assert_eq!(expected["expected"][2]["contactState"], "released");
    assert_eq!(at(7).1, RelayContactState::Released);
    assert_close(
        current(7),
        number(&expected["expected"][2], "coilCurrentAmps"),
        number(&expected["expected"][2], "coilCurrentToleranceAmps"),
        "coil current at 7 ms",
    );
    let released_at = at(7).2.expect("the release has a transition time");
    assert!(
        released_at > 0.005 && released_at <= 0.007 + 1e-12,
        "released at {released_at} s"
    );
    let (_, _, reply) = session
        .updates
        .iter()
        .find(|(step, _, _)| *step == 6)
        .expect("the supply update");
    assert_eq!(
        reply["snapshot"]["states"][0]["state"], "released",
        "the replaced sample of step 6 already shows the release"
    );
    for step in 7..=10 {
        assert_eq!(at(step).1, RelayContactState::Released, "step {step}");
        assert_eq!(at(step).2, Some(released_at), "step {step}");
    }

    // The hold path really is the contact: at 5 ms the normally open branch carries the
    // coil current and the normally closed branch carries none.
    let (snapshot, _) = session.run.frame_at(5);
    assert_close(
        value_of(
            &session.run.layout,
            snapshot,
            DEVICE,
            ResultQuantity::Current,
        ),
        current(5),
        1e-12,
        "the first current of the relay is the coil's",
    );
    let branch = |name: &str| {
        let slot = session
            .run
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == format!("{DEVICE}/{name}/current"))
            .unwrap();
        snapshot.values[slot.offset as usize]
    };
    assert!(branch("normally_open") > 0.049);
    assert!(branch("normally_closed").abs() < 1e-12);

    // Running the same document again gives the same states and the same bits.
    assert_eq!(run_relay().timeline, session.timeline);
}

#[test]
fn a_new_compile_of_the_relay_starts_a_fresh_run_whatever_happened_before() {
    let finished = run_relay();
    assert_eq!(
        finished.timeline.steps.last().map(|entry| entry.1),
        Some(RelayContactState::Released)
    );

    let again = Run::start_every_step(request(RELAY_REQUEST));

    assert_ne!(again.run_id, finished.run.run_id);
    assert_eq!(
        relay_state(&again.initial_snapshot),
        (RelayContactState::Picked, Some(0.0))
    );
}

#[test]
fn the_fuse_trips_at_the_agreed_i2t_opens_its_path_and_starts_intact_after_a_new_compile() {
    let expected = json_of(FUSE_EXPECTED);
    let mut run = Run::start_every_step(request(FUSE_REQUEST));
    let key = |name: &str| format!("{DEVICE}/{name}");
    let value = |run: &Run, snapshot: &ResultSnapshot, suffix: &str| {
        let slot = run
            .layout
            .slots
            .iter()
            .find(|slot| slot.key == suffix)
            .unwrap_or_else(|| panic!("no slot {suffix}"));
        snapshot.values[slot.offset as usize]
    };

    // Before anything happens: intact, nothing accumulated, the steady 2 A of the circuit.
    assert_eq!(fuse_state(&run.initial_snapshot), (FuseState::Intact, None));
    assert_close(
        value(&run, &run.initial_snapshot, &key("accumulated_i2t")),
        0.0,
        1e-12,
        "I2t at step 0",
    );
    assert_close(
        value_of(
            &run.layout,
            &run.initial_snapshot,
            DEVICE,
            ResultQuantity::Current,
        ),
        2.0,
        1e-9,
        "fuse current at step 0",
    );

    run.advance(3);

    // At 1 ms the accumulator reaches the threshold in exactly one public step.
    let first = &expected["expected"][0];
    let (snapshot, _) = run.frame_at(1);
    let (state, transition) = fuse_state(snapshot);
    assert_eq!(state, FuseState::Tripped);
    assert_close(
        transition.expect("a trip time"),
        number(first, "timeSeconds"),
        number(first, "tripTimeToleranceSeconds"),
        "trip time",
    );
    assert_close(
        value(&run, snapshot, &key("accumulated_i2t")),
        number(first, "accumulatedI2tAmpereSquaredSeconds"),
        1e-12,
        "I2t at the trip",
    );

    // Afterwards the fuse is an open circuit: no current anywhere in the loop, and the
    // whole source voltage across the fuse. The trip is one-shot: it stays tripped.
    let second = &expected["expected"][1];
    for step in 2..=3 {
        let (snapshot, _) = run.frame_at(step);

        assert_eq!(fuse_state(snapshot).0, FuseState::Tripped, "step {step}");
        assert_close(
            value_of(&run.layout, snapshot, DEVICE, ResultQuantity::Current),
            number(second, "fuseCurrentAmps"),
            number(second, "currentToleranceAmps"),
            "fuse current after the trip",
        );
        assert_close(
            value_of(&run.layout, snapshot, DEVICE, ResultQuantity::Voltage),
            number(second, "fuseVoltageVolts"),
            number(second, "voltageToleranceVolts"),
            "voltage across the open fuse",
        );
        assert_close(
            value_of(&run.layout, snapshot, OTHER, ResultQuantity::Current),
            0.0,
            1e-12,
            "load current after the trip",
        );
        assert_close(
            value(&run, snapshot, &key("accumulated_i2t")),
            number(second, "accumulatedI2tAmpereSquaredSeconds"),
            1e-12,
            "I2t stays at the threshold",
        );
    }

    // A new compile is the reset: a new run, the fuse intact again, and it trips again.
    let mut restarted = Run::start_every_step(request(FUSE_REQUEST));

    assert_ne!(restarted.run_id, run.run_id);
    assert_eq!(
        fuse_state(&restarted.initial_snapshot),
        (FuseState::Intact, None)
    );
    assert_close(
        value_of(
            &restarted.layout,
            &restarted.initial_snapshot,
            DEVICE,
            ResultQuantity::Current,
        ),
        2.0,
        1e-9,
        "fuse current after the reset",
    );
    restarted.advance(1);
    assert_eq!(
        fuse_state(&restarted.frame_at(1).0).0,
        FuseState::Tripped,
        "the restarted run trips at the same time"
    );
}

/// `x(t)` of the motor's state equations `x' = A x + c`, `x = [i, omega]`, with
/// `A = [[-R/L, -k/L], [k/J, -b/J]]` and `c = [v/L, tau/J]`, from `x(0) = [0, 0]`
/// (docs/contracts/stage-4-components.md section 6).
/// The motor circuit's parameters, in SI units.
#[derive(Clone, Copy)]
struct Motor {
    volts: f64,
    resistance: f64,
    inductance: f64,
    constant: f64,
    inertia: f64,
    friction: f64,
    torque: f64,
}

/// The drawn motor circuit of the fixture: 10 V across the catalog's defaults, no load.
const FIXTURE_MOTOR: Motor = Motor {
    volts: 10.0,
    resistance: 2.0,
    inductance: 0.1,
    constant: 0.1,
    inertia: 0.01,
    friction: 0.01,
    torque: 0.0,
};

fn motor_closed_form(time: f64, motor: Motor) -> (f64, f64) {
    let Motor {
        volts,
        resistance,
        inductance,
        constant,
        inertia,
        friction,
        torque,
    } = motor;
    let a = [
        [-resistance / inductance, -constant / inductance],
        [constant / inertia, -friction / inertia],
    ];
    let c = [volts / inductance, torque / inertia];
    let trace = a[0][0] + a[1][1];
    let determinant = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    let root = (trace * trace - 4.0 * determinant).sqrt();
    let (l1, l2) = ((trace + root) / 2.0, (trace - root) / 2.0);
    // The steady state solves A x = -c.
    let x_inf = [
        (-c[0] * a[1][1] + c[1] * a[0][1]) / determinant,
        (-a[0][0] * c[1] + a[1][0] * c[0]) / determinant,
    ];
    let d = [-x_inf[0], -x_inf[1]];
    // exp(A t) d = (e^{l1 t}(A - l2 I) - e^{l2 t}(A - l1 I)) d / (l1 - l2)
    let apply = |lambda: f64| {
        [
            (a[0][0] - lambda) * d[0] + a[0][1] * d[1],
            a[1][0] * d[0] + (a[1][1] - lambda) * d[1],
        ]
    };
    let (m2, m1) = (apply(l2), apply(l1));
    let current = x_inf[0] + ((l1 * time).exp() * m2[0] - (l2 * time).exp() * m1[0]) / (l1 - l2);
    let speed = x_inf[1] + ((l1 * time).exp() * m2[1] - (l2 * time).exp() * m1[1]) / (l1 - l2);

    (current, speed)
}

fn motor_values(run: &Run, snapshot: &ResultSnapshot) -> (f64, f64) {
    (
        value_of(&run.layout, snapshot, DEVICE, ResultQuantity::Current),
        value_of(
            &run.layout,
            snapshot,
            DEVICE,
            ResultQuantity::AngularVelocity,
        ),
    )
}

#[test]
fn the_motor_starts_from_rest_and_follows_the_closed_form_solution() {
    let expected = json_of(MOTOR_EXPECTED);
    let reference = &expected["reference"];
    let mut run = Run::start_every_step(request(MOTOR_REQUEST));

    // Time zero is the configured rest state.
    assert_eq!(motor_values(&run, &run.initial_snapshot), (0.0, 0.0));
    assert!(run.advance(8000) >= 8000);

    // The closed form here and the fixture's numbers agree within the fixture's tolerance,
    // which makes it a fair reference at the other times as well. (The fixture's speed is
    // 3.2e-4 rad/s above the closed form: it approaches the steady 33.33333 rad/s from
    // below, and the fixture's value is above it. Both are far inside the 0.01 rad/s
    // tolerance; see the acceptance record.)
    let (current, speed) = motor_closed_form(8.0, FIXTURE_MOTOR);
    assert_close(
        speed,
        number(reference, "angularVelocityRadiansPerSecond"),
        number(reference, "angularVelocityToleranceRadiansPerSecond"),
        "closed-form speed at 8 s",
    );
    assert_close(
        current,
        number(reference, "armatureCurrentAmps"),
        number(reference, "armatureCurrentToleranceAmps"),
        "closed-form current at 8 s",
    );

    // The Engine against the reference: the fixture's time, and a spread of earlier ones.
    for step in [10, 100, 500, 1000, 2000, 4000, 8000] {
        let (i, w) = motor_values(&run, &run.frame_at(step).0);
        let (expected_i, expected_w) = motor_closed_form(step as f64 * 0.001, FIXTURE_MOTOR);

        assert_close(
            w,
            expected_w,
            number(reference, "angularVelocityToleranceRadiansPerSecond"),
            &format!("angular velocity at {step} ms"),
        );
        assert_close(
            i,
            expected_i,
            number(reference, "armatureCurrentToleranceAmps"),
            &format!("armature current at {step} ms"),
        );
    }
    let (i, w) = motor_values(&run, &run.frame_at(8000).0);
    assert_close(
        w,
        number(reference, "angularVelocityRadiansPerSecond"),
        number(reference, "angularVelocityToleranceRadiansPerSecond"),
        "angular velocity at 8 s",
    );
    assert_close(
        i,
        number(reference, "armatureCurrentAmps"),
        number(reference, "armatureCurrentToleranceAmps"),
        "armature current at 8 s",
    );

    // The stored trace carries the very values of the frame, to the bit.
    let keys = vec![
        slot_key(&run.layout, DEVICE, ResultQuantity::AngularVelocity),
        slot_key(&run.layout, DEVICE, ResultQuantity::Current),
    ];
    let samples = run.samples(&keys, 7000, 8000);
    let last = samples.last().expect("samples");

    assert_eq!(last.0, 8000);
    assert_eq!(last.1[0].to_bits(), w.to_bits());
    assert_eq!(last.1[1].to_bits(), i.to_bits());
}

#[test]
fn the_motor_settles_to_the_operating_point_of_a_load_torque() {
    // An applied shaft torque of -0.05 N·m opposes the rotation. The steady state of the
    // linear equations is omega = (v + R tau / k) / (R b / k + k) and i = (b omega - tau) / k.
    let mut loaded = serde_json::to_value(request(MOTOR_REQUEST)).unwrap();
    let motor = loaded["schematic"]["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == "pmdc_motor")
        .unwrap();
    motor["parameters"]["loadTorqueNewtonMeters"] = json!(-0.05);
    let loaded: CompileRequest = serde_json::from_value(loaded).unwrap();
    let mut run = Run::start_every_step(loaded);

    run.advance(8000);
    let (i, w) = motor_values(&run, &run.frame_at(8000).0);
    let (expected_i, expected_w) = motor_closed_form(
        8.0,
        Motor {
            torque: -0.05,
            ..FIXTURE_MOTOR
        },
    );

    assert_close(expected_w, 30.0, 0.01, "the steady speed under load");
    assert_close(expected_i, 3.5, 0.001, "the steady current under load");
    assert_close(w, expected_w, 0.01, "angular velocity under load");
    assert_close(i, expected_i, 0.001, "armature current under load");
    assert!(w < 33.0, "the load slows the motor");
}

/// The request with the meter's two pins swapped in the geometry, as if it were wired the
/// other way round.
fn reversed(request: &CompileRequest, component: &str) -> CompileRequest {
    let mut value = serde_json::to_value(request).unwrap();
    let pins = value["geometry"]["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["componentId"] == component)
        .unwrap()["pins"]
        .as_array_mut()
        .unwrap();
    let first = pins[0]["position"].clone();
    pins[0]["position"] = pins[1]["position"].clone();
    pins[1]["position"] = first;

    serde_json::from_value(value).unwrap()
}

#[test]
fn the_meters_read_with_their_sign_and_load_the_circuit_they_measure() {
    let expected = json_of(METERS_EXPECTED);
    let voltmeter = &expected["voltmeterCircuit"];
    let ammeter = &expected["ammeterCircuit"];

    // Voltmeter: 10 V behind 1 Mohm, read through 10 Mohm.
    let run = Run::start(request(VOLTMETER_REQUEST));
    let reading = value_of(
        &run.layout,
        &run.initial_snapshot,
        DEVICE,
        ResultQuantity::Voltage,
    );
    let input_current = value_of(
        &run.layout,
        &run.initial_snapshot,
        DEVICE,
        ResultQuantity::Current,
    );
    assert_close(
        reading,
        number(voltmeter, "expectedReadingVolts"),
        number(voltmeter, "readingToleranceVolts"),
        "voltmeter reading",
    );
    assert_close(
        input_current,
        number(voltmeter, "expectedInputCurrentAmps"),
        number(voltmeter, "currentToleranceAmps"),
        "voltmeter input current",
    );
    // Loading: an ideal meter would read the source's 10 V.
    assert!(10.0 - reading > 0.9, "the meter loads the source");

    // Wired the other way round, the reading changes sign and nothing else.
    let flipped = Run::start(reversed(&request(VOLTMETER_REQUEST), DEVICE));
    assert_close(
        value_of(
            &flipped.layout,
            &flipped.initial_snapshot,
            DEVICE,
            ResultQuantity::Voltage,
        ),
        -reading,
        1e-9,
        "reversed voltmeter reading",
    );
    assert_close(
        value_of(
            &flipped.layout,
            &flipped.initial_snapshot,
            DEVICE,
            ResultQuantity::Current,
        ),
        -input_current,
        1e-12,
        "reversed voltmeter input current",
    );

    // Ammeter: 10 V across 10 ohm plus its own 1 ohm.
    let run = Run::start(request(AMMETER_REQUEST));
    let reading = value_of(
        &run.layout,
        &run.initial_snapshot,
        DEVICE,
        ResultQuantity::Current,
    );
    let burden = value_of(
        &run.layout,
        &run.initial_snapshot,
        DEVICE,
        ResultQuantity::Voltage,
    );
    assert_close(
        reading,
        number(ammeter, "expectedReadingAmps"),
        number(ammeter, "readingToleranceAmps"),
        "ammeter reading",
    );
    assert_close(
        burden,
        number(ammeter, "expectedBurdenVoltageVolts"),
        number(ammeter, "voltageToleranceVolts"),
        "ammeter burden voltage",
    );
    // Loading: an ideal ammeter would read 10 V / 10 ohm = 1 A.
    assert!(
        1.0 - reading > 0.09,
        "the meter's burden lowers the current"
    );
    // The load carries the same current the meter reads.
    assert_close(
        value_of(
            &run.layout,
            &run.initial_snapshot,
            OTHER,
            ResultQuantity::Current,
        ),
        reading,
        1e-9,
        "load current",
    );

    let flipped = Run::start(reversed(&request(AMMETER_REQUEST), DEVICE));
    assert_close(
        value_of(
            &flipped.layout,
            &flipped.initial_snapshot,
            DEVICE,
            ResultQuantity::Current,
        ),
        -reading,
        1e-9,
        "reversed ammeter reading",
    );
    assert_close(
        value_of(
            &flipped.layout,
            &flipped.initial_snapshot,
            DEVICE,
            ResultQuantity::Voltage,
        ),
        -burden,
        1e-9,
        "reversed ammeter burden",
    );
}

#[test]
fn a_relay_whose_dropout_is_not_below_its_pickup_is_refused_and_keeps_the_previous_run() {
    let mut run = Run::start_every_step(request(RELAY_REQUEST));
    let (_, reply) = run.set_parameter(DEVICE, ParameterId::DropoutCurrentAmps, 0.05);

    // The Engine's cross-parameter rule: the update is refused with a structured diagnostic
    // and the run goes on as it was.
    let SetParameterResult::Failure { diagnostics } = reply else {
        panic!("a dropout above the pickup must be refused");
    };
    assert_eq!(
        diagnostics[0].code,
        open_circuit_engine::DiagnosticCode::InvalidParameter
    );
    assert_eq!(run.revision(), "rev-1");
    run.advance(2);
    assert_eq!(relay_state(&run.frame_at(2).0).0, RelayContactState::Picked);

    // The same values in a document are refused at compile.
    let mut edited = serde_json::to_value(request(RELAY_REQUEST)).unwrap();
    edited["schematic"]["components"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|component| component["typeId"] == "relay_spdt")
        .unwrap()["parameters"]["dropoutCurrentAmps"] = json!(0.05);
    let edited: CompileRequest = serde_json::from_value(edited).unwrap();
    let result = open_circuit_app_lib::simulation::SimulationCore::new(
        open_circuit_app_lib::simulation::SimulationConfig::default(),
    )
    .compile(edited);
    let CompileResult::Failure { diagnostics } = result else {
        panic!("the inconsistent relay must not compile");
    };

    assert_eq!(
        diagnostics[0].code,
        open_circuit_engine::DiagnosticCode::InvalidParameter
    );
}

/// Writes the session the TypeScript replay reads.
#[test]
fn the_stage_4_session_is_recorded_for_the_user_interface_replay() {
    // Relay: the update script of the fixture with every frame of the first 10 ms.
    let relay = run_relay();
    let relay_frames: Vec<Value> = (1..=10_u64)
        .map(|step| {
            let (snapshot, bytes) = relay.run.frame_at(step);
            json!({
                "step": step,
                "revision": snapshot.metadata.document_revision,
                "hex": hex(bytes),
            })
        })
        .collect();

    // Fuse: the trip, a trace of the accumulator, and a second compile of the same document.
    let mut fuse = Run::start_every_step(request(FUSE_REQUEST));
    fuse.advance(3);
    let fuse_frames: Vec<Value> = (1..=3_u64)
        .map(|step| json!({ "step": step, "hex": hex(&fuse.frame_at(step).1) }))
        .collect();
    let fuse_keys = vec![
        slot_key(&fuse.layout, DEVICE, ResultQuantity::AccumulatedI2t),
        slot_key(&fuse.layout, DEVICE, ResultQuantity::Current),
    ];
    let (_, fuse_trace, fuse_trace_request) = fuse.read(&fuse_keys, 0, 3);
    // The application's next compile of the same document carries the next revision.
    let mut again = request(FUSE_REQUEST);
    again.document_revision = "rev-2".to_owned();
    let mut fuse_again = Run::start_every_step(again);
    fuse_again.advance(1);

    // Motor: frames at the start and at 8 s, and a trace of the last second.
    let mut motor = Run::start_every_step(request(MOTOR_REQUEST));
    motor.advance(8000);
    let motor_frames: Vec<Value> = [1_u64, 1000, 8000]
        .iter()
        .map(|step| json!({ "step": step, "hex": hex(&motor.frame_at(*step).1) }))
        .collect();
    let motor_keys = vec![
        slot_key(&motor.layout, DEVICE, ResultQuantity::AngularVelocity),
        slot_key(&motor.layout, DEVICE, ResultQuantity::Current),
    ];
    let (_, motor_trace, motor_trace_request) = motor.read(&motor_keys, 7000, 8000);

    let voltmeter = Run::start(request(VOLTMETER_REQUEST));
    let ammeter = Run::start(request(AMMETER_REQUEST));

    let recorded = json!({
        "format": 1,
        "description": "The Stage 4 acceptance session, written by crates/app/tests/stage4_session.rs on every cargo test. src/store/stage4EndToEnd.test.tsx replays it through the TypeScript adapter, decoder, controllers, and panels.",
        "catalog": component_catalog(),
        "relay": {
            "request": request(RELAY_REQUEST),
            "compileReply": relay.run.compile_result,
            "frames": relay_frames,
            "updates": relay.updates.iter().map(|(step, sent, reply)| json!({
                "afterStep": step, "request": sent, "reply": reply,
            })).collect::<Vec<_>>(),
        },
        "fuse": {
            "request": request(FUSE_REQUEST),
            "compileReply": fuse.compile_result,
            "frames": fuse_frames,
            "traceRequest": fuse_trace_request,
            "traceResponseHex": hex(&fuse_trace),
            "secondCompileReply": fuse_again.compile_result,
            "secondFrames": [json!({ "step": 1, "hex": hex(&fuse_again.frame_at(1).1) })],
        },
        "motor": {
            "request": request(MOTOR_REQUEST),
            "compileReply": motor.compile_result,
            "frames": motor_frames,
            "traceRequest": motor_trace_request,
            "traceResponseHex": hex(&motor_trace),
        },
        "voltmeter": {
            "request": request(VOLTMETER_REQUEST),
            "compileReply": voltmeter.compile_result,
        },
        "ammeter": {
            "request": request(AMMETER_REQUEST),
            "compileReply": ammeter.compile_result,
        },
    });
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("stage4-session.json");
    fs::create_dir_all(path.parent().expect("a parent directory")).expect("create the directory");
    fs::write(
        &path,
        serde_json::to_string_pretty(&recorded).expect("the session serializes"),
    )
    .expect("write the session");
}
