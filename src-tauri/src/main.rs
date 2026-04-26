// Prevents additional console window on Windows in release, does nothing on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    macslim_lib::run()
}
