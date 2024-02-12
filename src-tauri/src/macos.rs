use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use nix::fcntl::{open, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::{mkfifo, write};
use tauri::{AppHandle, Config, Manager};

use crate::SHOW_EVENT_NAME;

fn fifo_path(config: &Config) -> PathBuf {
    let identifier = config.tauri.bundle.identifier.clone();
    let identifier = identifier.replace(['.', ','].as_ref(), "_");

    PathBuf::from(format!("/tmp/{}_instance", identifier))
}

fn run_fifo(path: &PathBuf, app: AppHandle) {
    // We don't need to do too much caring here, if anything sends a message to the
    // file, we trigger the 'Show Window' handler..
    let path_inner = path.clone();

    tokio::task::spawn(async move {
        loop {
            let file = File::open(&path_inner).unwrap();
            let reader = BufReader::new(file);
            for line in reader.lines() {
                println!("Received Relaunch Message: {}", line.unwrap());
                let _ = app.emit(SHOW_EVENT_NAME, None::<String>);
            }
        }
    });
}

pub fn setup_si(app: AppHandle) {
    let path = fifo_path(app.config());
    match mkfifo(&path, Mode::S_IRWXU) {
        Ok(_) => {
            // Should be good to run.
            run_fifo(&path, app);
        }
        Err(nix::Error::EEXIST) => {
            // We're going to need a Write only non blocking connection..
            let flags = OFlag::O_WRONLY | OFlag::O_NONBLOCK;

            let connection = match open(&path, flags, Mode::empty()) {
                Ok(f) => Some(f),
                Err(_) => None,
            };

            if let Some(connection) = connection {
                let message = SHOW_EVENT_NAME.as_bytes();
                if write(connection, message).is_ok() {
                    // Write successful, CYA!
                    std::process::exit(0);
                }
            }

            // If we get here, there's no handler on the other end of the file, so run ours.
            run_fifo(&path, app);
        }
        Err(e) => {
            eprintln!("Error Starting FIFO: {}", e);
        }
    }
}
