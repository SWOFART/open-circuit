#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod drawing_pdf;

use std::sync::Arc;

use open_circuit_app_lib::assignment::{
    ASSIGNMENT_EXTENSION, AssignmentSession, normalize_save_path, write_assignment_file,
};
use open_circuit_app_lib::checks::CheckService;
use open_circuit_app_lib::project::ProjectSession;
use open_circuit_app_lib::recovery::{RecoveryProbe, RecoveryStore};
use open_circuit_app_lib::simulation::{
    FrameSink, ServiceError, SimulationConfig, SimulationService, SinkClosed, TraceSink,
};
use open_circuit_engine::{
    Assignment, AssignmentDecoded, AssignmentSaveOutcome, CheckRequest, CheckResult,
    CompileRequest, CompileResult, ComponentCatalog, DiagnosisChecked, Schematic,
    SetOperatorStateRequest, SetOperatorStateResult, SetParameterRequest, SetParameterResult,
    StableId, StreamNotice, TraceRequest, check_diagnosis, component_catalog,
    validate_assignment_draft_value,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Manager, RunEvent, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

/// Managed state: the one simulation thread, shared by every command.
struct Simulation(Arc<SimulationService>);

/// Sends encoded frames to the webview as raw bytes, which arrive as an `ArrayBuffer`. A
/// notice (the stop of a run) goes over the same channel as JSON, which arrives as an object, so
/// the two kinds of message cannot be mistaken for each other and no second channel exists.
struct ChannelSink(Channel<InvokeResponseBody>);

impl FrameSink for ChannelSink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0
            .send(InvokeResponseBody::Raw(frame))
            .map_err(|_| SinkClosed)
    }

    fn notice(&mut self, notice: &StreamNotice) -> Result<(), SinkClosed> {
        let json = serde_json::to_string(notice).map_err(|_| SinkClosed)?;
        self.0
            .send(InvokeResponseBody::Json(json))
            .map_err(|_| SinkClosed)
    }
}

impl TraceSink for ChannelSink {
    fn send(&mut self, response: Vec<u8>) -> Result<(), SinkClosed> {
        self.0
            .send(InvokeResponseBody::Raw(response))
            .map_err(|_| SinkClosed)
    }
}

/// Requests wait for the simulation thread, so they run off the command thread.
async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ServiceError> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

async fn run_project_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| format!("Project operation could not finish: {error}"))?
}

#[tauri::command]
fn get_component_catalog() -> ComponentCatalog {
    component_catalog()
}

#[tauri::command]
async fn documentation_diagnostics(
    schematic: Schematic,
    geometry: open_circuit_engine::GeometryEnvelope,
) -> Result<
    Result<Vec<open_circuit_engine::Diagnostic>, Vec<open_circuit_engine::Diagnostic>>,
    String,
> {
    run_blocking(move || {
        Ok(open_circuit_engine::documentation_diagnostics(
            &schematic, &geometry,
        ))
    })
    .await
}

#[tauri::command]
async fn number_wires(
    schematic: Schematic,
    geometry: open_circuit_engine::GeometryEnvelope,
) -> Result<
    Result<Vec<open_circuit_engine::WireNumberEntry>, Vec<open_circuit_engine::Diagnostic>>,
    String,
> {
    run_blocking(move || Ok(open_circuit_engine::number_wires(&schematic, &geometry))).await
}

#[tauri::command]
async fn sheet_diagnostics(
    schematic: Schematic,
    geometry: Vec<open_circuit_engine::SheetComponentBounds>,
) -> Result<Vec<open_circuit_engine::Diagnostic>, String> {
    run_project_blocking(move || {
        open_circuit_engine::validate_project_schematic(&schematic)
            .map_err(|error| error.to_string())?;
        if geometry.len() != schematic.components.len()
            || geometry.iter().any(|entry| {
                !entry.min_x.is_finite()
                    || !entry.max_x.is_finite()
                    || !entry.min_y.is_finite()
                    || !entry.max_y.is_finite()
                    || entry.min_x > entry.max_x
                    || entry.min_y > entry.max_y
            })
        {
            return Err("Invalid sheet geometry.".into());
        }
        Ok(open_circuit_engine::sheet_diagnostics(
            &schematic, &geometry,
        ))
    })
    .await
}

#[tauri::command]
async fn bill_of_materials(
    schematic: Schematic,
) -> Result<open_circuit_engine::BillOfMaterials, String> {
    run_project_blocking(move || {
        open_circuit_engine::validate_project_schematic(&schematic)
            .map_err(|error| error.to_string())?;
        Ok(open_circuit_engine::bill_of_materials(&schematic))
    })
    .await
}

#[tauri::command]
async fn export_drawing(
    app: tauri::AppHandle,
    format: String,
    data: serde_json::Value,
) -> Result<bool, String> {
    let (extension, label) = match format.as_str() {
        "svg" => ("svg", "SVG drawing"),
        "png" => ("png", "PNG drawing"),
        "pdf" => ("pdf", "PDF drawing"),
        _ => return Err("Unknown drawing export format.".into()),
    };
    let Some(selected) = app
        .dialog()
        .file()
        .set_title(label)
        .set_file_name(format!("Drawing.{extension}"))
        .add_filter(label, &[extension])
        .blocking_save_file()
    else {
        return Ok(false);
    };
    let mut path = selected
        .into_path()
        .map_err(|_| "Expected a local export file.")?;
    let appended_extension = match path.extension().and_then(|suffix| suffix.to_str()) {
        None => {
            path.set_extension(extension);
            true
        }
        Some(suffix) if suffix.eq_ignore_ascii_case(extension) => false,
        _ => return Err(format!("Use the .{extension} extension for this export.")),
    };
    run_project_blocking(move || {
        let bytes = drawing_bytes(extension, &data)?;
        write_drawing_file(&path, &bytes, appended_extension)?;
        Ok(true)
    })
    .await
}

fn drawing_bytes(format: &str, data: &serde_json::Value) -> Result<Vec<u8>, String> {
    match format {
        "pdf" => drawing_pdf::encode(data),
        "svg" => data
            .as_str()
            .filter(|xml| xml.len() <= 16 * 1024 * 1024)
            .map(|xml| xml.as_bytes().to_vec())
            .ok_or_else(|| "Invalid SVG export data.".into()),
        "png" => {
            let bytes = data
                .as_array()
                .filter(|bytes| bytes.len() <= 20 * 1024 * 1024)
                .ok_or("Invalid PNG export data.")?;
            bytes
                .iter()
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|byte| u8::try_from(byte).ok())
                        .ok_or_else(|| "Invalid PNG byte.".into())
                })
                .collect()
        }
        _ => Err("Unknown drawing export format.".into()),
    }
}

#[tauri::command]
async fn export_sheet_drawings(
    app: tauri::AppHandle,
    format: String,
    pages: Vec<serde_json::Value>,
) -> Result<Option<String>, String> {
    if !matches!(format.as_str(), "svg" | "png") || !(2..=32).contains(&pages.len()) {
        return Err("Invalid sheet export request.".into());
    }
    let Some(selected) = app
        .dialog()
        .file()
        .set_title("Choose where to export all sheets")
        .blocking_pick_folder()
    else {
        return Ok(None);
    };
    let parent = selected
        .into_path()
        .map_err(|_| "Expected a local export folder.")?;
    run_project_blocking(move || export_sheet_set(&parent, &format, &pages).map(Some)).await
}

fn export_sheet_set(
    parent: &std::path::Path,
    format: &str,
    pages: &[serde_json::Value],
) -> Result<String, String> {
    let directory = tempfile::Builder::new()
        .prefix("Open Circuit Sheets-")
        .tempdir_in(parent)
        .map_err(|_| "Could not create a sheet export folder.")?;
    for (index, page) in pages.iter().enumerate() {
        let bytes = drawing_bytes(format, page)?;
        let path = directory
            .path()
            .join(format!("Sheet-{:02}.{format}", index + 1));
        write_drawing_file(&path, &bytes, true)?;
    }
    let path = directory.keep();
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| "Could not report the sheet export folder.".into())
}

fn write_drawing_file(
    path: &std::path::Path,
    bytes: &[u8],
    appended_extension: bool,
) -> Result<(), String> {
    use std::io::Write as _;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "Could not prepare the drawing export.")?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| "Could not write the drawing export.")?;
    if appended_extension {
        temporary.persist_noclobber(path)
            .map_err(|_| "Could not save the drawing. Choose the full filename to replace an existing export.")?;
    } else {
        temporary
            .persist(path)
            .map_err(|_| "Could not replace the drawing export.")?;
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn an_existing_export_requires_the_selected_filename() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("Circuit.pdf");
    write_drawing_file(&file, b"initial", true).unwrap();
    assert!(write_drawing_file(&file, b"refused", true).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"initial");
    write_drawing_file(&file, b"updated", false).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"updated");
}

#[cfg(test)]
#[test]
fn sheet_export_keeps_all_pages_or_discards_the_new_folder() {
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path();
    let existing = parent.join("existing.svg");
    std::fs::write(&existing, "unchanged").unwrap();
    assert!(export_sheet_set(parent, "svg", &[json!("<svg/>"), json!(null)]).is_err());
    assert_eq!(std::fs::read_dir(parent).unwrap().count(), 1);
    let folder = export_sheet_set(parent, "svg", &[json!("<svg/>"), json!("<svg/>")]).unwrap();
    assert!(parent.join(&folder).join("Sheet-01.svg").exists());
    assert!(parent.join(&folder).join("Sheet-02.svg").exists());
    assert_eq!(std::fs::read_to_string(existing).unwrap(), "unchanged");
}

#[tauri::command]
async fn list_examples() -> Result<Vec<[String; 3]>, String> {
    run_project_blocking(open_circuit_app_lib::examples::list_examples).await
}

#[tauri::command]
async fn read_example(id: String, assignment_open: bool) -> Result<Schematic, String> {
    run_project_blocking(move || open_circuit_app_lib::examples::read_example(&id, assignment_open))
        .await
}

/// Opens a native file picker and replaces the active path only after validation succeeds.
#[tauri::command]
async fn open_project(
    app: tauri::AppHandle,
    session: State<'_, ProjectSession>,
    assignment_open: Option<bool>,
) -> Result<Option<Schematic>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("Open Open Circuit project")
        .add_filter("Open Circuit project", &["ocircuit"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Could not use the selected project file: {error}"))?;
    let project_path = path.clone();
    let project_session = session.inner().clone();
    // While an assignment is open, a project with DC rails or documentation is refused before
    // the project path changes (designators alone are dropped).
    let assignment_open = assignment_open.unwrap_or(false);
    let schematic =
        run_project_blocking(move || project_session.load_with(&project_path, assignment_open))
            .await?;
    Ok(Some(schematic))
}

/// Saves to the current project path or asks for a path when this is a new document.
#[tauri::command]
async fn save_project(
    app: tauri::AppHandle,
    session: State<'_, ProjectSession>,
    schematic: Schematic,
) -> Result<bool, String> {
    let path = match session.current_path()? {
        Some(path) => Some(path),
        None => choose_project_save_path(&app)?,
    };
    let Some(path) = path else {
        return Ok(false);
    };

    let project_session = session.inner().clone();
    run_project_blocking(move || project_session.save_to(&path, &schematic)).await?;
    Ok(true)
}

/// Saves the current schematic to a path selected with the native Save As dialog.
#[tauri::command]
async fn save_project_as(
    app: tauri::AppHandle,
    session: State<'_, ProjectSession>,
    schematic: Schematic,
) -> Result<bool, String> {
    let Some(path) = choose_project_save_path(&app)? else {
        return Ok(false);
    };

    let project_session = session.inner().clone();
    run_project_blocking(move || project_session.save_to(&path, &schematic)).await?;
    Ok(true)
}

/// The one recovery file's folder: `recovery/` in the application's local data directory, never
/// the project's folder (docs/contracts/stage-16-editor-improvements.md section 9).
fn recovery_store(app: &tauri::AppHandle) -> Result<RecoveryStore, String> {
    app.path()
        .app_local_data_dir()
        .map(|directory| RecoveryStore::new(directory.join("recovery")))
        .map_err(|error| format!("Could not find the application data folder: {error}"))
}

/// Autosave: writes the drawn document to the recovery file, replacing the previous one
/// atomically. It never writes the project file. A failure is a reason for the interface to show,
/// and the previous recovery file stays.
#[tauri::command]
async fn write_recovery(
    app: tauri::AppHandle,
    session: State<'_, ProjectSession>,
    schematic: Schematic,
) -> Result<(), String> {
    let store = recovery_store(&app)?;
    let source = session.current_path()?;
    run_project_blocking(move || {
        store.write(&schematic, source.as_deref(), env!("CARGO_PKG_VERSION"))
    })
    .await
}

/// At start: removes recovery files older than 30 days, then says what is left to offer. A file
/// that cannot be read is reported with the reason, so the application always starts.
#[tauri::command]
async fn probe_recovery(app: tauri::AppHandle) -> Result<RecoveryProbe, String> {
    let store = recovery_store(&app)?;
    run_project_blocking(move || {
        store.clean_old(std::time::SystemTime::now());
        Ok(store.probe())
    })
    .await
}

/// Deletes the recovery file: after an explicit save, a discarded New or Open, or a Discard at
/// the offer.
#[tauri::command]
async fn discard_recovery(app: tauri::AppHandle) -> Result<(), String> {
    let store = recovery_store(&app)?;
    run_project_blocking(move || store.discard()).await
}

/// Clears the Rust-owned file location after New or Open Example succeeds.
#[tauri::command]
fn clear_project_path(session: State<'_, ProjectSession>) -> Result<(), String> {
    session.clear_path()
}

fn choose_project_save_path(app: &tauri::AppHandle) -> Result<Option<std::path::PathBuf>, String> {
    let Some(file_path) = app
        .dialog()
        .file()
        .set_title("Save Open Circuit project")
        .set_file_name("Untitled.ocircuit")
        .add_filter("Open Circuit project", &["ocircuit"])
        .blocking_save_file()
    else {
        return Ok(None);
    };

    let path = file_path
        .into_path()
        .map_err(|error| format!("Could not use the selected project path: {error}"))?;
    Ok(Some(path))
}

/// Opens a native file picker and replaces the active assignment path only after the file
/// has been validated. A file the Engine refuses comes back as its list of errors.
#[tauri::command]
async fn open_assignment(
    app: tauri::AppHandle,
    session: State<'_, AssignmentSession>,
) -> Result<Option<AssignmentDecoded>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("Open Open Circuit assignment")
        .add_filter("Open Circuit assignment", &[ASSIGNMENT_EXTENSION])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Could not use the selected assignment file: {error}"))?;
    let assignment_session = session.inner().clone();
    let decoded = run_project_blocking(move || assignment_session.load(&path)).await?;
    Ok(Some(decoded))
}

/// Validates an assignment document with the Engine's rules, without touching any file.
#[tauri::command]
async fn validate_assignment(assignment: serde_json::Value) -> Result<AssignmentDecoded, String> {
    run_project_blocking(move || {
        Ok(AssignmentDecoded::from_result(
            validate_assignment_draft_value(&assignment),
        ))
    })
    .await
}

/// Saves to the current assignment path, or asks for a path when there is none. An invalid
/// document is refused with its errors before any dialog opens.
#[tauri::command]
async fn save_assignment(
    app: tauri::AppHandle,
    session: State<'_, AssignmentSession>,
    assignment: serde_json::Value,
) -> Result<AssignmentSaveOutcome, String> {
    let assignment = match validate_assignment_draft_value(&assignment) {
        Ok(assignment) => assignment,
        Err(errors) => return Ok(AssignmentSaveOutcome::Invalid { errors }),
    };
    let path = match session.current_path()? {
        Some(path) => Some(path),
        None => choose_assignment_save_path(&app, "Save Open Circuit assignment", "Untitled")?,
    };
    save_assignment_to(session.inner().clone(), path, assignment).await
}

/// Saves the assignment to a path selected with the native Save As dialog.
#[tauri::command]
async fn save_assignment_as(
    app: tauri::AppHandle,
    session: State<'_, AssignmentSession>,
    assignment: serde_json::Value,
) -> Result<AssignmentSaveOutcome, String> {
    let assignment = match validate_assignment_draft_value(&assignment) {
        Ok(assignment) => assignment,
        Err(errors) => return Ok(AssignmentSaveOutcome::Invalid { errors }),
    };
    let path = choose_assignment_save_path(&app, "Save Open Circuit assignment", "Untitled")?;
    save_assignment_to(session.inner().clone(), path, assignment).await
}

/// Writes the student's copy (the assignment without its reference, section 2.3) to a path
/// selected with the native dialog. The active assignment path does not change.
#[tauri::command]
async fn export_student_copy(
    app: tauri::AppHandle,
    assignment: serde_json::Value,
) -> Result<AssignmentSaveOutcome, String> {
    let assignment = match validate_assignment_draft_value(&assignment) {
        Ok(assignment) => assignment.student_copy(),
        Err(errors) => return Ok(AssignmentSaveOutcome::Invalid { errors }),
    };
    let Some(path) =
        choose_assignment_save_path(&app, "Export Open Circuit student copy", "Student copy")?
    else {
        return Ok(AssignmentSaveOutcome::Cancelled);
    };
    run_project_blocking(move || write_assignment_file(&path, &assignment)).await
}

/// Clears the Rust-owned assignment location after a new assignment is started.
#[tauri::command]
fn clear_assignment_path(session: State<'_, AssignmentSession>) -> Result<(), String> {
    session.clear_path()
}

async fn save_assignment_to(
    session: AssignmentSession,
    path: Option<std::path::PathBuf>,
    assignment: Assignment,
) -> Result<AssignmentSaveOutcome, String> {
    let Some(path) = path else {
        return Ok(AssignmentSaveOutcome::Cancelled);
    };
    run_project_blocking(move || session.save_to(&path, &assignment)).await
}

fn choose_assignment_save_path(
    app: &tauri::AppHandle,
    title: &str,
    name: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    let Some(file_path) = app
        .dialog()
        .file()
        .set_title(title)
        .set_file_name(format!("{name}.{ASSIGNMENT_EXTENSION}"))
        .add_filter("Open Circuit assignment", &[ASSIGNMENT_EXTENSION])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file_path
        .into_path()
        .map_err(|error| format!("Could not use the selected assignment path: {error}"))?;
    normalize_save_path(&path).map(Some)
}

/// Evaluates an assignment's checks on a student's schematic. The check has an Engine of its
/// own and runs away from the simulation thread, so it never delays or changes the live run.
/// The result carries the request's document revision.
#[tauri::command]
async fn check_assignment(
    checks: State<'_, Arc<CheckService>>,
    request: CheckRequest,
) -> Result<CheckResult, String> {
    let checks = Arc::clone(checks.inner());
    run_project_blocking(move || checks.run(request)).await
}

/// Evaluates a schema-2 build task with the same worker as legacy checks.
#[tauri::command]
async fn check_graded_assignment(
    checks: State<'_, Arc<CheckService>>,
    request: open_circuit_engine::GradedCheckRequest,
) -> Result<open_circuit_engine::GradedCheckResult, String> {
    let checks = Arc::clone(checks.inner());
    run_project_blocking(move || checks.run_graded(request)).await
}

/// Compares a fault-finding diagnosis with the assignment's faults. No circuit is solved.
#[tauri::command]
async fn check_fault_diagnosis(
    assignment: Assignment,
    diagnosis: serde_json::Value,
) -> Result<DiagnosisChecked, String> {
    run_project_blocking(move || {
        Ok(DiagnosisChecked::from_result(check_diagnosis(
            &assignment,
            &diagnosis,
        )))
    })
    .await
}

/// Compiles the schematic into a new run, with the request's optional faults applied to that
/// run only (they are never part of the schematic). Layout, diagnostics, and the initial snapshot
/// come back once, as JSON; frames follow only after `start_simulation`.
#[tauri::command]
async fn compile_circuit(
    simulation: State<'_, Simulation>,
    request: CompileRequest,
) -> Result<CompileResult, String> {
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.compile(request)).await
}

/// Recalculates one parameter of the active run through the Engine.
#[tauri::command]
async fn set_parameter(
    simulation: State<'_, Simulation>,
    request: SetParameterRequest,
) -> Result<SetParameterResult, String> {
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.set_parameter(request)).await
}

/// Moves one switch or push button of the active run through the Engine. The position is
/// operator state of the run: it is never saved and is not an edit of the document.
#[tauri::command]
async fn set_operator_state(
    simulation: State<'_, Simulation>,
    request: SetOperatorStateRequest,
) -> Result<SetOperatorStateResult, String> {
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.set_operator_state(request)).await
}

/// Reads the last committed Engine endpoint without advancing simulation time.
#[tauri::command]
async fn read_probe(
    simulation: State<'_, Simulation>,
    request: open_circuit_engine::ProbeRequest,
) -> Result<open_circuit_engine::ProbeResult, String> {
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.probe(request)).await
}

/// Lists one page of clamp targets for the requested run and document revision.
#[tauri::command]
async fn list_conductor_spans(
    simulation: State<'_, Simulation>,
    request: open_circuit_engine::ConductorSpanRequest,
) -> Result<Result<open_circuit_engine::ConductorSpanPage, open_circuit_engine::Diagnostic>, String>
{
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.conductor_spans(request)).await
}

/// Starts delivering binary frames of the active run over `on_frame`.
#[tauri::command]
async fn start_simulation(
    simulation: State<'_, Simulation>,
    run_id: StableId,
    on_frame: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    let service = Arc::clone(&simulation.0);
    let outcome =
        run_blocking(move || service.start(run_id, Box::new(ChannelSink(on_frame)))).await?;

    outcome.map_err(|diagnostics| {
        let code = diagnostics.first().map(|diagnostic| diagnostic.code);
        format!("the run cannot be started: {code:?}")
    })
}

/// Controls how often the worker takes existing public physics steps.
#[tauri::command]
async fn control_playback(
    simulation: State<'_, Simulation>,
    run_id: StableId,
    document_revision: String,
    action: String,
) -> Result<(), String> {
    use open_circuit_app_lib::simulation::PlaybackCommand;

    let command = match action.as_str() {
        "play" => PlaybackCommand::Play,
        "pause" => PlaybackCommand::Pause,
        "step" => PlaybackCommand::Step,
        "0.25" => PlaybackCommand::QuarterSpeed,
        "0.5" => PlaybackCommand::HalfSpeed,
        "1" => PlaybackCommand::NormalSpeed,
        "2" => PlaybackCommand::DoubleSpeed,
        _ => return Err("Unsupported playback action.".into()),
    };
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.playback(run_id, document_revision, command))
        .await?
        .map_err(|diagnostics| {
            let code = diagnostics.first().map(|diagnostic| diagnostic.code);
            format!("Playback command refused: {code:?}")
        })
}

/// Stops the stream of `run_id` and releases its channel. The run itself stays compiled.
/// A stop for a run that is no longer streaming is ignored.
#[tauri::command]
async fn stop_simulation(
    simulation: State<'_, Simulation>,
    run_id: StableId,
) -> Result<(), String> {
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.stop(Some(run_id))).await
}

/// The UI has processed every frame up to and including `frame_sequence`.
#[tauri::command]
async fn acknowledge_frame(
    simulation: State<'_, Simulation>,
    frame_sequence: u64,
) -> Result<(), String> {
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.acknowledge(frame_sequence)).await
}

/// Sends one bounded binary trace response for the requested run and signals.
#[tauri::command]
async fn read_traces(
    simulation: State<'_, Simulation>,
    request: TraceRequest,
    on_trace: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    let service = Arc::clone(&simulation.0);
    let outcome =
        run_blocking(move || service.read_traces(request, Box::new(ChannelSink(on_trace)))).await?;
    outcome.map_err(|diagnostics| {
        let code = diagnostics.first().map(|diagnostic| diagnostic.code);
        format!("the trace request cannot be served: {code:?}")
    })?;
    Ok(())
}

/// The UI has processed the trace response identified by this run and sequence.
#[tauri::command]
async fn acknowledge_trace(
    simulation: State<'_, Simulation>,
    run_id: StableId,
    trace_sequence: u64,
) -> Result<(), String> {
    let service = Arc::clone(&simulation.0);
    run_blocking(move || service.acknowledge_trace(run_id, trace_sequence)).await
}

fn main() {
    let simulation = Simulation(Arc::new(SimulationService::spawn(
        SimulationConfig::default(),
    )));

    tauri::Builder::default()
        .manage(simulation)
        .manage(ProjectSession::default())
        .manage(AssignmentSession::default())
        .manage(Arc::new(CheckService::default()))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_component_catalog,
            documentation_diagnostics,
            number_wires,
            bill_of_materials,
            sheet_diagnostics,
            export_drawing,
            export_sheet_drawings,
            open_project,
            list_examples,
            read_example,
            save_project,
            save_project_as,
            clear_project_path,
            write_recovery,
            probe_recovery,
            discard_recovery,
            open_assignment,
            validate_assignment,
            save_assignment,
            save_assignment_as,
            export_student_copy,
            clear_assignment_path,
            check_assignment,
            check_graded_assignment,
            check_fault_diagnosis,
            compile_circuit,
            set_parameter,
            set_operator_state,
            read_probe,
            list_conductor_spans,
            start_simulation,
            control_playback,
            stop_simulation,
            acknowledge_frame,
            read_traces,
            acknowledge_trace,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::Destroyed = event {
                let _ = window.state::<Simulation>().0.stop(None);
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build the Open Circuit desktop application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<Simulation>().0.shutdown();
            }
        });
}
