//! Assignments (docs/contracts/stage-5-assignments.md sections 2, 3, 5.7 and 7.2).
//!
//! An assignment is a teacher's task: text, a starting circuit, what is locked, what is checked
//! or which faults are hidden. This module owns its types, the validation of a file or an
//! edited document (without solving any circuit), the student's copy, and the comparison of a
//! fault-finding diagnosis with the faults. File access belongs to the application.
//!
//! Validation reports one error per problem, each with the JSON pointer of the field, in the
//! order of the contract's field table (`schemaVersion`, `id`, `title`, `instructions`,
//! `task`, `startingCircuit`, `restrictions`, `checks`, `faults`, `reference`) and, inside a
//! list, in list order. A pointer into a schematic names the schematic part (a component, or
//! the schematic itself) and not the field inside it.

use std::collections::{HashMap, HashSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::checks::{MAX_COMPONENT_COUNT, step_of};
use crate::faults::FaultOverlay;
use crate::slots::slot_for_key_rest;
use crate::{
    CheckCriterion, ComponentTypeId, DiagnosticCode, DiagnosticReference, FaultKind, FaultSpec,
    MAX_CHECK_CRITERIA, ParameterId, Schematic, StableId, Unit, component_catalog,
};

/// The only supported `schemaVersion`.
pub const ASSIGNMENT_SCHEMA_VERSION: u64 = 1;
/// The largest assignment file, in bytes.
pub const MAX_ASSIGNMENT_BYTES: usize = 4 * 1024 * 1024;
/// The most faults of a fault-finding task, and the most entries of a diagnosis.
pub const MAX_ASSIGNMENT_FAULTS: usize = 8;
/// The most errors one validation reports.
pub const MAX_ASSIGNMENT_ERRORS: usize = 32;
const MAX_TITLE_CHARS: usize = 120;
const MAX_INSTRUCTIONS_CHARS: usize = 8000;
const MAX_LABEL_CHARS: usize = 120;

/// What the application shows of a fault-finding result (section 5.7). A presentation policy
/// of the local application, not a protection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "RevealPolicy.ts")]
pub enum RevealPolicy {
    Never,
    AfterCheck,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, export_to = "AssignmentTask.ts")]
pub enum AssignmentTask {
    /// The student edits a copy of the starting circuit, which the Engine checks.
    Build,
    /// The student investigates a faulty circuit and submits a diagnosis.
    FindFault { reveal: RevealPolicy },
}

/// Editor restrictions (section 3). The defaults are every type and no locked component.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "Restrictions.ts")]
pub struct Restrictions {
    // `null` or absent: every catalog type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub allowed_component_types: Option<Vec<ComponentTypeId>>,
    #[serde(default)]
    pub locked_components: Vec<StableId>,
}

/// A teacher's task (section 2.1). `checks` belongs to a `build` task and `faults` to a
/// `findFault` task; validation refuses any other combination.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "Assignment.ts")]
pub struct Assignment {
    #[ts(type = "1")]
    pub schema_version: u32,
    pub id: StableId,
    pub title: String,
    pub instructions: String,
    pub task: AssignmentTask,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub starting_circuit: Option<Schematic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub restrictions: Option<Restrictions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub checks: Option<Vec<CheckCriterion>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub faults: Option<Vec<FaultSpec>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference: Option<Schematic>,
}

impl Assignment {
    /// The student's copy (section 2.3): the same assignment without the teacher's `reference`.
    pub fn student_copy(&self) -> Self {
        Self {
            reference: None,
            ..self.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "PascalCase")]
#[ts(export, export_to = "AssignmentErrorCode.ts")]
pub enum AssignmentErrorCode {
    Malformed,
    UnsupportedVersion,
    UnknownField,
    InvalidField,
    DuplicateId,
    UnknownReference,
    TaskMismatch,
    TooLarge,
}

impl AssignmentErrorCode {
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::Malformed => "assignment.error.malformed",
            Self::UnsupportedVersion => "assignment.error.unsupportedVersion",
            Self::UnknownField => "assignment.error.unknownField",
            Self::InvalidField => "assignment.error.invalidField",
            Self::DuplicateId => "assignment.error.duplicateId",
            Self::UnknownReference => "assignment.error.unknownReference",
            Self::TaskMismatch => "assignment.error.taskMismatch",
            Self::TooLarge => "assignment.error.tooLarge",
        }
    }
}

/// One problem of a file or a document (section 7.2): a code, the JSON pointer of the field
/// (empty for the whole document), and a localization key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "AssignmentError.ts")]
pub struct AssignmentError {
    pub code: AssignmentErrorCode,
    pub path: String,
    pub message_key: String,
}

impl AssignmentError {
    pub fn new(code: AssignmentErrorCode, path: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            message_key: code.message_key().to_owned(),
        }
    }
}

/// The result of validating a file or a document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export, export_to = "AssignmentDecoded.ts")]
pub enum AssignmentDecoded {
    Valid { assignment: Box<Assignment> },
    Invalid { errors: Vec<AssignmentError> },
}

impl AssignmentDecoded {
    pub fn from_result(result: Result<Assignment, Vec<AssignmentError>>) -> Self {
        match result {
            Ok(assignment) => Self::Valid {
                assignment: Box::new(assignment),
            },
            Err(errors) => Self::Invalid { errors },
        }
    }
}

/// The result of saving or exporting an assignment.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export, export_to = "AssignmentSaveOutcome.ts")]
pub enum AssignmentSaveOutcome {
    Saved,
    /// The user closed the file dialog; nothing was written.
    Cancelled,
    /// The document is not a valid assignment; nothing was written.
    Invalid {
        errors: Vec<AssignmentError>,
    },
}

/// Validates an assignment file's bytes and decodes them (section 2.5).
pub fn decode_assignment_json(bytes: &[u8]) -> Result<Assignment, Vec<AssignmentError>> {
    if bytes.len() > MAX_ASSIGNMENT_BYTES {
        return Err(vec![AssignmentError::new(
            AssignmentErrorCode::TooLarge,
            "",
        )]);
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| vec![AssignmentError::new(AssignmentErrorCode::Malformed, "")])?;
    validate_assignment_value(&value)
}

/// Validates an assignment document and returns it as the typed assignment.
pub fn validate_assignment_value(value: &Value) -> Result<Assignment, Vec<AssignmentError>> {
    let mut report = Report::default();
    let Some(root) = value.as_object() else {
        return Err(vec![AssignmentError::new(
            AssignmentErrorCode::Malformed,
            "",
        )]);
    };
    if root.get("schemaVersion").and_then(Value::as_u64) != Some(ASSIGNMENT_SCHEMA_VERSION) {
        return Err(vec![AssignmentError::new(
            AssignmentErrorCode::UnsupportedVersion,
            "/schemaVersion",
        )]);
    }
    report.unknown_fields(root, &ROOT_FIELDS, "");

    if let Some(id) = root.get("id") {
        report.stable_id(id, "/id");
    } else {
        report.push(AssignmentErrorCode::InvalidField, "/id");
    }
    report.text(
        root.get("title"),
        "/title",
        TextLimits {
            min: 1,
            max: MAX_TITLE_CHARS,
            required: true,
            forbid_control: true,
        },
    );
    // Instructions are plain text with line breaks (section 2.1).
    report.text(
        root.get("instructions"),
        "/instructions",
        TextLimits {
            min: 0,
            max: MAX_INSTRUCTIONS_CHARS,
            required: true,
            forbid_control: false,
        },
    );
    let kind = report.task(root.get("task"));

    let starting = root.get("startingCircuit");
    let starting_ids = starting.map(|schematic| report.schematic(schematic, "/startingCircuit"));
    if starting.is_none() && kind == Some(TaskKind::FindFault) {
        report.push(AssignmentErrorCode::TaskMismatch, "/startingCircuit");
    }
    let context = Context::new(starting.and_then(Value::as_object));

    report.restrictions(root.get("restrictions"), &context);

    let mut shared_ids = HashSet::new();
    shared_ids.extend(starting_ids.into_iter().flatten());
    // The reference is the last field, but its IDs are needed to judge the criteria and the
    // faults, so it is read first into a report of its own that is appended at the end.
    let mut reference_report = Report::default();
    let reference_ids = root
        .get("reference")
        .map(|schematic| reference_report.schematic(schematic, "/reference"));
    shared_ids.extend(reference_ids.into_iter().flatten());

    let mut authored_ids = HashSet::new();
    match kind {
        Some(TaskKind::Build) => {
            report.checks(root.get("checks"), &context, &shared_ids, &mut authored_ids);
            if root.contains_key("faults") {
                report.push(AssignmentErrorCode::TaskMismatch, "/faults");
            }
        }
        Some(TaskKind::FindFault) => {
            if root.contains_key("checks") {
                report.push(AssignmentErrorCode::TaskMismatch, "/checks");
            }
            report.faults(root.get("faults"), &context, &shared_ids, &mut authored_ids);
        }
        None => {}
    }

    report.errors.extend(reference_report.errors);
    report.errors.truncate(MAX_ASSIGNMENT_ERRORS);
    if !report.errors.is_empty() {
        return Err(report.errors);
    }
    let assignment: Assignment = serde_json::from_value(value.clone())
        .map_err(|_| vec![AssignmentError::new(AssignmentErrorCode::InvalidField, "")])?;
    check_faults_against_the_circuit(&assignment)?;
    Ok(assignment)
}

/// Validates a typed assignment: the same rules as a file.
pub fn validate_assignment(assignment: &Assignment) -> Result<(), Vec<AssignmentError>> {
    let value = serde_json::to_value(assignment)
        .map_err(|_| vec![AssignmentError::new(AssignmentErrorCode::InvalidField, "")])?;
    validate_assignment_value(&value).map(|_| ())
}

/// Encodes a valid assignment as the bytes of an assignment file: readable JSON with a final
/// newline. An invalid or oversized assignment is refused.
pub fn encode_assignment_json(assignment: &Assignment) -> Result<Vec<u8>, Vec<AssignmentError>> {
    validate_assignment(assignment)?;
    let mut bytes = serde_json::to_vec_pretty(assignment)
        .map_err(|_| vec![AssignmentError::new(AssignmentErrorCode::Malformed, "")])?;
    bytes.push(b'\n');
    if bytes.len() > MAX_ASSIGNMENT_BYTES {
        return Err(vec![AssignmentError::new(
            AssignmentErrorCode::TooLarge,
            "",
        )]);
    }
    Ok(bytes)
}

const ROOT_FIELDS: [&str; 10] = [
    "schemaVersion",
    "id",
    "title",
    "instructions",
    "task",
    "startingCircuit",
    "restrictions",
    "checks",
    "faults",
    "reference",
];
const SCHEMATIC_FIELDS: [&str; 5] = [
    "schemaVersion",
    "components",
    "wires",
    "junctions",
    "referenceMarkers",
];
const SCHEMATIC_LISTS: [&str; 4] = ["components", "wires", "junctions", "referenceMarkers"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TaskKind {
    Build,
    FindFault,
}

/// What the starting circuit contains, read from the raw document so that a criterion can be
/// judged even when another part of the file is wrong.
struct Context {
    types: HashMap<String, ComponentTypeId>,
}

impl Context {
    fn new(starting: Option<&Map<String, Value>>) -> Self {
        let mut types = HashMap::new();
        let components = starting
            .and_then(|schematic| schematic.get("components"))
            .and_then(Value::as_array);
        for component in components.into_iter().flatten() {
            let id = component.get("id").and_then(Value::as_str);
            let type_id = component
                .get("typeId")
                .and_then(Value::as_str)
                .and_then(parse_enum::<ComponentTypeId>);
            if let (Some(id), Some(type_id)) = (id, type_id) {
                types.insert(id.to_owned(), type_id);
            }
        }
        Self { types }
    }

    fn type_of(&self, component_id: &str) -> Option<ComponentTypeId> {
        self.types.get(component_id).copied()
    }
}

fn parse_enum<T: DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_value(Value::String(text.to_owned())).ok()
}

fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn child(base: &str, segment: impl std::fmt::Display) -> String {
    format!("{base}/{}", escape(&segment.to_string()))
}

/// The rules for a text field.
#[derive(Clone, Copy)]
struct TextLimits {
    min: usize,
    max: usize,
    required: bool,
    forbid_control: bool,
}

#[derive(Default)]
struct Report {
    errors: Vec<AssignmentError>,
}

impl Report {
    fn push(&mut self, code: AssignmentErrorCode, path: impl Into<String>) {
        if self.errors.len() < MAX_ASSIGNMENT_ERRORS {
            self.errors.push(AssignmentError::new(code, path));
        }
    }

    fn unknown_fields(&mut self, object: &Map<String, Value>, known: &[&str], base: &str) {
        for key in object.keys() {
            if !known.contains(&key.as_str()) {
                self.push(AssignmentErrorCode::UnknownField, child(base, key));
            }
        }
    }

    fn stable_id(&mut self, value: &Value, path: &str) -> Option<StableId> {
        let id = value.as_str().and_then(|text| StableId::parse(text).ok());
        if id.is_none() {
            self.push(AssignmentErrorCode::InvalidField, path);
        }
        id
    }

    /// A string of `min..=max` characters. A missing required field is an error.
    fn text(&mut self, value: Option<&Value>, path: &str, limits: TextLimits) {
        match value {
            None if limits.required => self.push(AssignmentErrorCode::InvalidField, path),
            None => {}
            Some(Value::String(text)) => {
                let length = text.chars().count();
                let has_control = limits.forbid_control && text.chars().any(char::is_control);
                if length < limits.min || length > limits.max || has_control {
                    self.push(AssignmentErrorCode::InvalidField, path);
                }
            }
            Some(_) => self.push(AssignmentErrorCode::InvalidField, path),
        }
    }

    fn finite(&mut self, value: Option<&Value>, path: &str, required: bool) -> Option<f64> {
        match value {
            None => {
                if required {
                    self.push(AssignmentErrorCode::InvalidField, path);
                }
                None
            }
            Some(value) => {
                let number = value.as_f64().filter(|number| number.is_finite());
                if number.is_none() {
                    self.push(AssignmentErrorCode::InvalidField, path);
                }
                number
            }
        }
    }

    /// An integer from `min` to `max`, inclusive.
    fn count(&mut self, value: Option<&Value>, path: &str, min: u64, max: u64) -> Option<u64> {
        let number = value
            .and_then(Value::as_u64)
            .filter(|number| (min..=max).contains(number));
        if number.is_none() {
            self.push(AssignmentErrorCode::InvalidField, path);
        }
        number
    }

    fn task(&mut self, value: Option<&Value>) -> Option<TaskKind> {
        let Some(task) = value.and_then(Value::as_object) else {
            self.push(AssignmentErrorCode::InvalidField, "/task");
            return None;
        };
        self.unknown_fields(task, &["kind", "reveal"], "/task");
        let kind = match task.get("kind").and_then(Value::as_str) {
            Some("build") => TaskKind::Build,
            Some("findFault") => TaskKind::FindFault,
            _ => {
                self.push(AssignmentErrorCode::InvalidField, "/task/kind");
                return None;
            }
        };
        match (kind, task.get("reveal")) {
            (TaskKind::Build, Some(_)) => {
                self.push(AssignmentErrorCode::TaskMismatch, "/task/reveal");
            }
            (TaskKind::FindFault, None) => {
                self.push(AssignmentErrorCode::TaskMismatch, "/task/reveal");
            }
            (TaskKind::FindFault, Some(reveal)) => {
                if !matches!(reveal.as_str(), Some("never" | "afterCheck")) {
                    self.push(AssignmentErrorCode::InvalidField, "/task/reveal");
                }
            }
            (TaskKind::Build, None) => {}
        }
        Some(kind)
    }

    /// Checks a schematic and returns the IDs it uses, whatever else is wrong with it.
    fn schematic(&mut self, value: &Value, path: &str) -> Vec<String> {
        let mut ids = Vec::new();
        let Some(object) = value.as_object() else {
            self.push(AssignmentErrorCode::InvalidField, path);
            return ids;
        };
        if object.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
            self.push(
                AssignmentErrorCode::UnsupportedVersion,
                child(path, "schemaVersion"),
            );
            return ids;
        }
        let before = self.errors.len();
        self.unknown_fields(object, &SCHEMATIC_FIELDS, path);
        let mut seen = HashSet::new();
        for list in SCHEMATIC_LISTS {
            let items = match object.get(list) {
                Some(Value::Array(items)) => items,
                _ => {
                    self.push(AssignmentErrorCode::InvalidField, child(path, list));
                    continue;
                }
            };
            for (index, item) in items.iter().enumerate() {
                let item_path = child(&child(path, list), index);
                let id = item.get("id").and_then(Value::as_str);
                match id.filter(|id| StableId::parse(*id).is_ok()) {
                    None => self.push(AssignmentErrorCode::InvalidField, child(&item_path, "id")),
                    Some(id) => {
                        ids.push(id.to_owned());
                        if !seen.insert(id.to_owned()) {
                            self.push(AssignmentErrorCode::DuplicateId, child(&item_path, "id"));
                        }
                    }
                }
            }
        }
        if self.errors.len() > before {
            return ids;
        }

        let schematic: Schematic = match serde_json::from_value(value.clone()) {
            Ok(schematic) => schematic,
            Err(error) => {
                let code = if error.to_string().starts_with("unknown field") {
                    AssignmentErrorCode::UnknownField
                } else {
                    AssignmentErrorCode::InvalidField
                };
                self.push(code, path);
                return ids;
            }
        };
        for diagnostic in schematic.validate() {
            if diagnostic.severity != crate::DiagnosticSeverity::Error
                || diagnostic.code == DiagnosticCode::DuplicateId
            {
                continue;
            }
            let component = diagnostic.references.iter().find_map(|reference| {
                let DiagnosticReference::Component { component_id } = reference else {
                    return None;
                };
                schematic
                    .components
                    .iter()
                    .position(|component| component.id() == component_id)
            });
            let at = match component {
                Some(index) => child(&child(path, "components"), index),
                None => path.to_owned(),
            };
            self.push(AssignmentErrorCode::InvalidField, at);
        }
        ids
    }

    fn restrictions(&mut self, value: Option<&Value>, context: &Context) {
        let Some(value) = value else {
            return;
        };
        let Some(restrictions) = value.as_object() else {
            self.push(AssignmentErrorCode::InvalidField, "/restrictions");
            return;
        };
        self.unknown_fields(
            restrictions,
            &["allowedComponentTypes", "lockedComponents"],
            "/restrictions",
        );
        match restrictions.get("allowedComponentTypes") {
            None | Some(Value::Null) => {}
            Some(Value::Array(types)) => {
                let path = "/restrictions/allowedComponentTypes";
                if types.len() > component_catalog().component_types.len() {
                    self.push(AssignmentErrorCode::TooLarge, path);
                } else {
                    for (index, type_id) in types.iter().enumerate() {
                        let item = child(path, index);
                        match type_id.as_str() {
                            None => self.push(AssignmentErrorCode::InvalidField, item),
                            Some(text) if parse_enum::<ComponentTypeId>(text).is_none() => {
                                self.push(AssignmentErrorCode::UnknownReference, item);
                            }
                            Some(_) => {}
                        }
                    }
                }
            }
            Some(_) => self.push(
                AssignmentErrorCode::InvalidField,
                "/restrictions/allowedComponentTypes",
            ),
        }
        match restrictions.get("lockedComponents") {
            None => {}
            Some(Value::Array(locked)) => {
                for (index, id) in locked.iter().enumerate() {
                    let item = child("/restrictions/lockedComponents", index);
                    if let Some(id) = self.stable_id(id, &item)
                        && context.type_of(id.as_str()).is_none()
                    {
                        self.push(AssignmentErrorCode::UnknownReference, item);
                    }
                }
            }
            Some(_) => self.push(
                AssignmentErrorCode::InvalidField,
                "/restrictions/lockedComponents",
            ),
        }
    }

    /// A criterion or fault ID is unique among them and differs from every ID of the schematics.
    fn authored_id(
        &mut self,
        value: Option<&Value>,
        path: &str,
        shared_ids: &HashSet<String>,
        authored_ids: &mut HashSet<String>,
    ) {
        let Some(value) = value else {
            self.push(AssignmentErrorCode::InvalidField, path);
            return;
        };
        if let Some(id) = self.stable_id(value, path)
            && (shared_ids.contains(id.as_str()) || !authored_ids.insert(id.as_str().to_owned()))
        {
            self.push(AssignmentErrorCode::DuplicateId, path);
        }
    }

    fn checks(
        &mut self,
        value: Option<&Value>,
        context: &Context,
        shared_ids: &HashSet<String>,
        authored_ids: &mut HashSet<String>,
    ) {
        let Some(Value::Array(checks)) = value else {
            let code = if value.is_none() {
                AssignmentErrorCode::TaskMismatch
            } else {
                AssignmentErrorCode::InvalidField
            };
            self.push(code, "/checks");
            return;
        };
        if checks.len() > MAX_CHECK_CRITERIA {
            self.push(AssignmentErrorCode::TooLarge, "/checks");
            return;
        }
        for (index, criterion) in checks.iter().enumerate() {
            self.criterion(
                criterion,
                &child("/checks", index),
                context,
                shared_ids,
                authored_ids,
            );
        }
    }

    fn criterion(
        &mut self,
        value: &Value,
        base: &str,
        context: &Context,
        shared_ids: &HashSet<String>,
        authored_ids: &mut HashSet<String>,
    ) {
        let Some(criterion) = value.as_object() else {
            self.push(AssignmentErrorCode::InvalidField, base);
            return;
        };
        let kind = criterion.get("kind").and_then(Value::as_str);
        let specific: &[&str] = match kind {
            Some("value") => &[
                "signal",
                "atSeconds",
                "expected",
                "toleranceAbs",
                "toleranceRel",
            ],
            Some("range") => &["signal", "fromSeconds", "toSeconds", "min", "max"],
            Some("deviceState") => &["componentId", "state", "atSeconds"],
            Some("componentCount") => &["typeId", "min", "max"],
            _ => {
                self.push(AssignmentErrorCode::InvalidField, child(base, "kind"));
                return;
            }
        };
        let known = ["kind", "id", "label"]
            .iter()
            .chain(specific)
            .copied()
            .collect::<Vec<_>>();
        self.unknown_fields(criterion, &known, base);
        self.authored_id(
            criterion.get("id"),
            &child(base, "id"),
            shared_ids,
            authored_ids,
        );
        self.text(
            criterion.get("label"),
            &child(base, "label"),
            TextLimits {
                min: 0,
                max: MAX_LABEL_CHARS,
                required: false,
                forbid_control: false,
            },
        );
        let at = |field: &str| child(base, field);
        match kind {
            Some("value") => {
                self.signal(criterion.get("signal"), &at("signal"), context);
                self.time(criterion.get("atSeconds"), &at("atSeconds"));
                self.finite(criterion.get("expected"), &at("expected"), true);
                self.non_negative(criterion.get("toleranceAbs"), &at("toleranceAbs"), true);
                self.non_negative(criterion.get("toleranceRel"), &at("toleranceRel"), false);
            }
            Some("range") => {
                self.signal(criterion.get("signal"), &at("signal"), context);
                let from = self.time(criterion.get("fromSeconds"), &at("fromSeconds"));
                let to = self.time(criterion.get("toSeconds"), &at("toSeconds"));
                if let (Some(from), Some(to)) = (from, to)
                    && from > to
                {
                    self.push(AssignmentErrorCode::InvalidField, at("toSeconds"));
                }
                let min = self.finite(criterion.get("min"), &at("min"), true);
                let max = self.finite(criterion.get("max"), &at("max"), true);
                if let (Some(min), Some(max)) = (min, max)
                    && min > max
                {
                    self.push(AssignmentErrorCode::InvalidField, at("max"));
                }
            }
            Some("deviceState") => {
                self.device_state(criterion, base, context);
                self.time(criterion.get("atSeconds"), &at("atSeconds"));
            }
            _ => {
                match criterion.get("typeId").and_then(Value::as_str) {
                    None => self.push(AssignmentErrorCode::InvalidField, at("typeId")),
                    Some(text) if parse_enum::<ComponentTypeId>(text).is_none() => {
                        self.push(AssignmentErrorCode::UnknownReference, at("typeId"));
                    }
                    Some(_) => {}
                }
                let min = self.count(
                    criterion.get("min"),
                    &at("min"),
                    0,
                    u64::from(MAX_COMPONENT_COUNT),
                );
                if let Some(max) = criterion.get("max") {
                    let lowest = min.unwrap_or(0);
                    self.count(
                        Some(max),
                        &at("max"),
                        lowest,
                        u64::from(MAX_COMPONENT_COUNT),
                    );
                }
            }
        }
    }

    fn non_negative(&mut self, value: Option<&Value>, path: &str, required: bool) {
        if let Some(number) = self.finite(value, path, required)
            && number < 0.0
        {
            self.push(AssignmentErrorCode::InvalidField, path);
        }
    }

    /// A time (section 4.2): a multiple of the public step, at most 60 s. Returns its step.
    fn time(&mut self, value: Option<&Value>, path: &str) -> Option<u64> {
        let step = value.and_then(Value::as_f64).and_then(step_of);
        if step.is_none() {
            self.push(AssignmentErrorCode::InvalidField, path);
        }
        step
    }

    /// A signal (section 4.1): a layout key of a component in the starting circuit and the
    /// catalog unit of that slot.
    fn signal(&mut self, value: Option<&Value>, path: &str, context: &Context) {
        let Some(signal) = value.and_then(Value::as_object) else {
            self.push(AssignmentErrorCode::InvalidField, path);
            return;
        };
        self.unknown_fields(signal, &["key", "unit"], path);
        let unit = signal
            .get("unit")
            .and_then(Value::as_str)
            .and_then(parse_enum::<Unit>);
        if unit.is_none() {
            self.push(AssignmentErrorCode::InvalidField, child(path, "unit"));
        }
        let Some(key) = signal.get("key").and_then(Value::as_str) else {
            self.push(AssignmentErrorCode::InvalidField, child(path, "key"));
            return;
        };
        let key_path = child(path, "key");
        let split = key.find([':', '/']).map(|at| key.split_at(at));
        let definition = split.and_then(|(component_id, rest)| {
            let type_id = context.type_of(component_id)?;
            slot_for_key_rest(type_id, rest)
        });
        let Some(definition) = definition else {
            self.push(AssignmentErrorCode::UnknownReference, key_path);
            return;
        };
        if unit.is_some_and(|unit| unit != definition.unit) {
            self.push(AssignmentErrorCode::InvalidField, child(path, "unit"));
        }
    }

    fn device_state(&mut self, criterion: &Map<String, Value>, base: &str, context: &Context) {
        let component_path = child(base, "componentId");
        let type_id = match criterion.get("componentId") {
            None => {
                self.push(AssignmentErrorCode::InvalidField, component_path.clone());
                None
            }
            Some(id) => self.stable_id(id, &component_path).and_then(|id| {
                match context.type_of(id.as_str()) {
                    Some(type_id) => Some(type_id),
                    None => {
                        self.push(
                            AssignmentErrorCode::UnknownReference,
                            component_path.clone(),
                        );
                        None
                    }
                }
            }),
        };
        let allowed: &[&str] = match type_id {
            Some(ComponentTypeId::RelaySpdt) => &["released", "picked"],
            Some(ComponentTypeId::Fuse) => &["intact", "tripped"],
            Some(_) => {
                // The component cannot have a state, so its state is not judged.
                self.push(AssignmentErrorCode::InvalidField, component_path);
                return;
            }
            None => &["released", "picked", "intact", "tripped"],
        };
        let valid = criterion
            .get("state")
            .and_then(Value::as_str)
            .is_some_and(|state| allowed.contains(&state));
        if !valid {
            self.push(AssignmentErrorCode::InvalidField, child(base, "state"));
        }
    }

    fn faults(
        &mut self,
        value: Option<&Value>,
        context: &Context,
        shared_ids: &HashSet<String>,
        authored_ids: &mut HashSet<String>,
    ) {
        let Some(value) = value else {
            self.push(AssignmentErrorCode::TaskMismatch, "/faults");
            return;
        };
        let Some(faults) = value.as_array().filter(|faults| !faults.is_empty()) else {
            self.push(AssignmentErrorCode::InvalidField, "/faults");
            return;
        };
        if faults.len() > MAX_ASSIGNMENT_FAULTS {
            self.push(AssignmentErrorCode::TooLarge, "/faults");
            return;
        }
        for (index, fault) in faults.iter().enumerate() {
            self.fault(
                fault,
                &child("/faults", index),
                context,
                shared_ids,
                authored_ids,
            );
        }
    }

    fn fault(
        &mut self,
        value: &Value,
        base: &str,
        context: &Context,
        shared_ids: &HashSet<String>,
        authored_ids: &mut HashSet<String>,
    ) {
        let Some(fault) = value.as_object() else {
            self.push(AssignmentErrorCode::InvalidField, base);
            return;
        };
        self.unknown_fields(
            fault,
            &["id", "kind", "componentId", "parameterId", "value"],
            base,
        );
        self.authored_id(
            fault.get("id"),
            &child(base, "id"),
            shared_ids,
            authored_ids,
        );
        let component_path = child(base, "componentId");
        match fault.get("componentId") {
            None => self.push(AssignmentErrorCode::InvalidField, component_path),
            Some(id) => {
                if let Some(id) = self.stable_id(id, &component_path)
                    && context.type_of(id.as_str()).is_none()
                {
                    self.push(AssignmentErrorCode::UnknownReference, component_path);
                }
            }
        }
        let kind = fault.get("kind").and_then(Value::as_str);
        if !matches!(kind, Some("open" | "short" | "parameter")) {
            self.push(AssignmentErrorCode::InvalidField, child(base, "kind"));
        }
        let parameter = kind == Some("parameter");
        let parameter_path = child(base, "parameterId");
        match (parameter, fault.get("parameterId")) {
            (true, Some(id)) => {
                if id.as_str().and_then(parse_enum::<ParameterId>).is_none() {
                    self.push(AssignmentErrorCode::InvalidField, parameter_path);
                }
            }
            (true, None) | (false, Some(_)) => {
                self.push(AssignmentErrorCode::InvalidField, parameter_path);
            }
            (false, None) => {}
        }
        let value_path = child(base, "value");
        match (parameter, fault.get("value")) {
            (true, value) => {
                self.finite(value, &value_path, true);
            }
            (false, Some(_)) => self.push(AssignmentErrorCode::InvalidField, value_path),
            (false, None) => {}
        }
    }
}

/// The checks that need the typed assignment: each fault is valid on the starting circuit
/// (section 5.3) and every faulted component is locked (section 5.7).
fn check_faults_against_the_circuit(assignment: &Assignment) -> Result<(), Vec<AssignmentError>> {
    let (AssignmentTask::FindFault { .. }, Some(faults), Some(starting)) = (
        &assignment.task,
        &assignment.faults,
        &assignment.starting_circuit,
    ) else {
        return Ok(());
    };
    let mut report = Report::default();
    if let Err(diagnostics) = FaultOverlay::new(starting, faults) {
        for diagnostic in diagnostics {
            let at = diagnostic.references.iter().find_map(|reference| {
                let DiagnosticReference::Component { component_id } = reference else {
                    return None;
                };
                faults
                    .iter()
                    .position(|fault| &fault.component_id == component_id)
            });
            let path = at.map_or_else(|| "/faults".to_owned(), |index| child("/faults", index));
            report.push(AssignmentErrorCode::InvalidField, path);
        }
    }
    let locked = assignment
        .restrictions
        .as_ref()
        .map(|restrictions| restrictions.locked_components.as_slice())
        .unwrap_or_default();
    for (index, fault) in faults.iter().enumerate() {
        if !locked.contains(&fault.component_id) {
            report.push(
                AssignmentErrorCode::TaskMismatch,
                child(&child("/faults", index), "componentId"),
            );
        }
    }
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(report.errors)
    }
}

// The comparison of a diagnosis with the faults (section 5.7).

/// One entry of a student's diagnosis: a component and what is wrong with it. It carries no value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "DiagnosisEntry.ts")]
pub struct DiagnosisEntry {
    pub component_id: StableId,
    pub kind: FaultKind,
    // Required for a `parameter` entry and absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parameter_id: Option<ParameterId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "DiagnosisEntryVerdict.ts")]
pub enum DiagnosisEntryVerdict {
    /// A fault with the same component, kind, and, for `parameter`, parameter.
    Correct,
    /// A fault on that component of another kind, or on another parameter.
    WrongKind,
    /// No fault on that component.
    Incorrect,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "DiagnosisFaultVerdict.ts")]
pub enum DiagnosisFaultVerdict {
    Found,
    Missed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "DiagnosisOutcome.ts")]
pub enum DiagnosisOutcome {
    Passed,
    Partial,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "DiagnosisEntryResult.ts")]
pub struct DiagnosisEntryResult {
    pub entry: DiagnosisEntry,
    pub verdict: DiagnosisEntryVerdict,
    pub message_key: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "DiagnosisFaultResult.ts")]
pub struct DiagnosisFaultResult {
    pub fault_id: StableId,
    pub verdict: DiagnosisFaultVerdict,
    pub message_key: String,
}

/// The full detail of a comparison. What the application shows of it is the task's `reveal`
/// policy (a presentation policy, not a protection).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "DiagnosisResult.ts")]
pub struct DiagnosisResult {
    pub outcome: DiagnosisOutcome,
    pub entries: Vec<DiagnosisEntryResult>,
    pub faults: Vec<DiagnosisFaultResult>,
    pub outcome_key: String,
}

/// The result of checking a diagnosis: the comparison, or the errors of the diagnosis.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export, export_to = "DiagnosisChecked.ts")]
pub enum DiagnosisChecked {
    Checked { result: DiagnosisResult },
    Invalid { errors: Vec<AssignmentError> },
}

impl DiagnosisChecked {
    pub fn from_result(result: Result<DiagnosisResult, Vec<AssignmentError>>) -> Self {
        match result {
            Ok(result) => Self::Checked { result },
            Err(errors) => Self::Invalid { errors },
        }
    }
}

fn entry_verdict_key(verdict: DiagnosisEntryVerdict) -> &'static str {
    match verdict {
        DiagnosisEntryVerdict::Correct => "diagnosis.entry.correct",
        DiagnosisEntryVerdict::WrongKind => "diagnosis.entry.wrongKind",
        DiagnosisEntryVerdict::Incorrect => "diagnosis.entry.incorrect",
    }
}

/// Validates a diagnosis against the starting circuit (file-style errors with a pointer into
/// the diagnosis) and compares it with the faults. No circuit is solved.
pub fn check_diagnosis(
    assignment: &Assignment,
    diagnosis: &Value,
) -> Result<DiagnosisResult, Vec<AssignmentError>> {
    // The assignment is judged by the same rules as a file, so a document that was changed
    // after it was opened (an unlocked fault, a wrong version) is refused, not compared.
    validate_assignment(assignment)?;
    let (AssignmentTask::FindFault { .. }, Some(faults)) = (&assignment.task, &assignment.faults)
    else {
        return Err(vec![AssignmentError::new(
            AssignmentErrorCode::TaskMismatch,
            "/task/kind",
        )]);
    };
    let entries = decode_diagnosis(assignment, diagnosis)?;

    let entry_results = entries
        .into_iter()
        .map(|entry| {
            let on_component = faults
                .iter()
                .filter(|fault| fault.component_id == entry.component_id)
                .collect::<Vec<_>>();
            let verdict = if on_component.is_empty() {
                DiagnosisEntryVerdict::Incorrect
            } else if on_component
                .iter()
                .any(|fault| matches_entry(fault, &entry))
            {
                DiagnosisEntryVerdict::Correct
            } else {
                DiagnosisEntryVerdict::WrongKind
            };
            DiagnosisEntryResult {
                entry,
                verdict,
                message_key: entry_verdict_key(verdict).to_owned(),
            }
        })
        .collect::<Vec<_>>();
    let fault_results = faults
        .iter()
        .map(|fault| {
            let found = entry_results.iter().any(|result| {
                result.verdict == DiagnosisEntryVerdict::Correct
                    && matches_entry(fault, &result.entry)
            });
            let verdict = if found {
                DiagnosisFaultVerdict::Found
            } else {
                DiagnosisFaultVerdict::Missed
            };
            DiagnosisFaultResult {
                fault_id: fault.id.clone(),
                verdict,
                message_key: match verdict {
                    DiagnosisFaultVerdict::Found => "diagnosis.fault.found",
                    DiagnosisFaultVerdict::Missed => "diagnosis.fault.missed",
                }
                .to_owned(),
            }
        })
        .collect::<Vec<_>>();

    let every_fault_found = fault_results
        .iter()
        .all(|result| result.verdict == DiagnosisFaultVerdict::Found);
    let any_wrong = entry_results.iter().any(|result| {
        matches!(
            result.verdict,
            DiagnosisEntryVerdict::WrongKind | DiagnosisEntryVerdict::Incorrect
        )
    });
    let any_useful = entry_results.iter().any(|result| {
        matches!(
            result.verdict,
            DiagnosisEntryVerdict::Correct | DiagnosisEntryVerdict::WrongKind
        )
    });
    let outcome = if every_fault_found && !any_wrong {
        DiagnosisOutcome::Passed
    } else if !any_useful {
        DiagnosisOutcome::Failed
    } else {
        DiagnosisOutcome::Partial
    };
    let outcome_key = match outcome {
        DiagnosisOutcome::Passed => "diagnosis.outcome.passed",
        DiagnosisOutcome::Partial => "diagnosis.outcome.partial",
        DiagnosisOutcome::Failed => "diagnosis.outcome.failed",
    };
    Ok(DiagnosisResult {
        outcome,
        entries: entry_results,
        faults: fault_results,
        outcome_key: outcome_key.to_owned(),
    })
}

fn matches_entry(fault: &FaultSpec, entry: &DiagnosisEntry) -> bool {
    fault.component_id == entry.component_id
        && fault.kind == entry.kind
        && (fault.kind != FaultKind::Parameter || fault.parameter_id == entry.parameter_id)
}

fn decode_diagnosis(
    assignment: &Assignment,
    diagnosis: &Value,
) -> Result<Vec<DiagnosisEntry>, Vec<AssignmentError>> {
    let Some(list) = diagnosis.as_array() else {
        return Err(vec![AssignmentError::new(
            AssignmentErrorCode::InvalidField,
            "",
        )]);
    };
    if list.len() > MAX_ASSIGNMENT_FAULTS {
        return Err(vec![AssignmentError::new(
            AssignmentErrorCode::TooLarge,
            "",
        )]);
    }
    let types = assignment
        .starting_circuit
        .iter()
        .flat_map(|schematic| &schematic.components)
        .map(|component| (component.id().clone(), component.type_id()))
        .collect::<HashMap<_, _>>();
    let catalog = component_catalog();

    let mut report = Report::default();
    let mut entries = Vec::with_capacity(list.len());
    for (index, item) in list.iter().enumerate() {
        let base = child("", index);
        let Some(object) = item.as_object() else {
            report.push(AssignmentErrorCode::InvalidField, base);
            continue;
        };
        report.unknown_fields(object, &["componentId", "kind", "parameterId"], &base);
        let component_path = child(&base, "componentId");
        let type_id = match object.get("componentId") {
            None => {
                report.push(AssignmentErrorCode::InvalidField, component_path);
                None
            }
            Some(id) => report.stable_id(id, &component_path).and_then(|id| {
                let type_id = types.get(&id).copied();
                if type_id.is_none() {
                    report.push(AssignmentErrorCode::UnknownReference, component_path);
                }
                type_id
            }),
        };
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .and_then(parse_enum::<FaultKind>);
        if kind.is_none() {
            report.push(AssignmentErrorCode::InvalidField, child(&base, "kind"));
        }
        let parameter_path = child(&base, "parameterId");
        match (kind, object.get("parameterId")) {
            (Some(FaultKind::Parameter), Some(id)) => {
                let parameter = id.as_str().and_then(parse_enum::<ParameterId>);
                let belongs = parameter.is_some_and(|parameter| {
                    type_id.is_none_or(|type_id| {
                        catalog
                            .component_types
                            .iter()
                            .find(|entry| entry.type_id == type_id)
                            .is_some_and(|entry| {
                                entry
                                    .parameters
                                    .iter()
                                    .any(|definition| definition.id == parameter)
                            })
                    })
                });
                if !belongs {
                    report.push(AssignmentErrorCode::InvalidField, parameter_path);
                }
            }
            (Some(FaultKind::Parameter), None)
            | (Some(FaultKind::Open | FaultKind::Short), Some(_)) => {
                report.push(AssignmentErrorCode::InvalidField, parameter_path);
            }
            _ => {}
        }
        if report.errors.is_empty()
            && let Ok(entry) = serde_json::from_value::<DiagnosisEntry>(item.clone())
        {
            entries.push(entry);
        }
    }
    if report.errors.is_empty() {
        Ok(entries)
    } else {
        Err(report.errors)
    }
}
