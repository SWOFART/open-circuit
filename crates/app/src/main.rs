#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn get_component_catalog() -> open_circuit_engine::ComponentCatalog {
    open_circuit_engine::component_catalog()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_component_catalog])
        .run(tauri::generate_context!())
        .expect("failed to run the Open Circuit desktop application");
}
