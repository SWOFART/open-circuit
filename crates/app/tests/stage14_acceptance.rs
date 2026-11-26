//! Runs the three editor-drawn Stage 14 acceptance circuits through the native simulation core.

mod common;

use common::{Run, assert_close};
use open_circuit_engine::{
    CompileRequest, DeviceState, EmergencyStopPosition, OperatorPosition, RelayContactState,
    ResultSnapshot, SetOperatorStateResult,
};
use serde_json::Value;

const STAR_DELTA: &str =
    include_str!("../../../tests/golden/stage-14/timed-star-delta.request.json");
const STOP_CHAIN: &str =
    include_str!("../../../tests/golden/stage-14/emergency-stop-chain.request.json");
const LED_AMPLIFIER: &str =
    include_str!("../../../tests/golden/stage-14/led-amplifier.request.json");
const REFERENCES: &str = include_str!("../../../tests/golden/stage-14/references.json");

fn start(drawing: &str) -> Run {
    let request: CompileRequest = serde_json::from_str(drawing).expect("editor-drawn request");
    Run::start_every_step(request)
}

fn id(part: u8) -> String {
    format!("00000000-0000-4000-8000-{part:012}")
}

fn value(run: &Run, snapshot: &ResultSnapshot, part: u8, result: &str) -> f64 {
    let key = format!("{}/{result}", id(part));
    let slot = run
        .layout
        .slots
        .iter()
        .find(|slot| slot.key == key)
        .unwrap_or_else(|| panic!("missing {key}"));
    snapshot.values[slot.offset as usize]
}

fn command(run: &mut Run, part: u8, position: OperatorPosition) -> ResultSnapshot {
    let (_, result) = run.set_operator_state(&id(part), position);
    match result {
        SetOperatorStateResult::Success { snapshot, .. } => snapshot,
        other => panic!("operator action failed: {other:?}"),
    }
}

fn contactor_picked(frame: &ResultSnapshot, part: u8) -> bool {
    frame
        .states
        .iter()
        .find_map(|state| match state {
            DeviceState::Contactor {
                component_id,
                state,
                ..
            } if component_id.as_str() == id(part) => Some(*state == RelayContactState::Picked),
            _ => None,
        })
        .expect("contactor state")
}

fn timer_output(frame: &ResultSnapshot, part: u8) -> bool {
    frame
        .states
        .iter()
        .find_map(|state| match state {
            DeviceState::RelayOnDelay {
                component_id,
                output_on,
                ..
            } if component_id.as_str() == id(part) => Some(*output_on),
            _ => None,
        })
        .expect("timer state")
}

#[test]
fn timed_star_delta_starts_in_star_and_transfers_with_a_dead_interval() {
    let mut run = start(STAR_DELTA);
    run.advance(20);
    let start = command(&mut run, 4, OperatorPosition::Closed);
    assert_eq!(start.metadata.simulation_step, 20);
    run.advance(160);

    // Timers start at the settled closure of the Stage 6 switch. Its control
    // voltage is applied to the newest interval, so the edge is seen at step 20.
    for (part, delay_steps) in [(7, 90), (8, 120)] {
        let deadline = 20 + delay_steps;
        assert!(!timer_output(&run.frame_at(deadline - 1).0, part));
        assert!(timer_output(&run.frame_at(deadline).0, part));
    }
    assert!(contactor_picked(&run.frame_at(80).0, 5));
    assert!(contactor_picked(&run.frame_at(80).0, 9));
    assert!(!contactor_picked(&run.frame_at(80).0, 11));
    let star_release = (81..=180)
        .find(|step| !contactor_picked(&run.frame_at(*step).0, 9))
        .expect("star contactor releases");
    let delta_pickup = (81..=180)
        .find(|step| contactor_picked(&run.frame_at(*step).0, 11))
        .expect("delta contactor picks");
    assert!(star_release >= 110 && star_release < delta_pickup);
    assert!(delta_pickup >= 140);
    for step in 21..=180 {
        let frame = &run.frame_at(step).0;
        let star = contactor_picked(frame, 9);
        let delta = contactor_picked(frame, 11);
        assert!(!(star && delta), "overlap at step {step}");
        assert_eq!(star, step < star_release);
        assert_eq!(delta, step >= delta_pickup);
    }
}

#[test]
fn emergency_stop_latches_and_the_timer_restarts_only_on_reset() {
    let mut run = start(STOP_CHAIN);
    let reference: Value = serde_json::from_str(REFERENCES).unwrap();
    let lamp_power = &reference["cases"]["lamp-24"];
    let timer = &reference["cases"]["relay-on-deadline"];
    let delay = timer["state"]["delayMs"].as_u64().unwrap();
    run.advance(delay as u32);
    assert!(!timer_output(&run.frame_at(delay - 1).0, 3));
    assert!(timer_output(&run.frame_at(delay).0, 3));
    for oracle in ["analytical", "ngspice"] {
        let expected = lamp_power[oracle]["powerWatts"].as_f64().unwrap();
        assert_close(
            value(&run, &run.frame_at(delay).0, 4, "powerWatts"),
            expected,
            1e-8 + 1e-3 * expected.abs(),
            oracle,
        );
    }
    for position in [OperatorPosition::Pressed, OperatorPosition::Released] {
        let response = command(&mut run, 2, position);
        assert!(response.states.iter().any(|state| matches!(state,
            DeviceState::EmergencyStop { component_id, position: EmergencyStopPosition::Latched, .. }
                if component_id.as_str() == id(2)
        )));
        assert!(!timer_output(&response, 3));
        assert_eq!(value(&run, &response, 4, "powerWatts"), 0.0);
    }
    command(&mut run, 2, OperatorPosition::Reset);
    run.advance(delay as u32);
    assert!(!timer_output(&run.frame_at(2 * delay - 1).0, 3));
    assert!(timer_output(&run.frame_at(2 * delay).0, 3));
}

#[test]
fn amplifier_follower_drives_the_led_at_the_independent_load_line() {
    let run = start(LED_AMPLIFIER);
    let frame = &run.initial_snapshot;
    // Vled = n*Vt*ln(1+I/Is). The 220-ohm load and the equivalent
    // closed-loop follower source (3*1000/1001 V, 1/1001 ohms) set I.
    let diode_voltage = |amps: f64| 2.0 * 0.02586492578632875 * (amps / 1e-14).ln_1p();
    let source = 3000.0 / 1001.0;
    let mut low = 0.0;
    let mut high = 3.0 / 220.0;
    for _ in 0..60 {
        let current = (low + high) / 2.0;
        if diode_voltage(current) + (220.0 + 1.0 / 1001.0) * current < source {
            low = current;
        } else {
            high = current;
        }
    }
    let current = (low + high) / 2.0;
    for (part, key, expected) in [
        (2, "outputVoltageVolts", source - current / 1001.0),
        (2, "outputCurrentAmps", -current),
        (4, "voltageVolts", diode_voltage(current)),
        (4, "currentAmps", current),
    ] {
        assert_close(
            value(&run, frame, part, key),
            expected,
            1e-8 + 1e-3 * expected.abs(),
            key,
        );
    }
    assert!(frame.states.iter().any(|state| matches!(state,
        DeviceState::Led { component_id, lit: true } if component_id.as_str() == id(4)
    )));
}
