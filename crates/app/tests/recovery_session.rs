//! Autosave beside an explicit save (docs/contracts/stage-16-editor-improvements.md section 9):
//! the recovery file never touches the project file, and a document restored from it has no path.

use std::fs;

use open_circuit_app_lib::project::{ProjectSession, read_project_file};
use open_circuit_app_lib::recovery::{RecoveryProbe, RecoveryStore};
use open_circuit_engine::{Schematic, decode_project_json};

const DIVIDER: &str = include_str!("../../../tests/golden/stage-1-divider.schematic.json");

fn divider() -> Schematic {
    decode_project_json(DIVIDER.as_bytes()).unwrap()
}

#[test]
fn autosave_never_writes_the_project_file_or_anything_next_to_it() {
    let workspace = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let project = workspace.path().join("lab.ocircuit");
    let session = ProjectSession::default();
    session.save_to(&project, &divider()).unwrap();
    let saved = fs::read(&project).unwrap();

    // The document is edited, and autosave writes it many times.
    let mut edited = divider();
    edited.components.truncate(2);
    let recovery = RecoveryStore::new(data.path().join("recovery"));
    for _ in 0..3 {
        recovery
            .write(&edited, session.current_path().unwrap().as_deref(), "1.0.0")
            .unwrap();
    }

    assert_eq!(fs::read(&project).unwrap(), saved);
    let beside: Vec<_> = fs::read_dir(workspace.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(beside, ["lab.ocircuit"]);
    assert!(!data.path().join("recovery").starts_with(workspace.path()));
}

#[test]
fn the_offer_names_the_project_and_restoring_it_leaves_the_original_alone() {
    let workspace = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let project = workspace.path().join("lab.ocircuit");
    let session = ProjectSession::default();
    session.save_to(&project, &divider()).unwrap();
    let saved = fs::read(&project).unwrap();
    let mut edited = divider();
    edited.components.truncate(2);
    let recovery = RecoveryStore::new(data.path().join("recovery"));
    recovery.write(&edited, Some(&project), "1.0.0").unwrap();

    // A new session after a crash: nothing is open, so a restored document has no path.
    let after_crash = ProjectSession::default();
    let RecoveryProbe::Offer(offer) = recovery.probe() else {
        panic!("a recovery file was written");
    };
    assert_eq!(
        offer.source_path.as_deref(),
        Some(project.to_string_lossy().as_ref())
    );
    assert_eq!(offer.schematic, edited);
    assert_eq!(after_crash.current_path().unwrap(), None);

    // The project on disk is still the last explicit save, and reading it gives the old document.
    assert_eq!(fs::read(&project).unwrap(), saved);
    assert_eq!(read_project_file(&project).unwrap(), divider());
}

#[test]
fn an_explicit_save_and_a_discard_leave_nothing_to_offer() {
    let data = tempfile::tempdir().unwrap();
    let recovery = RecoveryStore::new(data.path().join("recovery"));
    recovery.write(&divider(), None, "1.0.0").unwrap();
    assert!(matches!(recovery.probe(), RecoveryProbe::Offer(_)));

    recovery.discard().unwrap();

    assert_eq!(recovery.probe(), RecoveryProbe::None);
}
