//! Stage 13 references (issue #202): the Engine against the closed-form results of
//! `scripts/stage13-references.mjs`, committed in `tests/golden/stage-13/`. The generator has no
//! dependency on the Engine; each correct fixture is paired with wrong variants whose different
//! results the Engine must reproduce as well (a wrong circuit is not repaired).

use open_circuit_engine::{AdvanceRequest, CompileRequest, CompileResult, Engine};
use serde_json::Value;

const ANALYTICAL: &str = include_str!("../../../tests/golden/stage-13/analytical.json");
const MANIFEST: &str = include_str!("../../../tests/golden/stage-13/manifest.json");

struct Run {
    engine: Engine,
    run_id: open_circuit_engine::StableId,
    layout: open_circuit_engine::ResultLayout,
    snapshot: open_circuit_engine::ResultSnapshot,
    step: u32,
}

fn start(request: &Value) -> Run {
    let request: CompileRequest =
        serde_json::from_value(request.clone()).expect("a compile request");
    let mut engine = Engine::default();
    let result = engine.compile(request);
    let CompileResult::Success {
        run_id,
        layout,
        initial_snapshot,
        ..
    } = result
    else {
        panic!("a reference circuit should compile: {result:?}");
    };
    Run {
        engine,
        run_id,
        layout,
        snapshot: initial_snapshot,
        step: 0,
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

    fn advance_to(&mut self, step: u32) {
        assert!(step >= self.step, "checks must be in step order");
        if step > self.step {
            self.snapshot = self
                .engine
                .advance(AdvanceRequest {
                    run_id: self.run_id.clone(),
                    step_count: step - self.step,
                })
                .expect("the run advances");
            self.step = step;
        }
    }
}

fn number(value: &Value, field: &str) -> f64 {
    value[field]
        .as_f64()
        .unwrap_or_else(|| panic!("no number {field} in {value}"))
}

fn fixtures() -> Vec<Value> {
    let all: Value = serde_json::from_str(ANALYTICAL).unwrap();
    all["fixtures"].as_array().unwrap().clone()
}

fn run_checks(fixture: &Value) {
    let id = fixture["id"].as_str().unwrap();
    let mut checks: Vec<&Value> = fixture["checks"].as_array().unwrap().iter().collect();
    assert!(!checks.is_empty(), "{id} has no checks");
    checks.sort_by_key(|check| check["step"].as_u64().unwrap_or(0));

    let mut run = start(&fixture["request"]);
    for check in checks {
        let key = check["key"].as_str().unwrap();
        match check["kind"].as_str().unwrap() {
            "value" => {
                run.advance_to(check["step"].as_u64().unwrap() as u32);
                let (expected, relative, absolute) = (
                    number(check, "expected"),
                    number(check, "relative"),
                    number(check, "absolute"),
                );
                let actual = run.value(key);
                assert!(
                    (actual - expected).abs() <= absolute + relative * expected.abs(),
                    "{id}: {key} at step {} is {actual}, expected {expected} (relative {relative}, absolute {absolute})",
                    run.step
                );
            }
            "tripStep" => {
                // A trip check runs on a run of its own, from the start.
                let mut tripped_run = start(&fixture["request"]);
                let horizon = check["horizonSteps"].as_u64().unwrap() as u32;
                let mut tripped_at = (tripped_run.value(key) == 1.0).then_some(0);
                for step in 1..=horizon {
                    if tripped_at.is_some() {
                        break;
                    }
                    tripped_run.advance_to(step);
                    if tripped_run.value(key) == 1.0 {
                        tripped_at = Some(step);
                    }
                }
                match (check["expectedSeconds"].as_f64(), tripped_at) {
                    (None, None) => {}
                    (None, Some(step)) => {
                        panic!(
                            "{id}: {key} tripped at step {step}, expected never within {horizon} steps"
                        )
                    }
                    (Some(expected), None) => {
                        panic!(
                            "{id}: {key} never tripped within {horizon} steps, expected {expected} s"
                        )
                    }
                    (Some(expected), Some(step)) => {
                        let seconds = f64::from(step) * 1.0e-3;
                        let tolerance = number(check, "toleranceSeconds");
                        assert!(
                            (seconds - expected).abs() <= tolerance,
                            "{id}: {key} tripped at {seconds} s, expected {expected} s (tolerance {tolerance} s)"
                        );
                    }
                }
            }
            other => panic!("{id}: unknown check kind {other}"),
        }
    }
}

#[test]
fn the_fixture_files_describe_each_other() {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    let fixtures = fixtures();

    assert_eq!(
        manifest["fixtureCount"].as_u64().unwrap() as usize,
        fixtures.len()
    );
    let wrong = fixtures
        .iter()
        .filter(|fixture| fixture["variant"] == "wrong")
        .count();
    assert_eq!(manifest["wrongVariants"].as_u64().unwrap() as usize, wrong);
    // Every protection family has a wrong variant to tell a correct reading from a wrong one.
    for family in [
        "earth-fault",
        "touch-voltage",
        "rcd",
        "breaker",
        "short-circuit",
    ] {
        assert!(
            fixtures
                .iter()
                .any(|f| f["family"] == family && f["variant"] == "wrong"),
            "{family} has no wrong variant"
        );
    }
}

#[test]
fn the_engine_matches_every_correct_reference() {
    for fixture in fixtures()
        .iter()
        .filter(|fixture| fixture["variant"] == "correct")
    {
        run_checks(fixture);
    }
}

#[test]
fn the_engine_reproduces_every_wrong_variant_as_its_own_distinct_result() {
    for fixture in fixtures()
        .iter()
        .filter(|fixture| fixture["variant"] == "wrong")
    {
        run_checks(fixture);
    }
}
