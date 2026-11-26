//! Stage 15 acceptance against the drawn divider and independent divider equation.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use open_circuit_app_lib::checks::CheckService;
use open_circuit_app_lib::frame::decode_frame;
use open_circuit_app_lib::simulation::{
    FrameSink, PHYSICS_STEP, PlaybackCommand, SimulationConfig, SimulationCore, SinkClosed,
};
use open_circuit_engine::{
    CheckRequest, CompileRequest, CompileResult, GeometryEnvelope, GradedCheckRequest, GridPoint,
    ProbeRequest, ProbeResult, ProbeTarget, ResultSnapshot, Unit, decode_assignment_json,
};

const ASSIGNMENT: &[u8] =
    include_bytes!("../../../tests/golden/stage-15-divider-scored.ocassignment");
const GEOMETRY: &str = include_str!("../../../tests/golden/stage-1-divider.geometry.json");
const EXPECTED_VOLTS: f64 = 5.0 * 1000.0 / (1000.0 + 1000.0 + 0.1);

struct Frames(Arc<Mutex<Vec<ResultSnapshot>>>);

impl FrameSink for Frames {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0.lock().unwrap().push(decode_frame(&frame).unwrap());
        Ok(())
    }
}

fn reading(core: &SimulationCore, snapshot: &ResultSnapshot) -> f64 {
    let ProbeResult::Reading {
        metadata,
        value,
        unit,
    } = core.probe(ProbeRequest {
        run_id: snapshot.metadata.run_id.clone(),
        document_revision: snapshot.metadata.document_revision.clone(),
        frame_sequence: snapshot.metadata.frame_sequence,
        target: ProbeTarget::Voltage {
            positive: GridPoint { x: 6, y: 0 },
            negative: GridPoint { x: 9, y: 0 },
        },
    })
    else {
        panic!("expected a committed voltage measurement")
    };
    assert_eq!(unit, Unit::Volt);
    assert_eq!(metadata, snapshot.metadata);
    value
}

#[test]
fn a_student_measures_steps_and_receives_a_partial_score_and_hint() {
    let assignment = decode_assignment_json(ASSIGNMENT).unwrap();
    let schematic = assignment.starting_circuit.unwrap();
    let geometry: GeometryEnvelope = serde_json::from_str(GEOMETRY).unwrap();
    let mut core = SimulationCore::new(SimulationConfig {
        frame_interval: PHYSICS_STEP,
        catch_up_budget: Duration::from_secs(5),
        ..SimulationConfig::default()
    });
    let revision = "divider-acceptance";
    let result = core.compile(CompileRequest {
        schematic: schematic.clone(),
        geometry: geometry.clone(),
        document_revision: revision.into(),
        faults: None,
    });
    let CompileResult::Success {
        run_id,
        initial_snapshot,
        ..
    } = result
    else {
        panic!("divider fixture must compile: {result:?}")
    };
    let frames = Arc::new(Mutex::new(Vec::new()));
    let start = Instant::now();
    core.start(run_id.clone(), Box::new(Frames(Arc::clone(&frames))), start)
        .unwrap();
    core.playback(&run_id, revision, PlaybackCommand::Pause, start)
        .unwrap();
    core.tick(start);
    assert!((reading(&core, &initial_snapshot) - EXPECTED_VOLTS).abs() < 1e-9);

    let later = start + Duration::from_secs(3);
    core.tick(later);
    assert_eq!(frames.lock().unwrap().len(), 1);
    core.playback(&run_id, revision, PlaybackCommand::Step, later)
        .unwrap();
    core.tick(later + PHYSICS_STEP);
    let stepped = frames.lock().unwrap().last().unwrap().clone();
    assert_eq!(stepped.metadata.simulation_step, 1);
    assert_eq!(stepped.metadata.simulation_time_seconds, 0.001);
    assert!((reading(&core, &stepped) - EXPECTED_VOLTS).abs() < 1e-9);

    let graded = CheckService::default()
        .run_graded(GradedCheckRequest {
            request: CheckRequest {
                checks: assignment.checks.unwrap(),
                locked_components: Vec::new(),
                starting_circuit: schematic.clone(),
                schematic,
                geometry,
                document_revision: revision.into(),
            },
            grading: assignment.grading.unwrap(),
        })
        .unwrap();
    assert_eq!((graded.score.earned, graded.score.possible), (7, 10));
    assert!(graded.score.criteria[0].hint.is_none());
    assert_eq!(graded.score.criteria[1].earned, 0);
    assert_eq!(
        graded.score.criteria[1].hint.as_ref().unwrap().key,
        "resistors"
    );
    assert_eq!(
        graded.score.criteria[1].hint.as_ref().unwrap().text,
        "This starter circuit has two resistors. The exercise calls for three."
    );
    assert!((reading(&core, &stepped) - EXPECTED_VOLTS).abs() < 1e-9);
}
