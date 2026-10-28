#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use open_circuit_app_lib::assignment::{
    ASSIGNMENT_EXTENSION, AssignmentSession, normalize_save_path, write_assignment_file,
};
use open_circuit_app_lib::checks::CheckService;
use open_circuit_app_lib::project::ProjectSession;
use open_circuit_app_lib::simulation::{
    FrameSink, ServiceError, SimulationConfig, SimulationService, SinkClosed, TraceSink,
};
use open_circuit_engine::{
    Assignment, AssignmentDecoded, AssignmentSaveOutcome, CheckRequest, CheckResult,
    CompileRequest, CompileResult, ComponentCatalog, DiagnosisChecked, Schematic,
    SetParameterRequest, SetParameterResult, StableId, TraceRequest, check_diagnosis,
    component_catalog, validate_assignment_value,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Manager, RunEvent, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

/// Managed state: the one simulation thread, shared by every command.
struct Simulation(Arc<SimulationService>);

/// Sends encoded frames to the webview as raw bytes, which arrive as an `ArrayBuffer`.
struct ChannelSink(Channel<InvokeResponseBody>);

impl FrameSink for ChannelSink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), SinkClosed> {
        self.0
            .send(InvokeResponseBody::Raw(frame))
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

/// Opens a native file picker and replaces the active path only after validation succeeds.
#[tauri::command]
async fn open_project(
    app: tauri::AppHandle,
    session: State<'_, ProjectSession>,
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
    let schematic = run_project_blocking(move || project_session.load(&project_path)).await?;
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
        Ok(AssignmentDecoded::from_result(validate_assignment_value(
            &assignment,
        )))
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
    let assignment = match validate_assignment_value(&assignment) {
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
    let assignment = match validate_assignment_value(&assignment) {
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
    let assignment = match validate_assignment_value(&assignment) {
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
            open_project,
            save_project,
            save_project_as,
            clear_project_path,
            open_assignment,
            validate_assignment,
            save_assignment,
            save_assignment_as,
            export_student_copy,
            clear_assignment_path,
            check_assignment,
            check_fault_diagnosis,
            compile_circuit,
            set_parameter,
            start_simulation,
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
