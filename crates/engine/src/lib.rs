//! Transport-independent Stage 1 types and catalog for the circuit engine.

pub mod catalog;
mod devices;
pub mod diagnostics;
pub mod engine;
pub mod faults;
pub mod ids;
mod mna;
pub mod netlist;
pub mod project;
pub mod results;
pub mod schematic;
pub mod solver;

pub use catalog::{
    ComponentCatalog, ComponentCatalogEntry, ComponentTypeId, NumericDomain, ParameterDefinition,
    ParameterId, PinDefinition, PinId, PinRole, Unit, component_catalog,
};
pub use diagnostics::{Diagnostic, DiagnosticCode, DiagnosticReference, DiagnosticSeverity};
pub use engine::Engine;
pub use faults::{FaultKind, FaultSpec, SHORT_FAULT_RESISTANCE_OHMS, supported_fault_kinds};
pub use ids::{InvalidStableId, StableId};
pub use project::{ProjectDocumentError, decode_project_json, validate_project_schematic};
pub use results::{
    AdvanceRequest, CompileRequest, CompileResult, DeviceState, FrameMetadata, FuseState,
    RelayContactState, ResultLayout, ResultQuantity, ResultSignConvention, ResultSlot,
    ResultSnapshot, SIMULATION_TIMESTEP_SECONDS, SetParameterRequest, SetParameterResult,
    TraceBatch, TraceBatchMetadata, TracePoint, TraceQuery, TraceRequest, TraceSignal,
};
pub use schematic::{
    AmmeterParameters, ComponentGeometry, ComponentSpec, DcVoltageSourceParameters, FuseParameters,
    GeometryEnvelope, GridPoint, InductorParameters, JunctionSpec, PmdcMotorParameters,
    ReferenceMarkerSpec, RelayParameters, ResistorParameters, ResolvedPinPosition, SCHEMA_VERSION,
    Schematic, VoltmeterParameters, WireSpec,
};

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
