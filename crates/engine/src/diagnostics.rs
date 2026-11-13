use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{PinId, StableId};

/// Stable machine-readable diagnostic codes. The UI resolves localization keys to prose.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "PascalCase")]
#[ts(export, export_to = "DiagnosticCode.ts")]
pub enum DiagnosticCode {
    InvalidSchemaVersion,
    InvalidGeometry,
    InvalidRailName,
    DuplicateRailName,
    RailLimitExceeded,
    UnsupportedAssignmentRails,
    InvalidParameter,
    DuplicateId,
    UnknownComponentType,
    UnknownPin,
    DetachedReferenceMarker,
    NoGroundReference,
    FloatingNode,
    ShortCircuit,
    InconsistentIdealConstraint,
    InconsistentState,
    SingularSystem,
    StepAccuracyLimited,
    EventChatter,
    NonFiniteState,
    NonConvergence,
    ModelRangeExceeded,
    InvalidFault,
    StaleRun,
    StaleDocumentRevision,
}

impl DiagnosticCode {
    pub const fn localization_key(self) -> &'static str {
        match self {
            Self::InvalidSchemaVersion => "diagnostic.invalidSchemaVersion",
            Self::InvalidGeometry => "diagnostic.invalidGeometry",
            Self::InvalidRailName => "diagnostic.invalidRailName",
            Self::DuplicateRailName => "diagnostic.duplicateRailName",
            Self::RailLimitExceeded => "diagnostic.railLimitExceeded",
            Self::UnsupportedAssignmentRails => "diagnostic.unsupportedAssignmentRails",
            Self::InvalidParameter => "diagnostic.invalidParameter",
            Self::DuplicateId => "diagnostic.duplicateId",
            Self::UnknownComponentType => "diagnostic.unknownComponentType",
            Self::UnknownPin => "diagnostic.unknownPin",
            Self::DetachedReferenceMarker => "diagnostic.detachedReferenceMarker",
            Self::NoGroundReference => "diagnostic.noGroundReference",
            Self::FloatingNode => "diagnostic.floatingNode",
            Self::ShortCircuit => "diagnostic.shortCircuit",
            Self::InconsistentIdealConstraint => "diagnostic.inconsistentIdealConstraint",
            Self::InconsistentState => "diagnostic.inconsistentState",
            Self::SingularSystem => "diagnostic.singularSystem",
            Self::StepAccuracyLimited => "diagnostic.stepAccuracyLimited",
            Self::EventChatter => "diagnostic.eventChatter",
            Self::NonFiniteState => "diagnostic.nonFiniteState",
            Self::NonConvergence => "diagnostic.nonConvergence",
            Self::ModelRangeExceeded => "diagnostic.modelRangeExceeded",
            Self::InvalidFault => "diagnostic.invalidFault",
            Self::StaleRun => "diagnostic.staleRun",
            Self::StaleDocumentRevision => "diagnostic.staleDocumentRevision",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "DiagnosticSeverity.ts")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// Typed references let the UI highlight the affected item without parsing message text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "DiagnosticReference.ts")]
pub enum DiagnosticReference {
    Component {
        component_id: StableId,
    },
    Pin {
        component_id: StableId,
        pin_id: String,
    },
    Wire {
        wire_id: StableId,
    },
    Junction {
        junction_id: StableId,
    },
    ReferenceMarker {
        reference_marker_id: StableId,
    },
    Node {
        node_id: StableId,
    },
    Rail {
        rail_id: StableId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "Diagnostic.ts")]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: DiagnosticSeverity,
    pub localization_key: String,
    pub references: Vec<DiagnosticReference>,
}

impl Diagnostic {
    pub fn error(code: DiagnosticCode, references: Vec<DiagnosticReference>) -> Self {
        Self {
            code,
            severity: DiagnosticSeverity::Error,
            localization_key: code.localization_key().to_owned(),
            references,
        }
    }

    pub fn warning(code: DiagnosticCode, references: Vec<DiagnosticReference>) -> Self {
        Self {
            code,
            severity: DiagnosticSeverity::Warning,
            localization_key: code.localization_key().to_owned(),
            references,
        }
    }
}

impl DiagnosticReference {
    pub(crate) fn sort_key(&self) -> (&str, u8, &str) {
        match self {
            Self::Component { component_id } => (component_id.as_str(), 0, ""),
            Self::Pin {
                component_id,
                pin_id,
            } => (component_id.as_str(), 1, pin_id),
            Self::Wire { wire_id } => (wire_id.as_str(), 2, ""),
            Self::Junction { junction_id } => (junction_id.as_str(), 3, ""),
            Self::ReferenceMarker {
                reference_marker_id,
            } => (reference_marker_id.as_str(), 4, ""),
            Self::Node { node_id } => (node_id.as_str(), 5, ""),
            Self::Rail { rail_id } => (rail_id.as_str(), 6, ""),
        }
    }
    pub fn pin(component_id: StableId, pin_id: PinId) -> Self {
        Self::Pin {
            component_id,
            pin_id: pin_id.as_str().to_owned(),
        }
    }
}
