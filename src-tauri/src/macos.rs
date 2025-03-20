use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};

use nix::fcntl::{open, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::{mkfifo, write};
use objc2::AllocAnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{NSAlert, NSApplication, NSApplicationActivationPolicy, NSCriticalAlertStyle, NSImage, NSInformationalAlertStyle, NSWindowLevel};
use objc2_foundation::{NSData, NSString};
use tauri::{AppHandle, Config, Emitter};

pub use objc2::MainThreadMarker;
pub use dispatch2::Queue;

use crate::SHOW_EVENT_NAME;

const ICON: &[u8] = include_bytes!("../icons/icon.icns");

// pub trait NSAlert: Sized {
//     unsafe fn alloc(_: Self) -> id {
//         msg_send![class!(NSAlert), alloc]
//     }
// }

fn fifo_path(config: &Config) -> PathBuf {
    let identifier = config.identifier.clone();
    let identifier = identifier.replace(['.', ','].as_ref(), "_");

    PathBuf::from(format!("/tmp/{}_instance", identifier))
}

fn run_fifo(path: &Path, app: AppHandle) {
    // We don't need to do too much caring here, if anything sends a message to the
    // file, we trigger the 'Show Window' handler..
    let path_inner = path.to_path_buf();

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
                Ok(f) => Some(unsafe { OwnedFd::from_raw_fd(f) }),
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

pub fn show_dock(mtm: MainThreadMarker) {
    // This is a little more involved, when we switch back to the regular policy, the icon will turn into a console.
    let ns_app = NSApplication::sharedApplication(mtm);
    ns_app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    unsafe { ns_app.setApplicationIconImage(get_icon().as_deref()); }
}

fn get_icon() -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(ICON);
    NSImage::initWithData(NSImage::alloc(), &data)
}

pub fn hide_dock(mtm: MainThreadMarker) {
    let ns_app = NSApplication::sharedApplication(mtm);
    ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

pub fn show_messagebox(mtm: MainThreadMarker, title: String, content: String) {
    unsafe {
        let alert = NSAlert::new(mtm);
        alert.setIcon(get_icon().as_deref());
        alert.setMessageText(&NSString::from_str(&title));
        alert.setInformativeText(&NSString::from_str(&content));
        alert.setAlertStyle(NSCriticalAlertStyle);

        // Get the Window..
        let window = alert.window();
        window.setLevel(NSWindowLevel::from(10u8));

        // Send the Alert..
        alert.runModal();
    }
}

pub fn show_question(mtm: MainThreadMarker, title: String, content: String) -> Result<(), ()> {
    let result: usize = unsafe {
        let alert = NSAlert::new(mtm);
        alert.setIcon(get_icon().as_deref());
        alert.addButtonWithTitle(&NSString::from_str("No"));
        alert.addButtonWithTitle(&NSString::from_str("Yes"));
        alert.setMessageText(&NSString::from_str(&title));
        alert.setInformativeText(&NSString::from_str(&content));
        alert.setAlertStyle(NSInformationalAlertStyle);

        // Get the Window..
        let window = alert.window();
        window.setLevel(NSWindowLevel::from(10u8));

        // Send the Alert..
        alert.runModal() as usize
    };
    if result == 1001 {
        return Ok(());
    }
    Err(())
}