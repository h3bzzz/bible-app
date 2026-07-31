//! The desktop binary.
//!
//! A thin wrapper: everything — the state, the commands, the wiring — lives in
//! `run` in the library, which the mobile targets link against too. The only
//! reason this file exists is that a desktop OS launches an executable, and an
//! executable needs a `main`.

// On Windows, a release build should not pop a console window behind the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    bible_desktop_lib::run();
}
