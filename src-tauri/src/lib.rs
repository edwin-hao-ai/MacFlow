pub mod app_scanner;
pub mod applications;
pub mod cache_cleaner;
pub mod cache_scanner;
pub mod dev_tool_rules;
pub mod docker;
pub mod monitor;
pub mod ports;
pub mod process_ops;
pub mod process_safety;
pub mod residue_scanner;
pub mod scanner;
pub mod storage;
pub mod tray;
pub mod uninstaller;
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
use residue_scanner::AppResidue;
use scanner::{ScanResult, SystemHealth};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use storage::{HistoryEntry, Storage, WhitelistEntry};
use sysinfo::System;
use tauri::{Manager, State, WindowEvent};
use uninstaller::{UninstallReport, UninstallTarget};

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

/// 列出所有可见用户进程（不做分类过滤，用于进程管理页）。
/// 与 scan_all 不同：返回全部，前端自己做展示/搜索/排序。
#[tauri::command]
async fn list_all_processes(state: State<'_, AppState>) -> Result<Vec<scanner::ProcessRow>, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    let mut rows = scanner::list_all(&mut sys);
    for row in &mut rows {
        if state.storage.is_whitelisted("process", &row.name) {
            row.whitelisted = true;
            row.protected = true;
            row.protected_reason = Some("命中白名单，默认不建议终止".into());
        }
    }
    Ok(rows)
}

// ========== 应用程序管理 ==========

#[tauri::command]
async fn list_applications(
    state: State<'_, AppState>,
) -> Result<Vec<applications::AppInfo>, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    let mut apps = applications::list_running_apps(&mut sys);
    for app in &mut apps {
        let mut protected_count = 0usize;
        let mut whitelisted_count = 0usize;
        for child in &mut app.children {
            if state.storage.is_whitelisted("process", &child.name) {
                child.whitelisted = true;
                child.protected = true;
                child.protected_reason = Some("命中白名单，默认不建议终止".into());
            }
            if child.protected {
                protected_count += 1;
            }
            if child.whitelisted {
                whitelisted_count += 1;
            }
        }
        app.protected_process_count = protected_count;
        app.whitelisted_process_count = whitelisted_count;
    }
    Ok(apps)
}

#[tauri::command]
async fn quit_application(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let result = applications::graceful_quit_app(&name).await;
    // 记录历史：成功 / 失败都写一条
    let _ = state.storage.log_history(
        "app_quit",
        &name,
        0,
        result.is_ok(),
        &result.as_ref().err().cloned().unwrap_or_else(|| "已发送退出信号".into()),
    );
    result
}

#[tauri::command]
async fn force_quit_application(
    state: State<'_, AppState>,
    name: String,
    pids: Vec<u32>,
) -> Result<Vec<(u32, String)>, String> {
    let results = applications::force_quit_app(&pids);
    let killed_count = results.iter().filter(|(_, o)| o.is_ok()).count();
    let total = results.len();
    let success = killed_count == total;
    let detail = format!("终止 {}/{} 个进程", killed_count, total);
    let _ = state.storage.log_history(
        "app_force_quit",
        &name,
        0,
        success,
        &detail,
    );
    Ok(results
        .into_iter()
        .map(|(pid, outcome)| (pid, outcome.message()))
        .collect())
}

// ========== Docker 深度视图 ==========

#[tauri::command]
async fn docker_available() -> Result<bool, String> {
    Ok(docker::is_available().await)
}

#[tauri::command]
async fn docker_inventory() -> Result<docker::DockerInventory, String> {
    docker::inventory().await
}

#[tauri::command]
async fn docker_remove_image(id: String) -> Result<(), String> {
    docker::remove_image(&id).await
}

#[tauri::command]
async fn docker_remove_container(id: String) -> Result<(), String> {
    docker::remove_container(&id).await
}

#[tauri::command]
async fn docker_remove_volume(name: String) -> Result<(), String> {
    docker::remove_volume(&name).await
}

#[tauri::command]
async fn docker_prune_all() -> Result<String, String> {
    docker::prune_all().await
}

#[derive(Serialize)]
pub struct KillResult {
    pub pid: u32,
    pub name: String,
    pub success: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct KillReport {
    pub killed: Vec<u32>,
    pub failed: Vec<u32>,
    pub details: Vec<KillResult>,
}

#[tauri::command]
async fn kill_processes(
    state: State<'_, AppState>,
    pids: Vec<u32>,
    names: Vec<String>,
) -> Result<KillReport, String> {
    let mut killed = Vec::new();
    let mut failed = Vec::new();
    let mut details = Vec::new();

    for (idx, pid) in pids.iter().enumerate() {
        let name = names.get(idx).cloned().unwrap_or_default();
        let outcome = process_ops::graceful_kill(*pid);
        let msg = outcome.message();
        let ok = outcome.is_ok();

        if ok {
            killed.push(*pid);
        } else {
            failed.push(*pid);
        }
        details.push(KillResult {
            pid: *pid,
            name: name.clone(),
            success: ok,
            message: msg.clone(),
        });

        let _ = state.storage.log_history(
            "process_kill",
            &format!("{} (PID {})", name, pid),
            0,
            ok,
            &msg,
        );
    }
    Ok(KillReport {
        killed,
        failed,
        details,
    })
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

// ========== 应用卸载 ==========

/// 扫描已安装应用列表
#[tauri::command]
async fn scan_installed_apps(state: State<'_, AppState>) -> Result<Vec<app_scanner::InstalledApp>, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    Ok(app_scanner::scan_installed_apps(&mut sys))
}

/// 扫描指定应用的残留文件
#[tauri::command]
async fn scan_app_residues(bundle_id: String, app_name: String) -> Result<AppResidue, String> {
    Ok(residue_scanner::scan_residues(&bundle_id, &app_name))
}

/// 批量卸载应用（移至废纸篓）
#[tauri::command]
async fn uninstall_apps(
    state: State<'_, AppState>,
    targets: Vec<UninstallTarget>,
) -> Result<Vec<UninstallReport>, String> {
    let mut reports = Vec::new();
    for target in &targets {
        let report = uninstaller::uninstall_app(target).await;
        // 记录卸载历史
        let detail = serde_json::json!({
            "moved": report.moved_count,
            "failed": report.failed_count,
        })
        .to_string();
        let _ = state.storage.log_history(
            "app_uninstall",
            &format!("{} ({})", report.app_name, report.bundle_id),
            report.total_freed_bytes,
            report.failed_count == 0,
            &detail,
        );
        reports.push(report);
    }
    Ok(reports)
}

/// 检查应用是否正在运行
#[tauri::command]
async fn check_app_running(
    state: State<'_, AppState>,
    bundle_path: String,
) -> Result<bool, String> {
    let mut sys = state.sys.lock().map_err(|e| e.to_string())?;
    Ok(uninstaller::is_app_running(&bundle_path, &mut sys))
}

/// 退出应用并执行卸载
#[tauri::command]
async fn quit_and_uninstall(
    state: State<'_, AppState>,
    app_name: String,
    target: UninstallTarget,
) -> Result<UninstallReport, String> {
    let report = uninstaller::quit_and_uninstall(&app_name, &target).await?;
    // 记录卸载历史
    let detail = serde_json::json!({
        "moved": report.moved_count,
        "failed": report.failed_count,
    })
    .to_string();
    let _ = state.storage.log_history(
        "app_uninstall",
        &format!("{} ({})", report.app_name, report.bundle_id),
        report.total_freed_bytes,
        report.failed_count == 0,
        &detail,
    );
    Ok(report)
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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
        .on_window_event(|window, event| {
            // 点 X 关闭 → 不退出应用，只把窗口藏起来，托盘保持驻留
            // 真正退出通过托盘菜单「退出 MacSlim」
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_health,
            scan_all,
            list_all_processes,
            kill_processes,
            scan_cache,
            clean_cache,
            get_history,
            get_whitelist,
            add_whitelist,
            remove_whitelist,
            list_applications,
            quit_application,
            force_quit_application,
            docker_available,
            docker_inventory,
            docker_remove_image,
            docker_remove_container,
            docker_remove_volume,
            docker_prune_all,
            scan_installed_apps,
            scan_app_residues,
            uninstall_apps,
            check_app_running,
            quit_and_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
