//! Transport-independent Stage 1 types and catalog for the circuit engine.

pub mod assignment;
mod bill_of_materials;
pub mod catalog;
pub mod checks;
mod devices;
pub mod diagnostics;
mod documentation;
pub mod engine;
pub mod faults;
mod grading;
pub mod ids;
mod mna;
pub mod netlist;
mod newton;
mod operator;
mod pin_groups;
pub mod probes;
pub mod project;
pub mod rails;
pub mod results;
pub mod schematic;
mod slots;
pub mod solver;
mod time_relay;

pub use assignment::{
    ASSIGNMENT_SCHEMA_VERSION, Assignment, AssignmentDecoded, AssignmentError, AssignmentErrorCode,
    AssignmentSaveOutcome, AssignmentTask, DiagnosisChecked, DiagnosisEntry, DiagnosisEntryResult,
    DiagnosisEntryVerdict, DiagnosisFaultResult, DiagnosisFaultVerdict, DiagnosisOutcome,
    DiagnosisResult, MAX_ASSIGNMENT_BYTES, MAX_ASSIGNMENT_ERRORS, MAX_ASSIGNMENT_FAULTS,
    Restrictions, RevealPolicy, check_diagnosis, decode_assignment_json, encode_assignment_json,
    validate_assignment, validate_assignment_draft_value, validate_assignment_value,
};
pub use bill_of_materials::{
    BillOfMaterials, MaterialParameter, MaterialRow, TerminalStripRow, bill_of_materials,
};
pub use catalog::{
    CategoryId, ComponentCatalog, ComponentCatalogEntry, ComponentTypeId, NumericDomain,
    ParameterDefinition, ParameterId, PinDefinition, PinGroupDefinition, PinId, PinRole, Unit,
    component_catalog,
};
pub use checks::{
    CheckCounts, CheckCriterion, CheckOutcome, CheckRequest, CheckResult, CheckSignal,
    CheckVerdict, CriterionResult, DeviceStateName, LockedResult, MAX_CHECK_CRITERIA,
    MAX_CHECK_SECONDS, MAX_COMPONENT_COUNT, evaluate_checks,
};
pub use diagnostics::{Diagnostic, DiagnosticCode, DiagnosticReference, DiagnosticSeverity};
pub use documentation::{
    DESIGNATOR_FIRST_LETTERS, DesignatorEntry, MAX_DESIGNATORS, MAX_NOTES, MAX_SHEETS,
    MAX_TERMINALS, MAX_WIRE_NUMBERS, NoteSpec, ProjectDocumentation, SheetComponentBounds,
    SheetSize, SheetSpec, TerminalSpec, TitleBlock, WireNumberEntry, documentation_diagnostics,
    is_valid_designator, number_wires, sheet_diagnostics,
};
pub use engine::Engine;
pub use faults::{FaultKind, FaultSpec, SHORT_FAULT_RESISTANCE_OHMS, supported_fault_kinds};
pub use grading::{
    AssignmentGrading, CriterionGrading, CriterionScore, GradedCheckRequest, GradedCheckResult,
    GradingScore, ScoreHint, evaluate_graded_checks,
};
pub use ids::{InvalidStableId, StableId};
pub use probes::*;
pub use project::{ProjectDocumentError, decode_project_json, validate_project_schematic};
pub use rails::{MAX_RAIL_CONTACT_WORK, MAX_RAIL_COUNT, RailSpec};
pub use results::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, EmergencyStopPosition,
    FrameMetadata, FuseState, KeySwitchPosition, OperationalAmplifierState, OperatorPosition,
    PushButtonPosition, RelayContactState, ResultLayout, ResultQuantity, ResultSignConvention,
    ResultSlot, ResultSnapshot, SIMULATION_TIMESTEP_SECONDS, SelectorPosition,
    SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest, SetParameterResult,
    StreamNotice, SwitchPosition, TraceBatch, TraceBatchMetadata, TracePoint, TraceQuery,
    TraceRequest, TraceSignal,
};
pub use schematic::{
    AcAmmeterParameters, AcVoltageSourceParameters, AcVoltmeterParameters, AmmeterParameters,
    BjtParameters, ComponentGeometry, ComponentSpec, DcVoltageSourceParameters, DiodeParameters,
    EarthElectrodeParameters, ExposedPartParameters, FuseParameters, GeometryEnvelope, GridPoint,
    InductorParameters, JunctionSpec, PmdcMotorParameters, PushButtonParameters, RcdParameters,
    ReferenceMarkerSpec, RelayParameters, ResistorParameters, ResolvedPinPosition, SCHEMA_VERSION,
    Schematic, SwitchParameters, ThreePhaseSourceDeltaParameters, ThreePhaseSourceStarParameters,
    VoltmeterParameters, WattmeterParameters, WireSpec,
};

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
