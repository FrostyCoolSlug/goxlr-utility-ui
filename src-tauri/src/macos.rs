use cocoa::appkit::{
    NSApplicationActivationPolicyAccessory, NSApplicationActivationPolicyRegular, NSImage,
};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSData, NSString};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use nix::fcntl::{open, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::{mkfifo, write};
use objc::{class, msg_send, sel, sel_impl};
use tauri::{AppHandle, Config, Manager};

use crate::SHOW_EVENT_NAME;

const ICON: &[u8] = include_bytes!("../icons/128x128.png");

pub trait NSAlert: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSAlert), alloc]
    }
}

fn fifo_path(config: &Config) -> PathBuf {
    let identifier = config.tauri.bundle.identifier.clone();
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

pub fn show_dock() {
    // This is a little more involved, when we switch back to the regular policy, the icon will turn into a console.
    unsafe {
        let ns_app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, setActivationPolicy: NSApplicationActivationPolicyRegular];
        let _: () = msg_send![ns_app, setApplicationIconImage: get_icon()];
    }
}

fn get_icon() -> id {
    unsafe {
        let data = NSData::dataWithBytes_length_(
            nil,
            ICON.as_ptr() as *const std::os::raw::c_void,
            ICON.len() as u64,
        );
        NSImage::initWithData_(NSImage::alloc(nil), data)
    }
}

pub fn hide_dock() {
    unsafe {
        let ns_app: id = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, setActivationPolicy: NSApplicationActivationPolicyAccessory];
    }
}

pub fn show_messagebox(title: String, content: String) {
    unsafe {
        let alert: id = msg_send![class!(NSAlert), alloc];
        let () = msg_send![alert, init];
        let () = msg_send![alert, autorelease];
        let () = msg_send![alert, setIcon: get_icon()];
        let () = msg_send![alert, setMessageText: NSString::alloc(nil).init_str(&title)];
        let () = msg_send![alert, setInformativeText: NSString::alloc(nil).init_str(&content)];
        let () = msg_send![alert, setAlertStyle: 2];

        // Get the Window..
        let window: id = msg_send![alert, window];
        let () = msg_send![window, setLevel: 10];

        // Send the Alert..
        let () = msg_send![alert, runModal];
    }
}

pub fn show_question(title: String, content: String) -> Result<(), ()> {
    let result: usize = unsafe {
        let alert: id = msg_send![class!(NSAlert), alloc];
        let () = msg_send![alert, init];
        let () = msg_send![alert, autorelease];
        let () = msg_send![alert, setIcon: get_icon()];
        let () = msg_send![alert, addButtonWithTitle: NSString::alloc(nil).init_str("No")];
        let () = msg_send![alert, addButtonWithTitle: NSString::alloc(nil).init_str("Yes")];
        let () = msg_send![alert, setMessageText: NSString::alloc(nil).init_str(&title)];
        let () = msg_send![alert, setInformativeText: NSString::alloc(nil).init_str(&content)];
        let () = msg_send![alert, setAlertStyle: 1];

        // Get the Window..
        let window: id = msg_send![alert, window];
        let () = msg_send![window, setLevel: 10];

        //let ns_app: id = msg_send![class!(NSApplication), sharedApplication];
        msg_send![alert, runModal]
    };
    if result == 1001 {
        return Ok(());
    }
    Err(())
}
