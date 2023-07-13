// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use goxlr_ipc::client::Client;
use goxlr_ipc::clients::ipc::ipc_client::IPCClient;
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse};
use interprocess::local_socket::tokio::LocalSocketStream;
use interprocess::local_socket::NameTypeSupport;
use serde::{Deserialize, Serialize};
use std::iter::once;
use std::ptr::null_mut;
use std::thread;
use tauri::{AppHandle, Manager, Wry};
use tungstenite::{connect, Message};
use url::Url;
use winapi::shared::minwindef::UINT;
use winapi::um::winuser::{MessageBoxW, MB_ICONERROR};

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
                MB_ICONERROR,
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
                MB_ICONERROR,
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

#[cfg(target_os = "windows")]
fn show_dialog(title: String, message: String, icon: UINT) -> Result<(), String> {
    let lp_title: Vec<u16> = title.encode_utf16().chain(once(0)).collect();
    let lp_message: Vec<u16> = message.encode_utf16().chain(once(0)).collect();

    unsafe {
        match MessageBoxW(null_mut(), lp_message.as_ptr(), lp_title.as_ptr(), icon) {
            0 => Err("Unable to Create Dialog".to_string()),
            _ => Ok(()),
        }
    }
}
