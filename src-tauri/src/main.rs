// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

#[derive(Clone, serde::Serialize)]
struct Payload {
    args: Vec<String>,
    cwd: String,
}

fn main() {
    // TODO: More Util related stuff, ask the Util for the web address, etc.

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            app.emit_all("single-instance", Payload { args: argv, cwd }).unwrap();
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let window = app.get_window("main").unwrap();
            let _ = window.eval("window.location.replace('http://localhost:14564')");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
