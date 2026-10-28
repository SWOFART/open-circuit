//! Assignment checks (docs/contracts/stage-5-assignments.md section 4).
//!
//! A check judges a student's schematic against the criteria of an assignment. It runs on its
//! own `Engine` instance, from step zero, through the public steps the criteria name, so it
//! never touches the run a user is watching, starts no stream, and depends on nothing but its
//! request: not on a frame rate, the wall-clock, a trace history, or an earlier check.
//!
//! Every problem with the circuit or with a criterion is a structured verdict, never an error:
//! a circuit that does not compile, a part that is gone, or a criterion that cannot be
//! evaluated makes the verdict `notEvaluated`.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    AdvanceRequest, CompileRequest, CompileResult, ComponentSpec, ComponentTypeId, DeviceState,
    Diagnostic, DiagnosticReference, DiagnosticSeverity, Engine, FuseState, GeometryEnvelope,
    RelayContactState, ResultLayout, ResultSnapshot, SIMULATION_TIMESTEP_SECONDS, Schematic,
    StableId, Unit,
};

/// At most this many criteria are evaluated; any further one is `notEvaluated`.
pub const MAX_CHECK_CRITERIA: usize = 64;
/// The longest time a criterion may name, in seconds: 60 000 public steps.
pub const MAX_CHECK_SECONDS: f64 = 60.0;
const MAX_CHECK_STEPS: u64 = 60_000;
/// The largest count a `componentCount` criterion may name.
pub const MAX_COMPONENT_COUNT: u32 = 1000;
/// A time is a multiple of the public step when it is this close to one (section 4.2).
const STEP_MULTIPLE_TOLERANCE: f64 = 1e-9;

/// A result slot a criterion reads: the layout key and the catalog unit of that slot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "CheckSignal.ts")]
pub struct CheckSignal {
    pub key: String,
    pub unit: Unit,
}

/// The state of a relay (`released`, `picked`) or a fuse (`intact`, `tripped`).
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "DeviceStateName.ts")]
pub enum DeviceStateName {
    Released,
    Picked,
    Intact,
    Tripped,
}

impl DeviceStateName {
    const fn of_relay(state: RelayContactState) -> Self {
        match state {
            RelayContactState::Released => Self::Released,
            RelayContactState::Picked => Self::Picked,
        }
    }

    const fn of_fuse(state: FuseState) -> Self {
        match state {
            FuseState::Intact => Self::Intact,
            FuseState::Tripped => Self::Tripped,
        }
    }

    const fn is_relay_state(self) -> bool {
        matches!(self, Self::Released | Self::Picked)
    }
}

/// One criterion of an assignment (section 4.1).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, export_to = "CheckCriterion.ts")]
pub enum CheckCriterion {
    // The signal's value at one public step is within the tolerance of `expected`.
    Value {
        id: StableId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        signal: CheckSignal,
        at_seconds: f64,
        expected: f64,
        tolerance_abs: f64,
        #[serde(default)]
        tolerance_rel: f64,
    },
    // At every public step from `from_seconds` to `to_seconds`, inclusive, `min <= value <= max`.
    Range {
        id: StableId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        signal: CheckSignal,
        from_seconds: f64,
        to_seconds: f64,
        min: f64,
        max: f64,
    },
    // The Engine-reported state of a relay or fuse at one public step equals `state`.
    DeviceState {
        id: StableId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        component_id: StableId,
        state: DeviceStateName,
        at_seconds: f64,
    },
    // The number of components of a type in the student's schematic is within `min..=max`.
    ComponentCount {
        id: StableId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        type_id: ComponentTypeId,
        min: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        max: Option<u32>,
    },
}

impl CheckCriterion {
    fn id(&self) -> &StableId {
        match self {
            Self::Value { id, .. }
            | Self::Range { id, .. }
            | Self::DeviceState { id, .. }
            | Self::ComponentCount { id, .. } => id,
        }
    }
}

/// What a check needs (section 4.6): the assignment's checks, restrictions, and starting
/// circuit, and the student's schematic with the pin geometry the editor derives.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "CheckRequest.ts")]
pub struct CheckRequest {
    pub checks: Vec<CheckCriterion>,
    // `restrictions.lockedComponents` of the assignment.
    pub locked_components: Vec<StableId>,
    pub starting_circuit: Schematic,
    pub schematic: Schematic,
    pub geometry: GeometryEnvelope,
    // The document revision the request was made for. It is echoed in the result so that the
    // application can discard a stale one.
    pub document_revision: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "CheckVerdict.ts")]
pub enum CheckVerdict {
    Pass,
    Fail,
    NotEvaluated,
}

impl CheckVerdict {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::NotEvaluated => "notEvaluated",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "CheckOutcome.ts")]
pub enum CheckOutcome {
    Passed,
    Partial,
    Failed,
    NotEvaluated,
}

impl CheckOutcome {
    const fn key(self) -> &'static str {
        match self {
            Self::Passed => "check.outcome.passed",
            Self::Partial => "check.outcome.partial",
            Self::Failed => "check.outcome.failed",
            Self::NotEvaluated => "check.outcome.notEvaluated",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "CheckCounts.ts")]
pub struct CheckCounts {
    pub pass: u32,
    pub fail: u32,
    pub not_evaluated: u32,
}

/// The verdict of one authored criterion. It repeats the criterion's own numbers, so a result
/// can be shown without the file. `measured` is absent when the verdict is `notEvaluated`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, export_to = "CriterionResult.ts")]
pub enum CriterionResult {
    Value {
        id: StableId,
        verdict: CheckVerdict,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        measured: Option<f64>,
        expected: f64,
        tolerance_abs: f64,
        tolerance_rel: f64,
        unit: Unit,
        at_seconds: f64,
        message_key: String,
        references: Vec<DiagnosticReference>,
    },
    Range {
        id: StableId,
        verdict: CheckVerdict,
        // The signal value at the step with the smallest margin to a bound.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        measured: Option<f64>,
        // The time of `measured`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        at_seconds: Option<f64>,
        min: f64,
        max: f64,
        unit: Unit,
        from_seconds: f64,
        to_seconds: f64,
        message_key: String,
        references: Vec<DiagnosticReference>,
    },
    DeviceState {
        id: StableId,
        verdict: CheckVerdict,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        measured: Option<DeviceStateName>,
        expected: DeviceStateName,
        at_seconds: f64,
        message_key: String,
        references: Vec<DiagnosticReference>,
    },
    ComponentCount {
        id: StableId,
        verdict: CheckVerdict,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        measured: Option<u32>,
        min: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        max: Option<u32>,
        message_key: String,
        references: Vec<DiagnosticReference>,
    },
}

impl CriterionResult {
    fn verdict(&self) -> CheckVerdict {
        match self {
            Self::Value { verdict, .. }
            | Self::Range { verdict, .. }
            | Self::DeviceState { verdict, .. }
            | Self::ComponentCount { verdict, .. } => *verdict,
        }
    }
}

/// The result of the implicit `locked` criterion (section 4.8): `pass` or `fail`, with the
/// locked components that are missing or changed as references.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "LockedResult.ts")]
pub struct LockedResult {
    pub verdict: CheckVerdict,
    pub message_key: String,
    pub references: Vec<DiagnosticReference>,
}

/// The result of a check (section 4.7). Every number in it is finite.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "CheckResult.ts")]
pub struct CheckResult {
    pub document_revision: String,
    pub outcome: CheckOutcome,
    pub counts: CheckCounts,
    pub criteria: Vec<CriterionResult>,
    pub locked: LockedResult,
    pub outcome_key: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// What a criterion needs, once its own fields are known to be evaluable.
#[derive(Clone, Copy, Debug)]
enum Plan {
    /// The criterion cannot be evaluated: its fields are out of bounds, or it is past the limit.
    Unevaluable,
    /// It reads the schematic only.
    Schematic,
    Value {
        step: u64,
    },
    Range {
        from: u64,
        to: u64,
    },
    State {
        step: u64,
    },
}

impl Plan {
    /// The last public step the criterion reads, if it needs a run.
    fn last_step(self) -> Option<u64> {
        match self {
            Self::Value { step } | Self::State { step } => Some(step),
            Self::Range { to, .. } => Some(to),
            Self::Unevaluable | Self::Schematic => None,
        }
    }
}

/// The signal value with the smallest margin to a bound so far, and when.
#[derive(Clone, Copy, Debug)]
struct Worst {
    margin: f64,
    value: f64,
    step: u64,
}

/// What a run found for one criterion.
#[derive(Clone, Copy, Debug, Default)]
struct Found {
    value: Option<f64>,
    worst: Option<Worst>,
    state: Option<DeviceStateName>,
}

/// Advances a started run by one public step, or reports why it stopped.
type StepFn<'a> = Box<dyn FnMut() -> Result<ResultSnapshot, Vec<Diagnostic>> + 'a>;

/// The start of a run: the circuit did not compile, or it did and can be stepped.
enum Start<'a> {
    Failed(Vec<Diagnostic>),
    Ready {
        layout: ResultLayout,
        snapshot: ResultSnapshot,
        diagnostics: Vec<Diagnostic>,
        advance: StepFn<'a>,
    },
}

/// Evaluates the checks of an assignment against a student's schematic.
///
/// The result depends only on the request. The circuit is compiled without faults in a fresh
/// `Engine` and advanced through the public steps the criteria need, at most 60 000.
pub fn evaluate_checks(request: CheckRequest) -> CheckResult {
    let mut engine = Engine::default();
    let start = match engine.compile(CompileRequest {
        schematic: request.schematic.clone(),
        geometry: request.geometry.clone(),
        document_revision: request.document_revision.clone(),
        faults: None,
    }) {
        CompileResult::Failure { diagnostics } => Start::Failed(diagnostics),
        CompileResult::Success {
            run_id,
            layout,
            initial_snapshot,
            diagnostics,
            ..
        } => Start::Ready {
            layout,
            snapshot: initial_snapshot,
            diagnostics,
            advance: Box::new(move || {
                engine.advance(AdvanceRequest {
                    run_id: run_id.clone(),
                    step_count: 1,
                })
            }),
        },
    };
    evaluate_started(request, start)
}

fn evaluate_started(request: CheckRequest, start: Start<'_>) -> CheckResult {
    let locked = evaluate_locked(
        &request.starting_circuit,
        &request.schematic,
        &request.locked_components,
    );
    let plans = request
        .checks
        .iter()
        .enumerate()
        .map(|(index, criterion)| {
            if index >= MAX_CHECK_CRITERIA {
                Plan::Unevaluable
            } else {
                plan(criterion)
            }
        })
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    let mut compiled = false;
    let mut found = vec![Found::default(); request.checks.len()];
    let mut reached = None;
    match start {
        Start::Failed(failure) => diagnostics.extend(failure),
        Start::Ready {
            layout,
            snapshot: initial_snapshot,
            diagnostics: compile_diagnostics,
            mut advance,
        } => {
            let has_errors = compile_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
            diagnostics.extend(compile_diagnostics);
            if !has_errors {
                compiled = true;
                let offsets = layout
                    .slots
                    .iter()
                    .map(|slot| (slot.key.as_str(), (slot.offset as usize, slot.unit)))
                    .collect::<HashMap<_, _>>();
                let last_step = plans.iter().filter_map(|plan| plan.last_step()).max();
                let mut snapshot = initial_snapshot;
                let mut step = 0;
                loop {
                    observe(
                        &request.checks,
                        &plans,
                        &offsets,
                        &snapshot,
                        step,
                        &mut found,
                    );
                    reached = Some(step);
                    if last_step.is_none_or(|last| step >= last) {
                        break;
                    }
                    match advance() {
                        Ok(next) => snapshot = next,
                        Err(stopped) => {
                            diagnostics.extend(stopped);
                            break;
                        }
                    }
                    step += 1;
                }
            }
        }
    }

    let criteria = request
        .checks
        .iter()
        .zip(&plans)
        .zip(&found)
        .map(|((criterion, plan), found)| {
            result_for(
                criterion,
                *plan,
                found,
                compiled.then_some(reached).flatten(),
                &request.schematic,
            )
        })
        .collect::<Vec<_>>();
    let counts = criteria
        .iter()
        .fold(CheckCounts::default(), |mut counts, result| {
            match result.verdict() {
                CheckVerdict::Pass => counts.pass += 1,
                CheckVerdict::Fail => counts.fail += 1,
                CheckVerdict::NotEvaluated => counts.not_evaluated += 1,
            }
            counts
        });
    let outcome = if locked.verdict == CheckVerdict::Fail {
        CheckOutcome::Failed
    } else if !compiled {
        CheckOutcome::NotEvaluated
    } else if criteria.is_empty() || counts.pass as usize == criteria.len() {
        CheckOutcome::Passed
    } else if counts.pass == 0 {
        CheckOutcome::Failed
    } else {
        CheckOutcome::Partial
    };

    CheckResult {
        document_revision: request.document_revision,
        outcome,
        counts,
        criteria,
        locked,
        outcome_key: outcome.key().to_owned(),
        diagnostics,
    }
}

/// Section 4.8: each locked component is in the student's schematic with the type and the
/// parameter values it has in the starting circuit. A component that is not in the starting
/// circuit cannot be compared and fails. Moving or rotating a part is allowed.
fn evaluate_locked(starting: &Schematic, student: &Schematic, locked: &[StableId]) -> LockedResult {
    let starting = starting
        .components
        .iter()
        .map(|component| (component.id().as_str(), component))
        .collect::<HashMap<_, _>>();
    let student = student
        .components
        .iter()
        .map(|component| (component.id().as_str(), component))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut references = Vec::new();
    for id in locked {
        if !seen.insert(id.as_str()) {
            continue;
        }
        let unchanged = match (starting.get(id.as_str()), student.get(id.as_str())) {
            (Some(original), Some(current)) => same_kind_and_parameters(original, current),
            _ => false,
        };
        if !unchanged {
            references.push(DiagnosticReference::Component {
                component_id: id.clone(),
            });
        }
    }
    let verdict = if references.is_empty() {
        CheckVerdict::Pass
    } else {
        CheckVerdict::Fail
    };
    LockedResult {
        verdict,
        message_key: format!("check.locked.{}", verdict.as_str()),
        references,
    }
}

/// Equal in type and in every parameter, ignoring the ID, position, and rotation.
fn same_kind_and_parameters(left: &ComponentSpec, right: &ComponentSpec) -> bool {
    match (left, right) {
        (
            ComponentSpec::Resistor { parameters: a, .. },
            ComponentSpec::Resistor { parameters: b, .. },
        ) => a == b,
        (
            ComponentSpec::DcVoltageSource { parameters: a, .. },
            ComponentSpec::DcVoltageSource { parameters: b, .. },
        ) => a == b,
        (
            ComponentSpec::Capacitor { parameters: a, .. },
            ComponentSpec::Capacitor { parameters: b, .. },
        ) => a == b,
        (
            ComponentSpec::Inductor { parameters: a, .. },
            ComponentSpec::Inductor { parameters: b, .. },
        ) => a == b,
        (
            ComponentSpec::RelaySpdt { parameters: a, .. },
            ComponentSpec::RelaySpdt { parameters: b, .. },
        ) => a == b,
        (ComponentSpec::Fuse { parameters: a, .. }, ComponentSpec::Fuse { parameters: b, .. }) => {
            a == b
        }
        (
            ComponentSpec::PmdcMotor { parameters: a, .. },
            ComponentSpec::PmdcMotor { parameters: b, .. },
        ) => a == b,
        (
            ComponentSpec::Voltmeter { parameters: a, .. },
            ComponentSpec::Voltmeter { parameters: b, .. },
        ) => a == b,
        (
            ComponentSpec::Ammeter { parameters: a, .. },
            ComponentSpec::Ammeter { parameters: b, .. },
        ) => a == b,
        _ => false,
    }
}

/// The public step of a time (section 4.2), or `None` if the time is not a non-negative
/// multiple of the public step of at most 60 s.
pub(crate) fn step_of(seconds: f64) -> Option<u64> {
    if !seconds.is_finite() || !(0.0..=MAX_CHECK_SECONDS).contains(&seconds) {
        return None;
    }
    let ratio = seconds / SIMULATION_TIMESTEP_SECONDS;
    let step = ratio.round();
    if (ratio - step).abs() > STEP_MULTIPLE_TOLERANCE {
        return None;
    }
    let step = step as u64;
    (step <= MAX_CHECK_STEPS).then_some(step)
}

fn plan(criterion: &CheckCriterion) -> Plan {
    match criterion {
        CheckCriterion::Value {
            at_seconds,
            expected,
            tolerance_abs,
            tolerance_rel,
            ..
        } => match step_of(*at_seconds) {
            Some(step)
                if expected.is_finite()
                    && tolerance_abs.is_finite()
                    && *tolerance_abs >= 0.0
                    && tolerance_rel.is_finite()
                    && *tolerance_rel >= 0.0 =>
            {
                Plan::Value { step }
            }
            _ => Plan::Unevaluable,
        },
        CheckCriterion::Range {
            from_seconds,
            to_seconds,
            min,
            max,
            ..
        } => match (step_of(*from_seconds), step_of(*to_seconds)) {
            (Some(from), Some(to))
                if from <= to && min.is_finite() && max.is_finite() && min <= max =>
            {
                Plan::Range { from, to }
            }
            _ => Plan::Unevaluable,
        },
        CheckCriterion::DeviceState { at_seconds, .. } => match step_of(*at_seconds) {
            Some(step) => Plan::State { step },
            None => Plan::Unevaluable,
        },
        CheckCriterion::ComponentCount { min, max, .. } => {
            let bounded = *min <= MAX_COMPONENT_COUNT
                && max.is_none_or(|max| max >= *min && max <= MAX_COMPONENT_COUNT);
            if bounded {
                Plan::Schematic
            } else {
                Plan::Unevaluable
            }
        }
    }
}

/// Records what the criteria read at one public step.
fn observe(
    checks: &[CheckCriterion],
    plans: &[Plan],
    offsets: &HashMap<&str, (usize, Unit)>,
    snapshot: &ResultSnapshot,
    step: u64,
    found: &mut [Found],
) {
    for ((criterion, plan), found) in checks.iter().zip(plans).zip(found.iter_mut()) {
        match (criterion, plan) {
            (CheckCriterion::Value { signal, .. }, Plan::Value { step: at }) if *at == step => {
                found.value = read_signal(offsets, signal, snapshot);
            }
            (
                CheckCriterion::Range {
                    signal, min, max, ..
                },
                Plan::Range { from, to },
            ) if (*from..=*to).contains(&step) => {
                if let Some(value) = read_signal(offsets, signal, snapshot) {
                    let margin = (value - min).min(max - value);
                    if found.worst.is_none_or(|worst| margin < worst.margin) {
                        found.worst = Some(Worst {
                            margin,
                            value,
                            step,
                        });
                    }
                }
            }
            (
                CheckCriterion::DeviceState {
                    component_id,
                    state,
                    ..
                },
                Plan::State { step: at },
            ) if *at == step => {
                found.state = read_state(snapshot, component_id, *state);
            }
            _ => {}
        }
    }
}

/// The finite value of a signal, if its key is in the layout with the named unit.
fn read_signal(
    offsets: &HashMap<&str, (usize, Unit)>,
    signal: &CheckSignal,
    snapshot: &ResultSnapshot,
) -> Option<f64> {
    let (offset, unit) = offsets.get(signal.key.as_str())?;
    if *unit != signal.unit {
        return None;
    }
    snapshot
        .values
        .get(*offset)
        .copied()
        .filter(|value| value.is_finite())
}

/// The state of a relay or fuse, if the component has the kind of state `expected` belongs to.
fn read_state(
    snapshot: &ResultSnapshot,
    component_id: &StableId,
    expected: DeviceStateName,
) -> Option<DeviceStateName> {
    snapshot.states.iter().find_map(|state| match state {
        DeviceState::Relay {
            component_id: id,
            state,
            ..
        } if id == component_id && expected.is_relay_state() => {
            Some(DeviceStateName::of_relay(*state))
        }
        DeviceState::Fuse {
            component_id: id,
            state,
            ..
        } if id == component_id && !expected.is_relay_state() => {
            Some(DeviceStateName::of_fuse(*state))
        }
        _ => None,
    })
}

/// A number from a criterion for the result: a non-finite one is shown as zero, so a result
/// never carries a number that is not finite (the criterion is `notEvaluated` anyway).
fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

fn message_key(kind: &str, verdict: CheckVerdict) -> String {
    format!("check.{kind}.{}", verdict.as_str())
}

/// The component a signal key names: the key is `<componentId>:<suffix>` or
/// `<componentId>/<suffix>`.
fn signal_reference(signal: &CheckSignal) -> Vec<DiagnosticReference> {
    let id = signal
        .key
        .split([':', '/'])
        .next()
        .and_then(|prefix| StableId::parse(prefix).ok());
    id.map(|component_id| DiagnosticReference::Component { component_id })
        .into_iter()
        .collect()
}

/// The verdict of one criterion. `reached` is the last public step the run computed, or
/// `None` if there was no run.
fn result_for(
    criterion: &CheckCriterion,
    plan: Plan,
    found: &Found,
    reached: Option<u64>,
    schematic: &Schematic,
) -> CriterionResult {
    let id = criterion.id().clone();
    let has_run_to = |step: u64| reached.is_some_and(|reached| reached >= step);
    match criterion {
        CheckCriterion::Value {
            signal,
            at_seconds,
            expected,
            tolerance_abs,
            tolerance_rel,
            ..
        } => {
            let measured = match plan {
                Plan::Value { step } if has_run_to(step) => found.value,
                _ => None,
            };
            let verdict = match measured {
                None => CheckVerdict::NotEvaluated,
                Some(measured)
                    if (measured - expected).abs()
                        <= tolerance_abs + tolerance_rel * expected.abs() =>
                {
                    CheckVerdict::Pass
                }
                Some(_) => CheckVerdict::Fail,
            };
            CriterionResult::Value {
                id,
                verdict,
                measured,
                expected: finite(*expected),
                tolerance_abs: finite(*tolerance_abs),
                tolerance_rel: finite(*tolerance_rel),
                unit: signal.unit,
                at_seconds: finite(*at_seconds),
                message_key: message_key("value", verdict),
                references: signal_reference(signal),
            }
        }
        CheckCriterion::Range {
            signal,
            from_seconds,
            to_seconds,
            min,
            max,
            ..
        } => {
            // The whole window must have been run through.
            let worst = match plan {
                Plan::Range { to, .. } if has_run_to(to) => found.worst,
                _ => None,
            };
            let verdict = match worst {
                None => CheckVerdict::NotEvaluated,
                Some(worst) if worst.margin >= 0.0 => CheckVerdict::Pass,
                Some(_) => CheckVerdict::Fail,
            };
            CriterionResult::Range {
                id,
                verdict,
                measured: worst.map(|worst| worst.value),
                at_seconds: worst.map(|worst| worst.step as f64 * SIMULATION_TIMESTEP_SECONDS),
                min: finite(*min),
                max: finite(*max),
                unit: signal.unit,
                from_seconds: finite(*from_seconds),
                to_seconds: finite(*to_seconds),
                message_key: message_key("range", verdict),
                references: signal_reference(signal),
            }
        }
        CheckCriterion::DeviceState {
            component_id,
            state,
            at_seconds,
            ..
        } => {
            let measured = match plan {
                Plan::State { step } if has_run_to(step) => found.state,
                _ => None,
            };
            let verdict = match measured {
                None => CheckVerdict::NotEvaluated,
                Some(measured) if measured == *state => CheckVerdict::Pass,
                Some(_) => CheckVerdict::Fail,
            };
            CriterionResult::DeviceState {
                id,
                verdict,
                measured,
                expected: *state,
                at_seconds: finite(*at_seconds),
                message_key: message_key("deviceState", verdict),
                references: vec![DiagnosticReference::Component {
                    component_id: component_id.clone(),
                }],
            }
        }
        CheckCriterion::ComponentCount {
            type_id, min, max, ..
        } => {
            let mut counted = schematic
                .components
                .iter()
                .filter(|component| component.type_id() == *type_id)
                .map(|component| component.id().clone())
                .collect::<Vec<_>>();
            counted.sort_by(|left, right| left.as_str().cmp(right.as_str()));
            let count = u32::try_from(counted.len()).unwrap_or(u32::MAX);
            let measured = matches!(plan, Plan::Schematic).then_some(count);
            let verdict = match measured {
                None => CheckVerdict::NotEvaluated,
                Some(count) if count >= *min && max.is_none_or(|max| count <= max) => {
                    CheckVerdict::Pass
                }
                Some(_) => CheckVerdict::Fail,
            };
            CriterionResult::ComponentCount {
                id,
                verdict,
                measured,
                min: *min,
                max: *max,
                message_key: message_key("componentCount", verdict),
                references: counted
                    .into_iter()
                    .map(|component_id| DiagnosticReference::Component { component_id })
                    .collect(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::{DiagnosticCode, FrameMetadata, ResultQuantity, ResultSignConvention, ResultSlot};

    const SIGNAL: &str = "00000000-0000-4000-8000-000000000001:voltage";

    #[test]
    fn a_time_is_a_multiple_of_the_public_step_of_at_most_60_seconds() {
        assert_eq!(step_of(0.0), Some(0));
        assert_eq!(step_of(-0.0), Some(0));
        assert_eq!(step_of(0.001), Some(1));
        assert_eq!(step_of(0.3), Some(300));
        assert_eq!(step_of(59.999), Some(59_999));
        assert_eq!(step_of(60.0), Some(60_000));
        assert_eq!(step_of(60.001), None);
        assert_eq!(step_of(0.0005), None);
        assert_eq!(step_of(0.0015), None);
        assert_eq!(step_of(-0.001), None);
        assert_eq!(step_of(f64::NAN), None);
        assert_eq!(step_of(f64::INFINITY), None);
    }

    fn id(suffix: u32) -> StableId {
        StableId::parse(format!("00000000-0000-4000-8000-{suffix:012x}")).unwrap()
    }

    fn signal() -> CheckSignal {
        CheckSignal {
            key: SIGNAL.to_owned(),
            unit: Unit::Volt,
        }
    }

    fn empty_schematic() -> Schematic {
        Schematic {
            schema_version: 1,
            components: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            reference_markers: Vec::new(),
        }
    }

    fn request(checks: Vec<CheckCriterion>) -> CheckRequest {
        CheckRequest {
            checks,
            locked_components: Vec::new(),
            starting_circuit: empty_schematic(),
            schematic: empty_schematic(),
            geometry: GeometryEnvelope {
                components: Vec::new(),
            },
            document_revision: "rev".to_owned(),
        }
    }

    fn snapshot(step: u64) -> ResultSnapshot {
        ResultSnapshot {
            metadata: FrameMetadata {
                run_id: id(1),
                document_revision: "rev".to_owned(),
                layout_version: 1,
                simulation_step: step,
                simulation_time_seconds: step as f64 * SIMULATION_TIMESTEP_SECONDS,
                frame_sequence: step,
            },
            // The signal equals its step number.
            values: vec![step as f64],
            states: Vec::new(),
        }
    }

    fn layout() -> ResultLayout {
        ResultLayout {
            layout_version: 1,
            value_count: 1,
            slots: vec![ResultSlot {
                key: SIGNAL.to_owned(),
                component_id: id(1),
                quantity: ResultQuantity::Voltage,
                unit: Unit::Volt,
                sign_convention: ResultSignConvention::VoltagePin1MinusPin2,
                offset: 0,
            }],
        }
    }

    fn value(suffix: u32, at_seconds: f64, expected: f64) -> CheckCriterion {
        CheckCriterion::Value {
            id: id(suffix),
            label: None,
            signal: signal(),
            at_seconds,
            expected,
            tolerance_abs: 0.0,
            tolerance_rel: 0.0,
        }
    }

    fn range(
        suffix: u32,
        from_seconds: f64,
        to_seconds: f64,
        min: f64,
        max: f64,
    ) -> CheckCriterion {
        CheckCriterion::Range {
            id: id(suffix),
            label: None,
            signal: signal(),
            from_seconds,
            to_seconds,
            min,
            max,
        }
    }

    /// A run whose steps 1 to `last_good` succeed and whose next step fails with a chatter
    /// error. `calls` counts the steps asked for.
    fn stopping_run(last_good: u64, calls: &Cell<u64>) -> Start<'_> {
        let mut step = 0;
        Start::Ready {
            layout: layout(),
            snapshot: snapshot(0),
            diagnostics: Vec::new(),
            advance: Box::new(move || {
                calls.set(calls.get() + 1);
                step += 1;
                if step <= last_good {
                    Ok(snapshot(step))
                } else {
                    Err(vec![Diagnostic::error(
                        DiagnosticCode::EventChatter,
                        Vec::new(),
                    )])
                }
            }),
        }
    }

    #[test]
    fn a_run_that_stops_early_leaves_only_the_later_criteria_unevaluated() {
        let calls = Cell::new(0);
        let checks = vec![
            value(10, 0.002, 2.0),
            value(11, 0.003, 3.0),
            value(12, 0.004, 4.0),
            range(13, 0.001, 0.003, 1.0, 3.0),
            range(14, 0.002, 0.006, 0.0, 9.0),
        ];
        let result = evaluate_started(request(checks), stopping_run(3, &calls));

        let verdicts = result
            .criteria
            .iter()
            .map(CriterionResult::verdict)
            .collect::<Vec<_>>();
        assert_eq!(
            verdicts,
            vec![
                CheckVerdict::Pass,
                CheckVerdict::Pass,
                CheckVerdict::NotEvaluated,
                CheckVerdict::Pass,
                CheckVerdict::NotEvaluated,
            ]
        );
        // The failure is reported, the circuit did compile, and the run is not asked again.
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            vec![DiagnosticCode::EventChatter]
        );
        assert_eq!(result.outcome, CheckOutcome::Partial);
        assert_eq!(calls.get(), 4);
    }

    #[test]
    fn the_run_is_stepped_only_as_far_as_the_latest_criterion_needs() {
        let calls = Cell::new(0);
        let result = evaluate_started(
            request(vec![value(10, 0.005, 5.0), value(11, 0.0, 0.0)]),
            stopping_run(100, &calls),
        );

        assert_eq!(result.outcome, CheckOutcome::Passed);
        assert_eq!(calls.get(), 5);

        let calls = Cell::new(0);
        let result = evaluate_started(
            request(vec![CheckCriterion::ComponentCount {
                id: id(10),
                label: None,
                type_id: ComponentTypeId::Resistor,
                min: 0,
                max: None,
            }]),
            stopping_run(100, &calls),
        );
        assert_eq!(result.outcome, CheckOutcome::Passed);
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn a_run_that_cannot_take_its_first_step_still_gives_step_zero() {
        let calls = Cell::new(0);
        let result = evaluate_started(
            request(vec![value(10, 0.0, 0.0), value(11, 0.001, 1.0)]),
            stopping_run(0, &calls),
        );

        assert_eq!(result.criteria[0].verdict(), CheckVerdict::Pass);
        assert_eq!(result.criteria[1].verdict(), CheckVerdict::NotEvaluated);
        assert_eq!(result.outcome, CheckOutcome::Partial);
    }

    #[test]
    fn an_error_diagnostic_of_a_started_run_is_a_failed_compile() {
        let start = Start::Ready {
            layout: layout(),
            snapshot: snapshot(0),
            diagnostics: vec![Diagnostic::error(DiagnosticCode::FloatingNode, Vec::new())],
            advance: Box::new(|| panic!("a run with an error is not stepped")),
        };
        let result = evaluate_started(request(vec![value(10, 0.0, 0.0)]), start);

        assert_eq!(result.outcome, CheckOutcome::NotEvaluated);
        assert_eq!(result.criteria[0].verdict(), CheckVerdict::NotEvaluated);
    }
}
