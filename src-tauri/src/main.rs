// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use directories::ProjectDirs;
use goxlr_ipc::client::Client;
use goxlr_ipc::clients::ipc::ipc_client::IPCClient;
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse};
use interprocess::local_socket::tokio::LocalSocketStream;
use interprocess::local_socket::NameTypeSupport;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{create_dir_all, File};
use std::io::ErrorKind;
use std::iter::once;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::{env, thread};
use tauri::{AppHandle, Manager, Wry};
use tungstenite::{connect, Message};
use url::Url;

static WINDOW_NAME: &str = "main";
static SHOW_EVENT_NAME: &str = "si-event";
static STOP_EVENT_NAME: &str = "seppuku";

// Why do I need to define there? :D
static SOCKET_PATH: &str = "/tmp/goxlr.socket";
static NAMED_PIPE: &str = "@goxlr.socket";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Host(String);

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 {
        if args[1] == "--install" {
            manage(true);
            return;
        }
        if args[1] == "--remove" {
            manage(false);
            return;
        }
    }

    let base_host = get_goxlr_host().await;

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            // Trigger a global event if something (eg, the util) attempts to open this again.
            app.trigger_global(SHOW_EVENT_NAME, None);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let global_window = app.handle();
            app.listen_global(SHOW_EVENT_NAME, move |_| {
                // Do anything and everything to make sure this Window is visible and focused!
                let window = global_window.get_window(WINDOW_NAME).unwrap();
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            });

            let shutdown_handle = app.handle();
            app.listen_global(STOP_EVENT_NAME, move |_| {
                // Terminate the App..
                shutdown_handle.exit(0);
            });

            // Spawn the GoXLR Utility Monitor..
            goxlr_utility_monitor(base_host, app.handle());

            let window = app.get_window(WINDOW_NAME).unwrap();
            let _ = window.eval("window.location.replace('http://localhost:14564')");
            Ok(())
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                event.window().hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}

async fn get_goxlr_host() -> String {
    let connection = LocalSocketStream::connect(match NameTypeSupport::query() {
        NameTypeSupport::OnlyPaths | NameTypeSupport::Both => SOCKET_PATH,
        NameTypeSupport::OnlyNamespaced => NAMED_PIPE,
    })
    .await;

    if connection.is_err() {
        // We only support windows for these currently..
        #[cfg(target_os = "windows")]
        {
            let _ = show_dialog(
                "Unable to Launch UI".to_string(),
                "The GoXLR Utility must be running before launching this app.".to_string(),
                Icon::ERROR,
            );
        }
        panic!("Unable to connect to the GoXLR NameSpace / Unix Socket");
    }

    let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection.unwrap());
    let mut client = IPCClient::new(socket);
    let _ = client.poll_status().await;
    let status = client.http_status();
    let host = if status.bind_address != "localhost" && status.bind_address != "0.0.0.0" {
        status.bind_address.clone()
    } else {
        "localhost".to_string()
    };

    format!("{}:{}", host, status.port)
}

fn goxlr_utility_monitor(host: String, handle: AppHandle<Wry>) {
    print!("Spawining the Monitor..");

    // Grab and Parse the URL..
    let address = format!("ws://{}/api/websocket", host);
    println!("Err: {}", address);
    let url = Url::parse(address.as_str()).expect("Bad URL Provided");

    // Attempt to connect to the websocket..
    let result = connect(url);
    if result.is_err() {
        // We only support windows for these currently..
        #[cfg(target_os = "windows")]
        {
            let _ = show_dialog(
                "Unable to Launch UI".to_string(),
                "Unable to connect to the GoXLR Utility".to_string(),
                Icon::ERROR,
            );
            panic!("Unable to Connect to the Utility");
        }
    }

    let (mut socket, _) = result.unwrap();
    thread::spawn(move || {
        // Anything that's not a valid message, or is a 'Close' message breaks the loop.
        while let Ok(message) = socket.read_message() {
            if let Message::Close(..) = message {
                // Break the loop so we can shutdown the app
                break;
            }
        }
        // Loop Ended, this happens when socket is closed.
        handle.trigger_global(STOP_EVENT_NAME, None);
    });
}

// Installs this app into the util..
fn manage(install: bool) {
    println!("Locating Settings File..");
    let path = get_settings_file();
    let json = if !&path.exists() {
        if !install {
            // If we're removing, and the path is missing, do nothing.
            return;
        }
        create_settings_path(&path);
        json!("{}")
    } else {
        load_settings(&path)
    };
    write_settings(&path, json, install);
}

fn create_settings_path(path: &Path) {
    println!("Creating path if needed..");
    if let Some(parent) = path.parent() {
        if let Err(e) = create_dir_all(parent) {
            if e.kind() != ErrorKind::AlreadyExists {
                panic!("Unable to Create Project Directory");
            }
        }
    }
}

fn load_settings(path: &PathBuf) -> Value {
    println!("Loading Existing Settings..");
    let path_str = String::from(path.to_string_lossy());
    match File::open(path) {
        Ok(reader) => serde_json::from_reader(reader)
            .unwrap_or_else(|_| panic!("Could not parse daemon settings file at {}", path_str)),
        Err(_) => panic!(
            "Could not open daemon settings file for reading at {}",
            path_str
        ),
    }
}

fn write_settings(path: &PathBuf, mut value: Value, install: bool) {
    let exe = env::current_exe().unwrap();

    value["activate"] = if install {
        Value::String(String::from(exe.to_string_lossy()))
    } else {
        Value::Null
    };

    let path_str = String::from(path.to_string_lossy());
    let writer = File::create(path).unwrap_or_else(|_| {
        panic!(
            "Could not open daemon settings file for writing at {}",
            path_str
        )
    });
    serde_json::to_writer_pretty(writer, &value)
        .unwrap_or_else(|_| panic!("Could not write to daemon settings file at {}", path_str));
}

fn get_settings_file() -> PathBuf {
    let proj_dirs = ProjectDirs::from("org", "GoXLR-on-Linux", "GoXLR-Utility")
        .expect("Couldn't find project directories");
    proj_dirs.config_dir().join("settings.json")
}

#[cfg(target_os = "windows")]
fn show_dialog(title: String, message: String, icon: Icon) -> Result<(), String> {
    use winapi::um::winuser::{MessageBoxW, MB_ICONERROR};

    let icon = match icon {
        Icon::ERROR => MB_ICONERROR,
    };

    let lp_title: Vec<u16> = title.encode_utf16().chain(once(0)).collect();
    let lp_message: Vec<u16> = message.encode_utf16().chain(once(0)).collect();

    unsafe {
        match MessageBoxW(null_mut(), lp_message.as_ptr(), lp_title.as_ptr(), icon) {
            0 => Err("Unable to Create Dialog".to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(target_os = "windows")]
enum Icon {
    ERROR,
}
