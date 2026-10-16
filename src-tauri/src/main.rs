//! Puri Cleaner desktop app: a window around the shared UI in `ui/`, with
//! one command that forwards every call to `puri_core::App`.

// No console window behind the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use puri_core::App;
use serde_json::Value;

#[tauri::command]
async fn api(state: tauri::State<'_, App>, cmd: String, args: Value) -> Result<Value, String> {
    state.call(&cmd, &args)
}

fn main() {
    tauri::Builder::default()
        .manage(App::new(None))
        .invoke_handler(tauri::generate_handler![api])
        .run(tauri::generate_context!())
        .expect("error while running Puri Cleaner");
}
