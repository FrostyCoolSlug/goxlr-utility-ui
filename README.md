# GoXLR Utility UI Wrapper
This is a simple app that wraps the GoXLR Utilities' UI into an independent browser based on Tauri.

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