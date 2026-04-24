// 卸载执行器：将应用和残留文件移至废纸篓
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 卸载目标
#[derive(Deserialize, Clone, Debug)]
pub struct UninstallTarget {
    pub bundle_path: String,
    pub app_name: String,
    pub bundle_id: String,
    pub residue_paths: Vec<String>,
}

/// 单个文件的移动结果
#[derive(Serialize, Clone, Debug)]
pub struct MoveResult {
    pub path: String,
    pub success: bool,
    pub error: Option<String>,
    pub size_bytes: u64,
}

/// 卸载报告
#[derive(Serialize, Clone, Debug)]
pub struct UninstallReport {
    pub app_name: String,
    pub bundle_id: String,
    pub total_freed_bytes: u64,
    pub moved_count: usize,
    pub failed_count: usize,
    pub details: Vec<MoveResult>,
}

/// 执行卸载（移至废纸篓）
pub async fn uninstall_app(target: &UninstallTarget) -> UninstallReport {
    let mut details = Vec::new();

    // 先移动 .app bundle 本体
    let bundle_result = trash_item(&target.bundle_path).await;
    details.push(bundle_result);

    // 再移动所有选中的残留文件
    for path in &target.residue_paths {
        let result = trash_item(path).await;
        details.push(result);
    }

    build_report(target, details)
}

/// 将单个文件/目录移至废纸篓
/// 优先使用 osascript 调用 NSFileManager.trashItem
/// 备选方案：直接 rename 到 ~/.Trash/
async fn trash_item(path_str: &str) -> MoveResult {
    let path = Path::new(path_str);
    let size = compute_size(path);

    if !path.exists() {
        return MoveResult {
            path: path_str.to_string(),
            success: false,
            error: Some("文件不存在".to_string()),
            size_bytes: 0,
        };
    }

    // 优先尝试 osascript（NSFileManager.trashItem）
    match trash_via_osascript(path_str).await {
        Ok(()) => {
            return MoveResult {
                path: path_str.to_string(),
                success: true,
                error: None,
                size_bytes: size,
            };
        }
        Err(_) => {
            // 备选方案：直接 rename 到 ~/.Trash/
            return trash_via_rename(path_str, size);
        }
    }
}

/// 通过 osascript 调用 Finder 移至废纸篓
async fn trash_via_osascript(path_str: &str) -> Result<(), String> {
    let escaped = path_str.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"use framework "Foundation"
set fm to current application's NSFileManager's defaultManager()
set theURL to current application's NSURL's fileURLWithPath:"{}"
set {{result_, theError}} to fm's trashItemAtURL:theURL resultingItemURL:(missing value) |error|:(reference)
if result_ as boolean is false then
    error (theError's localizedDescription() as text)
end if"#,
        escaped
    );

    let output = tokio::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
        .map_err(|e| format!("启动 osascript 失败: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

/// 备选方案：直接 rename 到 ~/.Trash/
fn trash_via_rename(path_str: &str, size: u64) -> MoveResult {
    let path = Path::new(path_str);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let trash_dir = match dirs::home_dir() {
        Some(h) => h.join(".Trash"),
        None => {
            return MoveResult {
                path: path_str.to_string(),
                success: false,
                error: Some("无法获取用户主目录".to_string()),
                size_bytes: 0,
            };
        }
    };

    let dest = trash_dir.join(&file_name);
    match std::fs::rename(path, &dest) {
        Ok(()) => MoveResult {
            path: path_str.to_string(),
            success: true,
            error: None,
            size_bytes: size,
        },
        Err(e) => MoveResult {
            path: path_str.to_string(),
            success: false,
            error: Some(format!("移动失败: {}", e)),
            size_bytes: 0,
        },
    }
}

/// 计算文件或目录大小
fn compute_size(path: &Path) -> u64 {
    if path.is_dir() {
        crate::app_scanner::dir_size(path)
    } else {
        path.metadata().map(|m| m.len()).unwrap_or(0)
    }
}

/// 从移动结果列表构建卸载报告
fn build_report(target: &UninstallTarget, details: Vec<MoveResult>) -> UninstallReport {
    let moved_count = details.iter().filter(|d| d.success).count();
    let failed_count = details.iter().filter(|d| !d.success).count();
    let total_freed_bytes: u64 = details.iter().map(|d| d.size_bytes).sum();

    UninstallReport {
        app_name: target.app_name.clone(),
        bundle_id: target.bundle_id.clone(),
        total_freed_bytes,
        moved_count,
        failed_count,
        details,
    }
}

/// 检查应用是否正在运行（通过 bundle 路径匹配进程）
pub fn is_app_running(bundle_path: &str, sys: &mut sysinfo::System) -> bool {
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (_pid, proc) in sys.processes() {
        let Some(exe) = proc.exe() else { continue };
        let exe_str = exe.to_string_lossy();
        if exe_str.starts_with(bundle_path) {
            return true;
        }
    }
    false
}

/// 优雅退出应用并等待最多 5 秒，然后执行卸载
pub async fn quit_and_uninstall(
    app_name: &str,
    target: &UninstallTarget,
) -> Result<UninstallReport, String> {
    // 发送优雅退出信号
    let escaped_name = app_name.replace('"', "\\\"");
    let script = format!(r#"tell application "{}" to quit"#, escaped_name);
    let _ = tokio::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await;

    // 等待最多 5 秒让应用退出
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // 检查 .app bundle 内的进程是否还在
        let check_script = format!(
            r#"tell application "System Events" to (name of processes) contains "{}""#,
            escaped_name
        );
        let output = tokio::process::Command::new("osascript")
            .args(["-e", &check_script])
            .output()
            .await;
        if let Ok(out) = output {
            let result = String::from_utf8_lossy(&out.stdout);
            if result.trim() == "false" {
                break;
            }
        }
    }

    Ok(uninstall_app(target).await)
}
