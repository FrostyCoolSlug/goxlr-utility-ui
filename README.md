# GoXLR Utility UI Wrapper
This is a simple app which uses tauri to wrap an app around the GoXLR Utilities configuration page.

The [Releases Page](https://github.com/FrostyCoolSlug/goxlr-utility-ui/releases) contains packages for various
platforms, tested as best as possible, the following are available:

- `.deb` files for Debian 12+ and Ubuntu 22.04+ (and their derivatives)
- `.rpm` files for **FEDORA** only, 37+
- `.AppImage` files for all other Linux platforms
- `.exe` for Windows, although these will be bundled with the Windows Installer
- AUR contains the package as `goxlr-utility-ui`

## Installation

The installation method can differ depending on which version of the GoXLR Utility you're running, for easiest
and best results, we recommend 1.0.6+, but instructions are also present for older versions.

In all cases, replace `goxlr-utility-ui` with the name of the binary relevant to your platform (for example
`goxlr-utility-ui.exe` or `goxlr-utility-ui.AppImage`).

### GoXLR Utility 1.0.6+
If installed via a package, open the Utility's UI in a browser, navigate to System -> Settings, and select 
'Application' as the 'UI Handler'. Click on the System Tray, or run the GoXLR Utility from your applications
menu to fire up the app.

#### Alternative:
Run the `goxlr-utility-ui` binary (or double click the AppImage) while the utility is running, you should be 
asked whether you want to use this app to control your GoXLR, click 'Yes', and the app will immediately launch.

#### Alternative:
From a command line, run `goxlr-utility-ui --install`, and the App will be installed, then click the tray
icon or run the GoXLR Utility from your applications menu.

### GoXLR Utility 1.0.5 and below
Shut down the GoXLR Utility and then run `goxlr-utility-ui --install`, once done, start the Utility again.

## Removal
### GoXLR Utility 1.0.6+
Navigate to System -> Settings, and change the 'UI Handler' back to 'Browser'.

#### Alternative
From a command line, run `goxlr-utility-ui --remove`, then either click the System Tray, or run the GoXLR Utility
from your applications menu.

### GoXLR Utility 1.0.5 and below
Shut down the GoXLR Utility and then run `goxlr-utility-ui --remove`, once done, start the Utility again.


## Note
This app is bound to the runtime of the Utility, once started it'll remain open until the utility exits.
If you press the 'Close' button on the Window, it'll simply hide itself away in the background until
it is needed again. This is primarily to ensure it's responsive, as spawning up a new browser and app 
every time someone clicks the button is a slow and heavy process. If this application is run while it
is already running, it'll un-hide the window and bring it to the front for instant access.

This app maintains a backend websocket connection to the utility, when that socket is closed, it's assumed
that the utility has exited, at which point this app will terminate cleanly.

## Support
This is primarily supported over at the [GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility) repo, 
where it's included in all Windows builds. Feel free to open an issue here if you have any problems!

## Building
Simple instructions:

* Install Rust via rustup
* Run `cargo install tauri-cli`
* Run `cargo tauri build`

The build may require a few packages from your repo, under linux `webkit2gtk`, `libappindicator` and `gtk` may be
required for building, [This Link](https://tauri.app/v1/guides/getting-started/prerequisites#setting-up-linux) should
have all the info needed for building on your system.

The binaries should be produced in `src-tauri/target/release/bundle`, I use the AppImage, you can do what
you want :)
