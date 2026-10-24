#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use open_circuit_app_lib::simulation::{
    FrameSink, ServiceError, SimulationConfig, SimulationService, SinkClosed,
};
use open_circuit_engine::{
    CompileRequest, CompileResult, ComponentCatalog, SetParameterRequest, SetParameterResult,
    StableId, component_catalog,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Manager, RunEvent, State, WindowEvent};

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

/// Requests wait for the simulation thread, so they run off the command thread.
async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ServiceError> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_component_catalog() -> ComponentCatalog {
    component_catalog()
}

/// Compiles the schematic into a new run. Layout, diagnostics, and the initial snapshot
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

fn main() {
    let simulation = Simulation(Arc::new(SimulationService::spawn(
        SimulationConfig::default(),
    )));

    tauri::Builder::default()
        .manage(simulation)
        .invoke_handler(tauri::generate_handler![
            get_component_catalog,
            compile_circuit,
            set_parameter,
            start_simulation,
            stop_simulation,
            acknowledge_frame,
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
