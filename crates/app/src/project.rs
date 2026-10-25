//! Project-file input and persistence for the desktop shell.
//!
//! This module owns filesystem access and the active file path. The Engine owns
//! version-aware decoding and structural validation; the frontend receives only
//! validated document values.

use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use open_circuit_engine::{Schematic, decode_project_json, validate_project_schematic};
use tempfile::NamedTempFile;

const PROJECT_EXTENSION: &str = "ocircuit";
const MAX_PROJECT_BYTES: u64 = 16 * 1024 * 1024;

/// The active project path is application state, never part of the schematic.
#[derive(Clone, Default)]
pub struct ProjectSession {
    path: Arc<Mutex<Option<PathBuf>>>,
}

impl ProjectSession {
    pub fn current_path(&self) -> Result<Option<PathBuf>, String> {
        self.path
            .lock()
            .map(|path| path.clone())
            .map_err(|_| "The project session is unavailable.".to_owned())
    }

    pub fn clear_path(&self) -> Result<(), String> {
        let mut path = self
            .path
            .lock()
            .map_err(|_| "The project session is unavailable.".to_owned())?;
        *path = None;
        Ok(())
    }

    pub fn set_path(&self, next_path: PathBuf) -> Result<(), String> {
        let mut path = self
            .path
            .lock()
            .map_err(|_| "The project session is unavailable.".to_owned())?;
        *path = Some(next_path);
        Ok(())
    }

    /// Replaces the active path only after the selected file has been fully read
    /// and validated. A failed open leaves the current project location intact.
    pub fn load(&self, path: &Path) -> Result<Schematic, String> {
        let schematic = read_project_file(path)?;
        let mut active_path = self
            .path
            .lock()
            .map_err(|_| "The project session is unavailable.".to_owned())?;
        *active_path = Some(path.to_path_buf());
        Ok(schematic)
    }

    /// Updates the active path only after the complete save has succeeded.
    pub fn save_to(&self, path: &Path, schematic: &Schematic) -> Result<PathBuf, String> {
        let saved_path = write_project_file(path, schematic)?;
        let mut active_path = self
            .path
            .lock()
            .map_err(|_| "The project session is unavailable.".to_owned())?;
        *active_path = Some(saved_path.clone());
        Ok(saved_path)
    }
}

/// Reads, deserializes, and validates a version 1 `.ocircuit` document.
///
/// Electrically incomplete schematics remain valid documents; this validates
/// structure and parameter domains without compiling or solving the circuit.
pub fn read_project_file(path: &Path) -> Result<Schematic, String> {
    require_project_extension(path)?;

    let file = File::open(path).map_err(|error| format!("Could not read project file: {error}"))?;
    if file
        .metadata()
        .map_err(|error| format!("Could not inspect project file: {error}"))?
        .len()
        > MAX_PROJECT_BYTES
    {
        return Err("Project file exceeds the 16 MiB size limit.".to_owned());
    }

    // Re-check after reading in case the file grew after its metadata was read.
    let mut bytes = Vec::new();
    file.take(MAX_PROJECT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read project file: {error}"))?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project file exceeds the 16 MiB size limit.".to_owned());
    }

    decode_project_json(&bytes).map_err(|error| error.to_string())
}

/// Writes a readable, deterministic JSON schematic by atomically replacing the
/// destination with a temporary file created in the same directory.
pub fn write_project_file(path: &Path, schematic: &Schematic) -> Result<PathBuf, String> {
    validate_project_schematic(schematic).map_err(|error| error.to_string())?;
    let path = normalize_save_path(path)?;
    let bytes = serde_json::to_vec_pretty(schematic)
        .map_err(|error| format!("Could not serialize project: {error}"))?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project file exceeds the 16 MiB size limit.".to_owned());
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| format!("Could not prepare a project save: {error}"))?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.write_all(b"\n"))
        .map_err(|error| format!("Could not write project file: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Could not flush project file: {error}"))?;
    temporary
        .persist(&path)
        .map_err(|error| format!("Could not complete project save: {}", error.error))?;

    Ok(path)
}

fn require_project_extension(path: &Path) -> Result<(), String> {
    let extension = path.extension().and_then(|extension| extension.to_str());
    if extension.is_some_and(|extension| extension.eq_ignore_ascii_case(PROJECT_EXTENSION)) {
        Ok(())
    } else {
        Err("Open Circuit project files must use the .ocircuit extension.".to_owned())
    }
}

fn normalize_save_path(path: &Path) -> Result<PathBuf, String> {
    let extension = path.extension().and_then(|extension| extension.to_str());
    match extension {
        Some(extension) if extension.eq_ignore_ascii_case(PROJECT_EXTENSION) => {
            Ok(path.to_path_buf())
        }
        Some(_) => Err("Open Circuit project files must use the .ocircuit extension.".to_owned()),
        None => {
            let mut path = path.to_path_buf();
            path.set_extension(PROJECT_EXTENSION);
            Ok(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use open_circuit_engine::Schematic;

    use super::{ProjectSession, read_project_file, write_project_file};

    const DIVIDER: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");

    fn divider() -> Schematic {
        serde_json::from_str(DIVIDER).expect("the canonical schematic should deserialize")
    }

    fn project_path(directory: &tempfile::TempDir, name: &str) -> PathBuf {
        directory.path().join(format!("{name}.ocircuit"))
    }

    #[test]
    fn save_and_open_round_trip_only_the_schematic() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = project_path(&directory, "divider");
        let expected = divider();

        write_project_file(&path, &expected).expect("the schematic should save");

        let serialized = fs::read_to_string(&path).expect("the project should be readable");
        assert!(serialized.ends_with('\n'));
        assert!(!serialized.contains("snapshot"));
        assert!(!serialized.contains("diagnostics"));
        assert!(!serialized.contains("runtimeState"));
        assert_eq!(
            read_project_file(&path).expect("the project should open"),
            expected
        );
    }

    #[test]
    fn save_adds_the_native_extension_when_the_dialog_omits_it() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join("new-project");

        let saved = write_project_file(&path, &divider()).expect("the schematic should save");

        assert_eq!(saved, project_path(&directory, "new-project"));
        assert!(saved.is_file());
    }

    #[test]
    fn rejects_malformed_unknown_and_unsupported_project_data() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = project_path(&directory, "invalid");

        for contents in [
            "{not json",
            r#"{"schemaVersion":2,"components":[],"wires":[],"junctions":[],"referenceMarkers":[]}"#,
            r#"{"schemaVersion":1,"components":[],"wires":[],"junctions":[],"referenceMarkers":[],"runtimeState":{}}"#,
        ] {
            fs::write(&path, contents).expect("invalid fixture should be written");
            assert!(read_project_file(&path).is_err());
        }
    }

    #[test]
    fn rejects_invalid_ids_rotations_and_catalog_parameter_values() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = project_path(&directory, "invalid-structure");
        let original: serde_json::Value =
            serde_json::from_str(DIVIDER).expect("the canonical JSON should parse");

        let mut invalid_id = original.clone();
        invalid_id["components"][0]["id"] = serde_json::json!("not-a-uuid");

        let mut duplicate_id = original.clone();
        duplicate_id["components"][1]["id"] = duplicate_id["components"][0]["id"].clone();

        let mut invalid_rotation = original.clone();
        invalid_rotation["components"][0]["rotationDeg"] = serde_json::json!(45);

        let mut invalid_parameter = original;
        invalid_parameter["components"][1]["parameters"]["resistanceOhms"] = serde_json::json!(0);

        for invalid in [
            invalid_id,
            duplicate_id,
            invalid_rotation,
            invalid_parameter,
        ] {
            fs::write(
                &path,
                serde_json::to_vec(&invalid).expect("the invalid document should serialize"),
            )
            .expect("invalid fixture should be written");
            assert!(read_project_file(&path).is_err());
        }
    }

    #[test]
    fn invalid_open_does_not_replace_the_active_project_path() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let existing = project_path(&directory, "existing");
        let invalid = project_path(&directory, "invalid");
        write_project_file(&existing, &divider()).expect("existing project should be written");
        fs::write(&invalid, "{}").expect("invalid project should be written");

        let session = ProjectSession::default();
        session
            .load(&existing)
            .expect("the existing project should load");
        assert!(session.load(&invalid).is_err());

        assert_eq!(session.current_path().unwrap(), Some(existing));
    }

    #[test]
    fn failed_save_leaves_the_previous_project_file_unchanged() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = project_path(&directory, "existing");
        let original = divider();
        write_project_file(&path, &original).expect("the original project should be written");
        let before = fs::read(&path).expect("the original bytes should be readable");

        let mut invalid = original;
        invalid.schema_version = 2;
        let session = ProjectSession::default();
        session
            .load(&path)
            .expect("the original project should load");
        assert!(session.save_to(&path, &invalid).is_err());

        assert_eq!(
            fs::read(&path).expect("the original bytes should remain"),
            before
        );
        assert_eq!(session.current_path().unwrap(), Some(path));
    }

    #[test]
    fn rejects_non_native_extensions() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let path = directory.path().join("project.json");

        assert!(write_project_file(&path, &divider()).is_err());
        assert!(read_project_file(&path).is_err());
    }
}
