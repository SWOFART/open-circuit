//! Version-aware decoding and validation for persisted schematic documents.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{
    ComponentSpec, DiagnosticCode, DiagnosticSeverity, JunctionSpec, ReferenceMarkerSpec,
    SCHEMA_VERSION, Schematic, WireSpec,
};

/// Frozen assignment-v1/project-v1 input shape, never exposed as a second frontend DTO.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LegacySchematicV1 {
    schema_version: u32,
    components: Vec<ComponentSpec>,
    wires: Vec<WireSpec>,
    junctions: Vec<JunctionSpec>,
    reference_markers: Vec<ReferenceMarkerSpec>,
}

impl LegacySchematicV1 {
    pub(crate) fn into_current_unvalidated(self) -> Schematic {
        Schematic {
            rails: Vec::new(),
            schema_version: SCHEMA_VERSION,
            components: self.components,
            wires: self.wires,
            junctions: self.junctions,
            reference_markers: self.reference_markers,
        }
    }

    pub(crate) fn normalize(self) -> Result<Schematic, ProjectDocumentError> {
        if self.schema_version != 1 {
            return Err(ProjectDocumentError::Invalid(vec![
                DiagnosticCode::InvalidSchemaVersion,
            ]));
        }
        let schematic = self.into_current_unvalidated();
        validate_project_schematic(&schematic)?;
        Ok(schematic)
    }

    pub(crate) fn from_current(schematic: &Schematic) -> Result<Self, ProjectDocumentError> {
        validate_project_schematic(schematic)?;
        if !schematic.rails.is_empty() {
            return Err(ProjectDocumentError::Invalid(vec![
                DiagnosticCode::UnsupportedAssignmentRails,
            ]));
        }
        Ok(Self {
            schema_version: 1,
            components: schematic.components.clone(),
            wires: schematic.wires.clone(),
            junctions: schematic.junctions.clone(),
            reference_markers: schematic.reference_markers.clone(),
        })
    }
}

/// A schematic file that is malformed or is not a valid supported document.
#[derive(Debug)]
pub enum ProjectDocumentError {
    Malformed(serde_json::Error),
    Invalid(Vec<DiagnosticCode>),
}

impl fmt::Display for ProjectDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(error) => write!(formatter, "Could not parse project JSON: {error}"),
            Self::Invalid(codes) => write!(
                formatter,
                "Project schematic failed validation: {}.",
                codes
                    .iter()
                    .map(|code| format!("{code:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl Error for ProjectDocumentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

/// Decodes a persisted schematic and applies the current version's Engine validation.
///
/// Strict v1 input is migrated without losing fields; v2 is the current input.
/// Probe only the version, then decode the original bytes into the exact shape,
/// retaining Serde's duplicate-field detection instead of round-tripping a Value.
pub fn decode_project_json(bytes: &[u8]) -> Result<Schematic, ProjectDocumentError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Version {
        schema_version: u32,
    }
    let version: Version =
        serde_json::from_slice(bytes).map_err(ProjectDocumentError::Malformed)?;
    if version.schema_version == 1 {
        let legacy: LegacySchematicV1 =
            serde_json::from_slice(bytes).map_err(ProjectDocumentError::Malformed)?;
        return legacy.normalize();
    }
    if version.schema_version != SCHEMA_VERSION {
        return Err(ProjectDocumentError::Invalid(vec![
            DiagnosticCode::InvalidSchemaVersion,
        ]));
    }
    let schematic: Schematic =
        serde_json::from_slice(bytes).map_err(ProjectDocumentError::Malformed)?;
    validate_project_schematic(&schematic)?;
    Ok(schematic)
}

/// Applies the current project format's Engine validation before persistence.
pub fn validate_project_schematic(schematic: &Schematic) -> Result<(), ProjectDocumentError> {
    let errors = schematic
        .validate()
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ProjectDocumentError::Invalid(errors))
    }
}

#[cfg(test)]
mod tests {
    use crate::{DiagnosticCode, Schematic};

    use super::{ProjectDocumentError, decode_project_json};

    #[test]
    fn rejects_malformed_json_and_unknown_fields() {
        assert!(matches!(
            decode_project_json(b"{not json"),
            Err(ProjectDocumentError::Malformed(_))
        ));
        assert!(matches!(
            decode_project_json(
                br#"{"schemaVersion":1,"components":[],"wires":[],"junctions":[],"referenceMarkers":[],"runtimeState":{}}"#
            ),
            Err(ProjectDocumentError::Malformed(_))
        ));
    }

    #[test]
    fn rejects_unsupported_versions_instead_of_treating_them_as_current() {
        let error = decode_project_json(
            br#"{"schemaVersion":3,"components":[],"wires":[],"junctions":[],"referenceMarkers":[]}"#,
        )
        .expect_err("unsupported schema versions must be rejected");

        assert!(matches!(
            error,
            ProjectDocumentError::Invalid(codes)
                if codes == vec![DiagnosticCode::InvalidSchemaVersion]
        ));
    }

    #[test]
    fn decodes_a_valid_version_one_document() {
        let schematic = decode_project_json(
            br#"{"schemaVersion":1,"components":[],"wires":[],"junctions":[],"referenceMarkers":[]}"#,
        )
        .expect("empty version 1 documents are valid");

        assert_eq!(
            schematic,
            Schematic {
                rails: Vec::new(),
                schema_version: crate::SCHEMA_VERSION,
                components: vec![],
                wires: vec![],
                junctions: vec![],
                reference_markers: vec![],
            }
        );
    }
}
