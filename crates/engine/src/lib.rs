//! Transport-independent Stage 1 types and catalog for the circuit engine.

pub mod catalog;
pub mod diagnostics;
pub mod ids;
pub mod netlist;
pub mod results;
pub mod schematic;

pub use catalog::{
    ComponentCatalog, ComponentCatalogEntry, ComponentTypeId, NumericDomain, ParameterDefinition,
    ParameterId, PinDefinition, PinId, PinRole, Unit, component_catalog,
};
pub use diagnostics::{Diagnostic, DiagnosticCode, DiagnosticReference, DiagnosticSeverity};
pub use ids::{InvalidStableId, StableId};
pub use results::{
    AdvanceRequest, CompileRequest, CompileResult, FrameMetadata, ResultLayout, ResultQuantity,
    ResultSignConvention, ResultSlot, ResultSnapshot, SIMULATION_TIMESTEP_SECONDS,
    SetParameterRequest, SetParameterResult,
};
pub use schematic::{
    ComponentGeometry, ComponentSpec, DcVoltageSourceParameters, GeometryEnvelope, GridPoint,
    JunctionSpec, ReferenceMarkerSpec, ResistorParameters, ResolvedPinPosition, SCHEMA_VERSION,
    Schematic, WireSpec,
};

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
