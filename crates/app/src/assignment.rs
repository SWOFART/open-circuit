//! Assignment-file input and persistence for the desktop shell
//! (docs/contracts/stage-5-assignments.md sections 2.5 and 6).
//!
//! This module owns filesystem access and the active file path, on the rules of
//! [`crate::project`]. The Engine owns the assignment's types and every validation rule; the
//! frontend receives only validated documents or structured errors, and never a path.

use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use open_circuit_engine::{
    Assignment, AssignmentDecoded, AssignmentError, AssignmentErrorCode, AssignmentSaveOutcome,
    MAX_ASSIGNMENT_BYTES, decode_assignment_json, encode_assignment_json,
};
use tempfile::NamedTempFile;

pub const ASSIGNMENT_EXTENSION: &str = "ocassignment";

/// The active assignment path is application state, never part of the assignment.
#[derive(Clone, Default)]
pub struct AssignmentSession {
    path: Arc<Mutex<Option<PathBuf>>>,
}

impl AssignmentSession {
    pub fn current_path(&self) -> Result<Option<PathBuf>, String> {
        self.path
            .lock()
            .map(|path| path.clone())
            .map_err(|_| "The assignment session is unavailable.".to_owned())
    }

    pub fn clear_path(&self) -> Result<(), String> {
        let mut path = self
            .path
            .lock()
            .map_err(|_| "The assignment session is unavailable.".to_owned())?;
        *path = None;
        Ok(())
    }

    /// Replaces the active path only after the selected file has been fully read and found
    /// valid. A file that is refused leaves the current location intact.
    pub fn load(&self, path: &Path) -> Result<AssignmentDecoded, String> {
        let decoded = read_assignment_file(path)?;
        if matches!(decoded, AssignmentDecoded::Valid { .. }) {
            let mut active_path = self
                .path
                .lock()
                .map_err(|_| "The assignment session is unavailable.".to_owned())?;
            *active_path = Some(path.to_path_buf());
        }
        Ok(decoded)
    }

    /// Saves the assignment and makes `path` the active path, only after the complete save
    /// has succeeded. An assignment that is not valid is not written.
    pub fn save_to(
        &self,
        path: &Path,
        assignment: &Assignment,
    ) -> Result<AssignmentSaveOutcome, String> {
        let outcome = write_assignment_file(path, assignment)?;
        if let (AssignmentSaveOutcome::Saved, Ok(saved_path)) =
            (&outcome, normalize_save_path(path))
        {
            let mut active_path = self
                .path
                .lock()
                .map_err(|_| "The assignment session is unavailable.".to_owned())?;
            *active_path = Some(saved_path);
        }
        Ok(outcome)
    }
}

/// Reads and validates an `.ocassignment` file. A file the Engine refuses, an oversized one
/// included, is a structured result; a file that cannot be read at all is an error string.
pub fn read_assignment_file(path: &Path) -> Result<AssignmentDecoded, String> {
    require_assignment_extension(path)?;

    let file =
        File::open(path).map_err(|error| format!("Could not read assignment file: {error}"))?;
    let too_large = || {
        Ok(AssignmentDecoded::Invalid {
            errors: vec![AssignmentError::new(AssignmentErrorCode::TooLarge, "")],
        })
    };
    if file
        .metadata()
        .map_err(|error| format!("Could not inspect assignment file: {error}"))?
        .len()
        > MAX_ASSIGNMENT_BYTES as u64
    {
        return too_large();
    }

    // Re-check after reading in case the file grew after its metadata was read.
    let mut bytes = Vec::new();
    file.take(MAX_ASSIGNMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read assignment file: {error}"))?;
    if bytes.len() > MAX_ASSIGNMENT_BYTES {
        return too_large();
    }

    Ok(AssignmentDecoded::from_result(decode_assignment_json(
        &bytes,
    )))
}

/// Writes a readable, deterministic JSON file by atomically replacing the destination with a
/// temporary file created in the same directory. The previous file survives any failure.
pub fn write_assignment_file(
    path: &Path,
    assignment: &Assignment,
) -> Result<AssignmentSaveOutcome, String> {
    let path = normalize_save_path(path)?;
    let bytes = match encode_assignment_json(assignment) {
        Ok(bytes) => bytes,
        Err(errors) => return Ok(AssignmentSaveOutcome::Invalid { errors }),
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| format!("Could not prepare an assignment save: {error}"))?;
    temporary
        .write_all(&bytes)
        .map_err(|error| format!("Could not write assignment file: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Could not flush assignment file: {error}"))?;
    temporary
        .persist(&path)
        .map_err(|error| format!("Could not complete assignment save: {}", error.error))?;

    Ok(AssignmentSaveOutcome::Saved)
}

fn has_assignment_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(ASSIGNMENT_EXTENSION))
}

fn require_assignment_extension(path: &Path) -> Result<(), String> {
    if has_assignment_extension(path) {
        Ok(())
    } else {
        Err("Open Circuit assignment files must use the .ocassignment extension.".to_owned())
    }
}

/// The path a save writes to: the dialog may leave the extension out, but it may not name
/// another one, so a save cannot overwrite a project or any other file type.
pub fn normalize_save_path(path: &Path) -> Result<PathBuf, String> {
    if has_assignment_extension(path) {
        return Ok(path.to_path_buf());
    }
    if path.extension().is_some() {
        return Err(
            "Open Circuit assignment files must use the .ocassignment extension.".to_owned(),
        );
    }
    let mut path = path.to_path_buf();
    path.set_extension(ASSIGNMENT_EXTENSION);
    Ok(path)
}
