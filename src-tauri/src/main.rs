// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc;

use directories::ProjectDirs;
use interprocess::local_socket::tokio::LocalSocketStream;
use interprocess::local_socket::NameTypeSupport;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::fs::{create_dir_all, File};
use std::io::ErrorKind;

use std::path::{Path, PathBuf};

use crate::ipc::Socket;
use tauri::{AppHandle, Manager};
use tungstenite::{connect, Message};
use url::Url;

static WINDOW_NAME: &str = "main";
static READY_EVENT_NAME: &str = "READY";
static SHOW_EVENT_NAME: &str = "si-event";
static STOP_EVENT_NAME: &str = "seppuku";

static SOCKET_PATH: &str = "/tmp/goxlr.socket";
static NAMED_PIPE: &str = "@goxlr.socket";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Host(String);

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 {
        if args[1] == "--install" {
            manage(true).await;
            return Ok(());
        }
        if args[1] == "--remove" {
            manage(false).await;
            return Ok(());
        }
    }

    goxlr_preflight().await?;

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            // Trigger a global event if something (eg, the util) attempts to open this again.
            let _ = app.emit(SHOW_EVENT_NAME, None::<String>);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let global_window = app.handle().clone();
            app.listen_global(SHOW_EVENT_NAME, move |_| {
                // Do anything and everything to make sure this Window is visible and focused!
                let window = global_window.get_window(WINDOW_NAME).unwrap();
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            });

            let ready_handle = app.handle().clone();
            app.listen_global(READY_EVENT_NAME, move |data| {
                let address = data.payload();
                let window = ready_handle.get_window(WINDOW_NAME).unwrap();
                let _ = window.eval(format!("window.location.replace({})", address).as_str());
            });

            let shutdown_handle = app.handle().clone();
            app.listen_global(STOP_EVENT_NAME, move |_| {
                // Terminate the App..
                shutdown_handle.exit(0);
            });
            tokio::task::spawn(goxlr_utility_monitor(app.handle().clone()));

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // MacOS doesn't support single instance, so only hide if we're not there
                if !cfg!(macos) {
                    window.hide().unwrap();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error running tauri app");

    Ok(())
}

async fn goxlr_preflight() -> Result<(), String> {
    let connection = LocalSocketStream::connect(match NameTypeSupport::query() {
        NameTypeSupport::OnlyPaths | NameTypeSupport::Both => SOCKET_PATH,
        NameTypeSupport::OnlyNamespaced => NAMED_PIPE,
    })
    .await;

    if connection.is_err() {
        // We only support windows for these currently..
        let message = String::from("The GoXLR Utility must be running before launching this app.");
        show_error("GoXLR Utility UI".to_string(), message);

        return Err(String::from(
            "Unable to connect to the GoXLR Namespace / Unix Socket",
        ));
    }
    let mut socket: Socket<Value, Value> = Socket::new(connection.unwrap());
    if socket.send(json!("GetStatus")).await.is_ok() {
        if let Ok(Some(result)) = socket.try_read().await {
            if let Some(status) = result.get("Status") {
                if let Some(config) = status.get("config") {
                    if let Some(activation) = config.get("activation") {
                        if let Some(path) = activation.get("active_path") {
                            let exe = get_current_path();
                            if path.as_str().is_none()
                                || PathBuf::from(path.as_str().unwrap()) != exe
                            {
                                let title = String::from("GoXLR Utility UI");
                                let message = String::from("Use this app to control your GoXLR?");
                                if show_option(title, message).is_ok() {
                                    // Attempt to Register ourselves as the UI App..
                                    let command = format!(
                                        "{{ \"Daemon\": {{ \"SetActivatorPath\": {}  }} }}",
                                        Value::String(exe.to_string_lossy().to_string())
                                    );
                                    let json = serde_json::from_str::<Value>(&command).unwrap();
                                    let _ = socket.send(json).await;
                                } else {
                                    return Err(String::from("Unable to obtain user consent"));
                                }
                            }
                        }
                    } else {
                        show_error(String::from("GoXLR Utility UI"), String::from("Please shut down the Utility, then run:\n\ngoxlr-utility-ui --install"));
                        return Err(String::from("Unable to Install the UI"));
                    }
                }
            }
        }
    }

    Ok(())
}

async fn get_goxlr_host() -> Result<String, String> {
    let connection = LocalSocketStream::connect(match NameTypeSupport::query() {
        NameTypeSupport::OnlyPaths | NameTypeSupport::Both => SOCKET_PATH,
        NameTypeSupport::OnlyNamespaced => NAMED_PIPE,
    })
    .await;

    if connection.is_err() {
        return Err(String::from("Unable to Connect to the Utility"));
    }

    let mut socket: Socket<Value, Value> = Socket::new(connection.unwrap());

    // We need to dig quite far into the result to get what we need, and pretty much every
    // node is an Option, so yay.. I should probably unwrap_or_else..
    return if socket.send(json!("GetStatus")).await.is_ok() {
        if let Ok(Some(result)) = socket.try_read().await {
            if let Some(status) = result.get("Status") {
                if let Some(config) = status.get("config") {
                    if let Some(http_settings) = config.get("http_settings") {
                        if let Some(address) = http_settings.get("bind_address") {
                            if let Some(address) = address.as_str() {
                                if let Some(port) = http_settings.get("port") {
                                    if let Some(port) = port.as_u64() {
                                        Ok(format!("{}:{}", address, port))
                                    } else {
                                        Err("Unable to Parse Port".into())
                                    }
                                } else {
                                    Err("Port Missing from http_status".into())
                                }
                            } else {
                                Err("Unable to parse bind_address as String".into())
                            }
                        } else {
                            Err("bind_address Missing from http_status".into())
                        }
                    } else {
                        Err("http_settings missing from Config response".into())
                    }
                } else {
                    Err("config missing from Status response".into())
                }
            } else {
                Err("Status missing from GetStatus response!".into())
            }
        } else {
            Err("Unable to retrieve GetStatus Response".into())
        }
    } else {
        Err("Unable to obtain GoXLR Utility Address".into())
    };
}

async fn supports_activation(socket: &mut Socket<Value, Value>) -> bool {
    if socket.send(json!("GetStatus")).await.is_ok() {
        if let Ok(Some(result)) = socket.try_read().await {
            if let Some(status) = result.get("Status") {
                if let Some(config) = status.get("config") {
                    if let Some(_) = config.get("activation") {
                        return true;
                    }
                }
            }
        }
    }
    return false;
}

async fn goxlr_utility_monitor(handle: AppHandle) {
    print!("Spawning the Monitor..");

    // We're going to dive straight into the thread here..
    let host_result = get_goxlr_host().await;
    if host_result.is_err() {
        let _ = handle.emit(STOP_EVENT_NAME, None::<String>);
        return;
    }
    let host = host_result.unwrap();

    // Grab and Parse the URL..
    let ws_address = format!("ws://{}/api/websocket", host);
    let http_address = format!("http://{}/", host);
    let url = Url::parse(ws_address.as_str()).expect("Bad URL Provided");

    // Attempt to connect to the websocket..
    let result = connect(url);
    if result.is_err() {
        // We only support windows for these currently..
        show_error(
            "Unable to Launch UI".to_string(),
            "Unable to connect to the GoXLR Utility".to_string(),
        );
        let _ = handle.emit(STOP_EVENT_NAME, None::<String>);
        return;
    }

    // Got a good connection, grab the socket..
    let (mut socket, _) = result.unwrap();
    // Trigger the event that lets the window know we're ready..
    let _ = handle.emit(READY_EVENT_NAME, &http_address);

    // Anything that's not a valid message, or is a 'Close' message breaks the loop.
    while let Ok(message) = socket.read() {
        if let Message::Close(..) = message {
            // Break the loop so we can shutdown the app
            break;
        }
    }
    // Loop Ended, this happens when socket is closed.
    let _ = handle.emit(STOP_EVENT_NAME, None::<String>);
}

// Installs this app into the util..
async fn manage(install: bool) {
    println!("Checking if Utility is Running..");
    let connection = LocalSocketStream::connect(match NameTypeSupport::query() {
        NameTypeSupport::OnlyPaths | NameTypeSupport::Both => SOCKET_PATH,
        NameTypeSupport::OnlyNamespaced => NAMED_PIPE,
    })
    .await;

    if connection.is_err() {
        println!("Utility Not Running, changing config directly..");
        println!("Locating Settings File..");
        let path = get_settings_file();
        let json = if !&path.exists() {
            if !install {
                // If we're removing, and the path is missing, do nothing.
                return;
            }
            create_settings_path(&path);
            json!({ "activate": Value::Null })
        } else {
            load_settings(&path)
        };
        write_settings(&path, json, install);
    } else {
        println!("Utility Running, attempting via IPC");
        let exe = if install {
            format!("\"{}\"", get_current_path().to_string_lossy())
        } else {
            String::from("null")
        };
        let method = if install { "Install" } else { "Remove" };

        let mut socket: Socket<Value, Value> = Socket::new(connection.unwrap());
        if supports_activation(&mut socket).await {
            // Attempt to Register ourselves as the UI App..
            let command = format!("{{ \"Daemon\": {{ \"SetActivatorPath\": {} }} }}", exe);
            println!("Executing: {}", command);
            let json = serde_json::from_str::<Value>(&command).unwrap();
            let _ = socket.send(json).await;
        } else {
            show_error(
                String::from("GoXLR Utility UI"),
                format!("Unable to {}, Please stop the GoXLR Utility first.", method),
            );
        }
    }
}

fn get_current_path() -> PathBuf {
    if let Ok(app_image) = env::var("APPIMAGE") {
        println!("Using AppImage at {}", &app_image);
        PathBuf::from(app_image)
    } else {
        env::current_exe().unwrap()
    }
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
    let exe = get_current_path();

    value["activate"] = if install {
        Value::String(format!("{}", exe.to_string_lossy()))
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

#[cfg(target_os = "linux")]
fn show_error(title: String, message: String) {
    use std::process::Command;
    // We have two choices here, kdialog, or zenity. We'll try both.
    if let Err(e) = Command::new("kdialog")
        .arg("--title")
        .arg(title.clone())
        .arg("--error")
        .arg(message.clone())
        .output()
    {
        println!("Error Running kdialog: {}, falling back to zenity..", e);
        let _ = Command::new("zenity")
            .arg("--title")
            .arg(title)
            .arg("--error")
            .arg("--text")
            .arg(message)
            .output();
    }
}

#[cfg(target_os = "linux")]
fn show_option(title: String, message: String) -> Result<(), ()> {
    use std::process::Command;
    // We need to grab the return status..
    if let Ok(status) = Command::new("kdialog")
        .arg("--title")
        .arg(title.clone())
        .arg("--yesno")
        .arg(message.clone())
        .status()
    {
        if status.success() {
            Ok(())
        } else {
            Err(())
        }
    } else if let Ok(status) = Command::new("zenity")
        .arg("--title")
        .arg(title)
        .arg("--question")
        .arg("--text")
        .arg(message)
        .status()
    {
        if status.success() {
            Ok(())
        } else {
            Err(())
        }
    } else {
        // We weren't able to trigger kdialog, or zenity, this is a failure.
        Err(())
    }
}

#[cfg(target_os = "windows")]
fn show_error(title: String, message: String) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title = HSTRING::from(title);
    let message = HSTRING::from(message);

    unsafe {
        MessageBoxW(None, &message, &title, MB_ICONERROR | MB_OK);
    }
}

#[cfg(target_os = "windows")]
fn show_option(title: String, message: String) -> Result<(), ()> {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONQUESTION, MB_YESNO, MESSAGEBOX_RESULT,
    };

    let title = HSTRING::from(title);
    let message = HSTRING::from(message);

    unsafe {
        let result = MessageBoxW(None, &message, &title, MB_ICONQUESTION | MB_YESNO);

        // Either the windows crate doesn't have the enum, or I can't find it.. Either way..
        // https://learn.microsoft.com/en-us/dotnet/api/system.windows.messageboxresult
        if result == MESSAGEBOX_RESULT(6) {
            return Ok(());
        }
        Err(())
    }
}

#[cfg(target_os = "macos")]
fn show_error(_title: String, _message: String) {}

#[cfg(target_os = "macos")]
fn show_option(_title: String, _message: String) -> Result<(), ()> {
    Ok(())
}
