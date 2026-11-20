//! Desktop app shell. It only wires `adv-session` to the window; it contains no format code.

// The Windows build must not open a console window next to the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    commands::run();
}
