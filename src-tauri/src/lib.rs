mod scanner;
mod process_ops;
mod whitelist;

use scanner::{ScanResult, SystemHealth};
use std::sync::Mutex;
use sysinfo::System;
use tauri::{Manager, State};

pub struct AppState {
    pub sys: Mutex<System>,
}

#[tauri::command]
async fn get_system_health(state: State<'_, AppState>) -> Result<SystemHealth, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    Ok(scanner::read_health(&mut sys))
}

#[tauri::command]
async fn scan_all(state: State<'_, AppState>) -> Result<ScanResult, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    Ok(scanner::scan(&mut sys))
}

#[derive(serde::Serialize)]
pub struct KillReport {
    pub killed: Vec<u32>,
    pub failed: Vec<u32>,
}

#[tauri::command]
async fn kill_processes(pids: Vec<u32>) -> Result<KillReport, String> {
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    for pid in pids {
        if process_ops::graceful_kill(pid) {
            killed.push(pid);
        } else {
            failed.push(pid);
        }
    }
    Ok(KillReport { killed, failed })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let mut sys = System::new_all();
            sys.refresh_all();
            app.manage(AppState {
                sys: Mutex::new(sys),
            });

            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                if let Some(window) = app.get_webview_window("main") {
                    let _ = apply_vibrancy(
                        &window,
                        NSVisualEffectMaterial::Sidebar,
                        Some(NSVisualEffectState::Active),
                        Some(12.0),
                    );
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_health,
            scan_all,
            kill_processes
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
