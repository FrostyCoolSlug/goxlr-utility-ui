# GoXLR Utility UI Wrapper
This is a simple app which uses tauri to wrap an app around the GoXLR Utilities configuration page.

To install, shutdown the GoXLR Utility and run the following commands (replacing `goxlr-utility-ui` with the
correct binary for your platform):


`goxlr-utility-ui --install`

Then start up the utility, it should immediately start using the UI instead of a web browser, if it doesn't work
or you encouter issues with it, it can be removed by shutting down the GoXLR Utility and running:

`goxlr-utility-ui --remove`

This app is bound to the runtime of the Utility, once started it'll remain open until the utility exits.
If you press the 'Close' button on the Window, it'll simply hide itself away in the background until
it is needed again. This is primarily to ensure it's responsive, as spawning up a new browser and app 
every time someone clicks the button is a slow and heavy process. If this application is run while it
is already running, it'll un-hide the window and bring it to the front for instant access.

This app maintains a backend websocket connection to the utility, when that socket is closed, it's assumed
that the utility has exited, at which point this app will terminate cleanly.

## Support
This is relatively unsupported, and is primarily a proof of concept, please treat it as such :)

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
