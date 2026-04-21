pub mod cache_cleaner;
pub mod cache_scanner;
pub mod monitor;
pub mod process_ops;
pub mod scanner;
pub mod storage;
pub mod tray;
pub mod whitelist;

// CLI-friendly re-exports
pub use cache_cleaner::clean as cache_cleaner_clean;
pub use cache_scanner::scan as cache_scanner_scan;
pub use scanner::read_health as scanner_read_health;
pub fn run_tauri() {
    run();
}

use cache_cleaner::CleanSummary;
use cache_scanner::{CacheItem, CacheScanResult};
use scanner::{ScanResult, SystemHealth};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use storage::{HistoryEntry, Storage, WhitelistEntry};
use sysinfo::System;
use tauri::{Manager, State};

pub struct AppState {
    pub sys: Mutex<System>,
    pub storage: Arc<Storage>,
}

// ========== System & Process ==========

#[tauri::command]
async fn get_system_health(state: State<'_, AppState>) -> Result<SystemHealth, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    Ok(scanner::read_health(&mut sys))
}

#[tauri::command]
async fn scan_all(state: State<'_, AppState>) -> Result<ScanResult, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    let mut result = scanner::scan(&mut sys);
    // 叠加用户自定义白名单
    let storage = state.storage.clone();
    result
        .processes
        .retain(|p| !storage.is_whitelisted("process", &p.name));
    Ok(result)
}

#[derive(Serialize)]
pub struct KillReport {
    pub killed: Vec<u32>,
    pub failed: Vec<u32>,
}

#[tauri::command]
async fn kill_processes(
    state: State<'_, AppState>,
    pids: Vec<u32>,
    names: Vec<String>,
) -> Result<KillReport, String> {
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    for (idx, pid) in pids.iter().enumerate() {
        let name = names.get(idx).cloned().unwrap_or_default();
        if process_ops::graceful_kill(*pid) {
            killed.push(*pid);
            let _ = state.storage.log_history(
                "process_kill",
                &format!("{} (PID {})", name, pid),
                0,
                true,
                "优雅终止成功",
            );
        } else {
            failed.push(*pid);
            let _ = state.storage.log_history(
                "process_kill",
                &format!("{} (PID {})", name, pid),
                0,
                false,
                "终止失败，可能是受保护进程",
            );
        }
    }
    Ok(KillReport { killed, failed })
}

// ========== Cache ==========

#[tauri::command]
async fn scan_cache() -> Result<CacheScanResult, String> {
    Ok(cache_scanner::scan().await)
}

#[tauri::command]
async fn clean_cache(
    state: State<'_, AppState>,
    items: Vec<CacheItem>,
) -> Result<CleanSummary, String> {
    let summary = cache_cleaner::clean(items).await;
    for r in &summary.reports {
        let _ = state.storage.log_history(
            "cache_clean",
            &r.label,
            r.freed_bytes,
            r.success,
            &r.error.clone().unwrap_or_else(|| "已清理".into()),
        );
    }
    Ok(summary)
}

// ========== History & Whitelist ==========

#[tauri::command]
async fn get_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<HistoryEntry>, String> {
    state.storage.recent_history(limit.unwrap_or(200))
}

#[tauri::command]
async fn get_whitelist(state: State<'_, AppState>) -> Result<Vec<WhitelistEntry>, String> {
    state.storage.list_whitelist()
}

#[tauri::command]
async fn add_whitelist(
    state: State<'_, AppState>,
    kind: String,
    value: String,
    note: String,
) -> Result<(), String> {
    state.storage.add_whitelist(&kind, &value, &note)
}

#[tauri::command]
async fn remove_whitelist(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.storage.remove_whitelist(id)
}

// ========== Entry point ==========

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let storage = Arc::new(Storage::open().expect("无法初始化存储"));

    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup({
            let storage = storage.clone();
            move |app| {
                let mut sys = System::new_all();
                sys.refresh_all();
                app.manage(AppState {
                    sys: Mutex::new(sys),
                    storage: storage.clone(),
                });

                // macOS 毛玻璃
                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{
                        apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
                    };
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = apply_vibrancy(
                            &window,
                            NSVisualEffectMaterial::Sidebar,
                            Some(NSVisualEffectState::Active),
                            Some(12.0),
                        );
                    }
                }

                // 系统托盘
                tray::init_tray(app.handle())?;

                // 后台健康监控（2 秒一次）
                monitor::start_background_monitor(app.handle().clone());

                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_health,
            scan_all,
            kill_processes,
            scan_cache,
            clean_cache,
            get_history,
            get_whitelist,
            add_whitelist,
            remove_whitelist,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
