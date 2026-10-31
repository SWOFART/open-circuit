//! Transport-independent Stage 1 types and catalog for the circuit engine.

pub mod assignment;
pub mod catalog;
pub mod checks;
mod devices;
pub mod diagnostics;
pub mod engine;
pub mod faults;
pub mod ids;
mod mna;
pub mod netlist;
mod operator;
pub mod project;
pub mod results;
pub mod schematic;
mod slots;
pub mod solver;

pub use assignment::{
    ASSIGNMENT_SCHEMA_VERSION, Assignment, AssignmentDecoded, AssignmentError, AssignmentErrorCode,
    AssignmentSaveOutcome, AssignmentTask, DiagnosisChecked, DiagnosisEntry, DiagnosisEntryResult,
    DiagnosisEntryVerdict, DiagnosisFaultResult, DiagnosisFaultVerdict, DiagnosisOutcome,
    DiagnosisResult, MAX_ASSIGNMENT_BYTES, MAX_ASSIGNMENT_ERRORS, MAX_ASSIGNMENT_FAULTS,
    Restrictions, RevealPolicy, check_diagnosis, decode_assignment_json, encode_assignment_json,
    validate_assignment, validate_assignment_value,
};
pub use catalog::{
    ComponentCatalog, ComponentCatalogEntry, ComponentTypeId, NumericDomain, ParameterDefinition,
    ParameterId, PinDefinition, PinId, PinRole, Unit, component_catalog,
};
pub use checks::{
    CheckCounts, CheckCriterion, CheckOutcome, CheckRequest, CheckResult, CheckSignal,
    CheckVerdict, CriterionResult, DeviceStateName, LockedResult, MAX_CHECK_CRITERIA,
    MAX_CHECK_SECONDS, MAX_COMPONENT_COUNT, evaluate_checks,
};
pub use diagnostics::{Diagnostic, DiagnosticCode, DiagnosticReference, DiagnosticSeverity};
pub use engine::Engine;
pub use faults::{FaultKind, FaultSpec, SHORT_FAULT_RESISTANCE_OHMS, supported_fault_kinds};
pub use ids::{InvalidStableId, StableId};
pub use project::{ProjectDocumentError, decode_project_json, validate_project_schematic};
pub use results::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, FrameMetadata, FuseState,
    OperatorPosition, PushButtonPosition, RelayContactState, ResultLayout, ResultQuantity,
    ResultSignConvention, ResultSlot, ResultSnapshot, SIMULATION_TIMESTEP_SECONDS,
    SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest, SetParameterResult,
    SwitchPosition, TraceBatch, TraceBatchMetadata, TracePoint, TraceQuery, TraceRequest,
    TraceSignal,
};
pub use schematic::{
    AmmeterParameters, ComponentGeometry, ComponentSpec, DcVoltageSourceParameters, FuseParameters,
    GeometryEnvelope, GridPoint, InductorParameters, JunctionSpec, PmdcMotorParameters,
    PushButtonParameters, ReferenceMarkerSpec, RelayParameters, ResistorParameters,
    ResolvedPinPosition, SCHEMA_VERSION, Schematic, SwitchParameters, VoltmeterParameters,
    WireSpec,
};

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
