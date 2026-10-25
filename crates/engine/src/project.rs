//! Version-aware decoding and validation for persisted schematic documents.

use std::{error::Error, fmt};

use crate::{DiagnosticCode, DiagnosticSeverity, Schematic};

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
/// This release supports version 1 only. Future migrations belong here so they
/// remain deterministic and independent of file I/O and desktop transports.
pub fn decode_project_json(bytes: &[u8]) -> Result<Schematic, ProjectDocumentError> {
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
            br#"{"schemaVersion":2,"components":[],"wires":[],"junctions":[],"referenceMarkers":[]}"#,
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
                schema_version: 1,
                components: vec![],
                wires: vec![],
                junctions: vec![],
                reference_markers: vec![],
            }
        );
    }
}
