//! Autosave and crash recovery for the desktop shell
//! (docs/contracts/stage-16-editor-improvements.md section 9).
//!
//! The interface asks for three operations and shows the answers: write the drawn document to
//! the **one recovery file** of the application's local data directory, say what is there at
//! start, and delete it. This module owns the filesystem access and has no dependency on Tauri,
//! so the failure cases are tested without a window.
//!
//! - The recovery file is never the project file and never next to it; the project file is only
//!   written by an explicit save.
//! - A write goes to a temporary file in the same directory and is renamed over the previous
//!   recovery file, so a crash during a write leaves the old file or the new one, never half of
//!   one, and a failed write leaves the previous good file in place.
//! - The file is user data: it holds the drawing and a small envelope, is never sent anywhere,
//!   and neither its content nor a decoder's quotation of it is logged or returned in a message.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use open_circuit_engine::{Schematic, decode_project_json, validate_project_schematic};
use serde::{Deserialize, Serialize};
use tempfile::Builder;

/// The format name of the envelope.
pub const RECOVERY_FORMAT: &str = "open-circuit-recovery";
/// The version of the envelope; an unknown one is refused with a message.
pub const RECOVERY_VERSION: u32 = 1;
/// The project limit: a recovery file is never larger, in either direction.
pub const MAX_RECOVERY_BYTES: u64 = 16 * 1024 * 1024;
/// Recovery files older than this are removed at every start.
pub const RECOVERY_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

const RECOVERY_FILE: &str = "recovery.json";
const TEMPORARY_PREFIX: &str = "recovery-";
const TEMPORARY_SUFFIX: &str = ".tmp";
/// A reason shown to the user is shortened to this many characters.
const REASON_CHARS: usize = 200;

/// The small envelope around the drawn document.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    format: String,
    version: u32,
    /// When it was written, in milliseconds since the Unix epoch.
    written_at_ms: u64,
    /// The path of the project the document came from, or none for a new document.
    source_path: Option<String>,
    app_version: String,
    /// The drawn document exactly as a project file holds it.
    schematic: serde_json::Value,
}

/// What a recovery file holds, as offered to the user at start.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryOffer {
    pub written_at_ms: u64,
    pub source_path: Option<String>,
    pub app_version: String,
    pub schematic: Schematic,
}

/// What is in the recovery directory at start.
#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RecoveryProbe {
    /// No recovery file: the application starts clean.
    None,
    /// A recovery file that was read and decoded.
    Offer(Box<RecoveryOffer>),
    /// A recovery file that cannot be read or decoded: it is offered for deletion with the reason.
    Unreadable { reason: String },
}

/// The recovery file of one directory.
#[derive(Clone, Debug)]
pub struct RecoveryStore {
    directory: PathBuf,
}

impl RecoveryStore {
    /// A store that keeps its file in `directory` (created when the first write needs it).
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn file(&self) -> PathBuf {
        self.directory.join(RECOVERY_FILE)
    }

    /// Writes the recovery file for `schematic`, replacing the previous one atomically.
    ///
    /// The document is validated as a project is, and a file over the project limit is refused
    /// before anything is written. On any failure the previous recovery file is untouched.
    pub fn write(
        &self,
        schematic: &Schematic,
        source_path: Option<&Path>,
        app_version: &str,
    ) -> Result<(), String> {
        let written_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64);
        self.write_at(schematic, source_path, app_version, written_at_ms)
    }

    /// Like `write`, with the time of the write given.
    pub fn write_at(
        &self,
        schematic: &Schematic,
        source_path: Option<&Path>,
        app_version: &str,
        written_at_ms: u64,
    ) -> Result<(), String> {
        validate_project_schematic(schematic).map_err(|error| shorten(&error.to_string()))?;
        let envelope = Envelope {
            format: RECOVERY_FORMAT.to_owned(),
            version: RECOVERY_VERSION,
            written_at_ms,
            source_path: source_path.map(|path| path.to_string_lossy().into_owned()),
            app_version: app_version.to_owned(),
            schematic: serde_json::to_value(schematic)
                .map_err(|error| format!("Could not serialize the document: {error}"))?,
        };
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|error| format!("Could not serialize the recovery file: {error}"))?;
        if bytes.len() as u64 > MAX_RECOVERY_BYTES {
            return Err("The document is too large for a recovery file (16 MiB).".to_owned());
        }

        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("Could not create the recovery folder: {error}"))?;
        let mut temporary = Builder::new()
            .prefix(TEMPORARY_PREFIX)
            .suffix(TEMPORARY_SUFFIX)
            .tempfile_in(&self.directory)
            .map_err(|error| format!("Could not prepare the recovery file: {error}"))?;
        temporary
            .write_all(&bytes)
            .map_err(|error| format!("Could not write the recovery file: {error}"))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| format!("Could not flush the recovery file: {error}"))?;
        temporary
            .persist(self.file())
            .map_err(|error| format!("Could not complete the recovery file: {}", error.error))?;
        Ok(())
    }

    /// What is in the recovery directory. A file that cannot be read or decoded is reported
    /// with the reason, never as an error, so the application always starts.
    pub fn probe(&self) -> RecoveryProbe {
        match self.read() {
            Ok(None) => RecoveryProbe::None,
            Ok(Some(offer)) => RecoveryProbe::Offer(Box::new(offer)),
            Err(reason) => RecoveryProbe::Unreadable { reason },
        }
    }

    /// Reads the recovery file with the project reader's bound: a larger file is refused unread.
    pub fn read(&self) -> Result<Option<RecoveryOffer>, String> {
        let file = match File::open(self.file()) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Could not read the recovery file: {error}")),
        };
        if file
            .metadata()
            .map_err(|error| format!("Could not inspect the recovery file: {error}"))?
            .len()
            > MAX_RECOVERY_BYTES
        {
            return Err("The recovery file exceeds the 16 MiB size limit.".to_owned());
        }

        // Re-check after reading in case the file grew after its metadata was read.
        let mut bytes = Vec::new();
        file.take(MAX_RECOVERY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read the recovery file: {error}"))?;
        if bytes.len() as u64 > MAX_RECOVERY_BYTES {
            return Err("The recovery file exceeds the 16 MiB size limit.".to_owned());
        }

        // A decoder's message may quote the text it refused; the reason says only what kind of
        // problem it was.
        let envelope: Envelope = serde_json::from_slice(&bytes)
            .map_err(|_| "The recovery file is not a recovery file of this program.".to_owned())?;
        if envelope.format != RECOVERY_FORMAT {
            return Err("The recovery file is not a recovery file of this program.".to_owned());
        }
        if envelope.version != RECOVERY_VERSION {
            return Err(
                "The recovery file was made by a different version of the program.".to_owned(),
            );
        }
        let schematic_bytes = serde_json::to_vec(&envelope.schematic)
            .map_err(|_| "The recovery file holds a document that cannot be read.".to_owned())?;
        let schematic = decode_project_json(&schematic_bytes)
            .map_err(|_| "The recovery file holds a document that is not valid.".to_owned())?;

        Ok(Some(RecoveryOffer {
            written_at_ms: envelope.written_at_ms,
            source_path: envelope.source_path,
            app_version: envelope.app_version,
            schematic,
        }))
    }

    /// Deletes the recovery file, and the temporary files a crashed write left. Nothing there is
    /// not an error.
    pub fn discard(&self) -> Result<(), String> {
        match fs::remove_file(self.file()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not delete the recovery file: {error}")),
        }
        self.remove_temporary_files();
        Ok(())
    }

    /// Removes the recovery file when it is older than `RECOVERY_MAX_AGE` at `now`, and the
    /// temporary files of crashed writes (none is running at start). Returns whether the
    /// recovery file was removed. Problems are not reported: the clean-up is best effort.
    pub fn clean_old(&self, now: SystemTime) -> bool {
        self.remove_temporary_files();
        let Ok(modified) = fs::metadata(self.file()).and_then(|meta| meta.modified()) else {
            return false;
        };
        let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
        age > RECOVERY_MAX_AGE && fs::remove_file(self.file()).is_ok()
    }

    fn remove_temporary_files(&self) {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(TEMPORARY_PREFIX) && name.ends_with(TEMPORARY_SUFFIX) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// A reason is one line of at most `REASON_CHARS` characters.
fn shorten(reason: &str) -> String {
    let line = reason.lines().next().unwrap_or("");
    if line.chars().count() > REASON_CHARS {
        let mut shortened: String = line.chars().take(REASON_CHARS - 1).collect();
        shortened.push('…');
        shortened
    } else {
        line.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIVIDER: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");

    fn divider() -> Schematic {
        decode_project_json(DIVIDER.as_bytes()).unwrap()
    }

    fn store(directory: &tempfile::TempDir) -> RecoveryStore {
        RecoveryStore::new(directory.path().join("recovery"))
    }

    #[test]
    fn nothing_is_there_before_the_first_write() {
        let directory = tempfile::tempdir().unwrap();

        assert_eq!(store(&directory).probe(), RecoveryProbe::None);
        // A missing folder is not an error either way.
        assert_eq!(store(&directory).discard(), Ok(()));
    }

    #[test]
    fn a_written_document_is_offered_with_its_time_and_project() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);

        recovery
            .write_at(
                &divider(),
                Some(Path::new("C:/work/lab.ocircuit")),
                "1.2.3",
                1_700_000_000_000,
            )
            .unwrap();

        let RecoveryProbe::Offer(offer) = recovery.probe() else {
            panic!("a recovery file was written");
        };
        assert_eq!(offer.written_at_ms, 1_700_000_000_000);
        assert_eq!(offer.source_path.as_deref(), Some("C:/work/lab.ocircuit"));
        assert_eq!(offer.app_version, "1.2.3");
        assert_eq!(offer.schematic, divider());
    }

    #[test]
    fn a_new_document_has_no_project_path() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);

        recovery.write_at(&divider(), None, "1.0.0", 5).unwrap();

        let RecoveryProbe::Offer(offer) = recovery.probe() else {
            panic!("a recovery file was written");
        };
        assert_eq!(offer.source_path, None);
    }

    #[test]
    fn a_second_write_replaces_the_first_and_leaves_no_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);

        recovery.write_at(&divider(), None, "1.0.0", 1).unwrap();
        recovery.write_at(&divider(), None, "1.0.0", 2).unwrap();

        let RecoveryProbe::Offer(offer) = recovery.probe() else {
            panic!("a recovery file was written");
        };
        assert_eq!(offer.written_at_ms, 2);
        let names: Vec<_> = fs::read_dir(directory.path().join("recovery"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, [RECOVERY_FILE]);
    }

    #[test]
    fn a_failed_write_keeps_the_previous_good_file() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        recovery.write_at(&divider(), None, "1.0.0", 1).unwrap();
        let before = fs::read(recovery.file()).unwrap();

        // A document the project rules refuse is never written, and nothing is deleted first.
        let mut invalid = divider();
        invalid.components.clear();
        invalid.wires.push(invalid.wires[0].clone());
        let result = recovery.write_at(&invalid, None, "1.0.0", 2);

        assert!(result.is_err());
        assert_eq!(fs::read(recovery.file()).unwrap(), before);
    }

    #[test]
    fn a_folder_that_cannot_be_created_is_a_reason_not_a_panic() {
        let directory = tempfile::tempdir().unwrap();
        // A file where the folder should be.
        let blocker = directory.path().join("recovery");
        fs::write(&blocker, "not a folder").unwrap();

        let result = RecoveryStore::new(blocker).write_at(&divider(), None, "1.0.0", 1);

        assert!(result.unwrap_err().contains("recovery"));
    }

    #[test]
    fn a_document_over_the_project_limit_is_refused_before_anything_is_written() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        let mut huge = divider();
        // Notes are documentation; many long ones push the file past 16 MiB.
        let note = open_circuit_engine::NoteSpec {
            id: open_circuit_engine::StableId::parse("00000000-0000-4000-8000-0000000000a1")
                .unwrap(),
            position: open_circuit_engine::GridPoint { x: 0, y: 0 },
            text: "x".repeat(MAX_RECOVERY_BYTES as usize + 1),
        };
        huge.documentation.notes.push(note);

        let result = recovery.write_at(&huge, None, "1.0.0", 1);

        assert!(result.is_err());
        assert!(!recovery.directory.exists());
    }

    #[test]
    fn a_file_that_is_not_a_recovery_file_is_unreadable_with_a_reason_that_quotes_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        fs::create_dir_all(&recovery.directory).unwrap();
        fs::write(recovery.file(), r#"{"secret drawing text": 1"#).unwrap();

        let RecoveryProbe::Unreadable { reason } = recovery.probe() else {
            panic!("the file is not a recovery file");
        };

        assert!(!reason.contains("secret"));
        assert!(reason.contains("recovery file"));
    }

    #[test]
    fn an_unknown_format_or_version_is_refused_with_a_message() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        recovery.write_at(&divider(), None, "1.0.0", 1).unwrap();
        let text = fs::read_to_string(recovery.file()).unwrap();

        fs::write(recovery.file(), text.replace(RECOVERY_FORMAT, "other")).unwrap();
        assert!(matches!(
            recovery.probe(),
            RecoveryProbe::Unreadable { reason } if reason.contains("not a recovery file")
        ));

        fs::write(
            recovery.file(),
            text.replace("\"version\":1", "\"version\":2"),
        )
        .unwrap();
        assert!(matches!(
            recovery.probe(),
            RecoveryProbe::Unreadable { reason } if reason.contains("different version")
        ));
    }

    #[test]
    fn an_extra_field_or_an_invalid_document_is_unreadable() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        recovery.write_at(&divider(), None, "1.0.0", 1).unwrap();
        let text = fs::read_to_string(recovery.file()).unwrap();

        fs::write(recovery.file(), text.replacen('{', "{\"extra\":true,", 1)).unwrap();
        assert!(matches!(recovery.probe(), RecoveryProbe::Unreadable { .. }));

        // A rotation the project format refuses.
        fs::write(
            recovery.file(),
            text.replace("\"rotationDeg\":0", "\"rotationDeg\":45"),
        )
        .unwrap();
        assert!(matches!(
            recovery.probe(),
            RecoveryProbe::Unreadable { reason } if reason.contains("not valid")
        ));
    }

    #[test]
    fn a_file_over_the_limit_is_refused_unread() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        fs::create_dir_all(&recovery.directory).unwrap();
        let file = File::create(recovery.file()).unwrap();
        file.set_len(MAX_RECOVERY_BYTES + 1).unwrap();

        assert!(matches!(
            recovery.probe(),
            RecoveryProbe::Unreadable { reason } if reason.contains("16 MiB")
        ));
    }

    #[test]
    fn discard_deletes_the_file_and_the_temporary_files_of_a_crashed_write() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        recovery.write_at(&divider(), None, "1.0.0", 1).unwrap();
        let crashed = recovery.directory.join("recovery-abc.tmp");
        fs::write(&crashed, "half").unwrap();
        let unrelated = recovery.directory.join("notes.txt");
        fs::write(&unrelated, "mine").unwrap();

        recovery.discard().unwrap();

        assert_eq!(recovery.probe(), RecoveryProbe::None);
        assert!(!crashed.exists());
        assert!(unrelated.exists());
        // Deleting twice is fine.
        recovery.discard().unwrap();
    }

    #[test]
    fn a_file_older_than_thirty_days_is_removed_at_start_and_a_newer_one_stays() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        recovery.write_at(&divider(), None, "1.0.0", 1).unwrap();
        let now = SystemTime::now();

        assert!(!recovery.clean_old(now));
        assert!(!recovery.clean_old(now + RECOVERY_MAX_AGE - Duration::from_secs(3600)));
        assert!(matches!(recovery.probe(), RecoveryProbe::Offer(_)));

        assert!(recovery.clean_old(now + RECOVERY_MAX_AGE + Duration::from_secs(3600)));
        assert_eq!(recovery.probe(), RecoveryProbe::None);
    }

    #[test]
    fn the_clean_up_removes_the_leftovers_of_a_crashed_write_and_never_fails() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);

        // No folder at all.
        assert!(!recovery.clean_old(SystemTime::now()));

        fs::create_dir_all(&recovery.directory).unwrap();
        let crashed = recovery.directory.join("recovery-xyz.tmp");
        fs::write(&crashed, "half").unwrap();
        assert!(!recovery.clean_old(SystemTime::now()));
        assert!(!crashed.exists());
    }

    #[test]
    fn the_probe_is_sent_to_the_interface_as_a_tagged_object() {
        let directory = tempfile::tempdir().unwrap();
        let recovery = store(&directory);
        assert_eq!(
            serde_json::to_value(recovery.probe()).unwrap(),
            serde_json::json!({"kind": "none"})
        );

        recovery
            .write_at(&divider(), Some(Path::new("lab.ocircuit")), "1.0.0", 7)
            .unwrap();
        let offer = serde_json::to_value(recovery.probe()).unwrap();
        assert_eq!(offer["kind"], "offer");
        assert_eq!(offer["writtenAtMs"], 7);
        assert_eq!(offer["sourcePath"], "lab.ocircuit");
        assert_eq!(offer["appVersion"], "1.0.0");
        assert_eq!(offer["schematic"]["schemaVersion"], 3);

        fs::write(recovery.file(), "{").unwrap();
        let unreadable = serde_json::to_value(recovery.probe()).unwrap();
        assert_eq!(unreadable["kind"], "unreadable");
        assert!(unreadable["reason"].is_string());
    }

    #[test]
    fn a_reason_is_one_short_line() {
        assert_eq!(shorten("first\nsecond"), "first");
        let long = shorten(&"x".repeat(500));
        assert_eq!(long.chars().count(), REASON_CHARS);
        assert!(long.ends_with('…'));
    }
}
