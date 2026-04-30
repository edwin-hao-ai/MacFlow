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
///
/// 流程：
/// 1. 先尝试 NSFileManager.trashItem（用户权限）
/// 2. 失败后尝试 rename 到 ~/.Trash/
/// 3. 如因权限不足失败（如 /Applications/ 下的 app），收集起来在最后用
///    `do shell script with administrator privileges` 一次性弹出系统授权框
///    批量移动，避免多次重复弹窗
pub async fn uninstall_app(target: &UninstallTarget) -> UninstallReport {
    let mut details: Vec<MoveResult> = Vec::new();
    let mut needs_admin: Vec<(String, u64)> = Vec::new();

    let mut all_paths: Vec<String> = Vec::with_capacity(1 + target.residue_paths.len());
    all_paths.push(target.bundle_path.clone());
    all_paths.extend(target.residue_paths.iter().cloned());

    for path_str in &all_paths {
        match try_trash_user(path_str).await {
            TrashOutcome::Done(result) => details.push(result),
            TrashOutcome::NeedsAdmin { size } => {
                needs_admin.push((path_str.clone(), size));
            }
        }
    }

    if !needs_admin.is_empty() {
        let paths: Vec<&str> = needs_admin.iter().map(|(p, _)| p.as_str()).collect();
        match trash_via_admin_batch(&paths).await {
            Ok(()) => {
                for (path, size) in needs_admin {
                    details.push(MoveResult {
                        path,
                        success: true,
                        error: None,
                        size_bytes: size,
                    });
                }
            }
            Err(e) => {
                let msg = if is_user_canceled(&e) {
                    "用户取消授权".to_string()
                } else {
                    format!("授权移动失败: {}", e)
                };
                for (path, _) in needs_admin {
                    details.push(MoveResult {
                        path,
                        success: false,
                        error: Some(msg.clone()),
                        size_bytes: 0,
                    });
                }
            }
        }
    }

    build_report(target, details)
}

enum TrashOutcome {
    Done(MoveResult),
    NeedsAdmin { size: u64 },
}

/// 用户权限尝试：osascript NSFileManager → rename。
/// 如全部因权限不足失败，则返回 NeedsAdmin 让上层批量授权处理。
async fn try_trash_user(path_str: &str) -> TrashOutcome {
    let path = Path::new(path_str);
    let size = compute_size(path);

    if !path.exists() {
        return TrashOutcome::Done(MoveResult {
            path: path_str.to_string(),
            success: false,
            error: Some("文件不存在".to_string()),
            size_bytes: 0,
        });
    }

    // 1. NSFileManager.trashItem
    match trash_via_osascript(path_str).await {
        Ok(()) => {
            return TrashOutcome::Done(MoveResult {
                path: path_str.to_string(),
                success: true,
                error: None,
                size_bytes: size,
            });
        }
        Err(e) if is_permission_denied_msg(&e) => {
            // 权限问题，先不返回失败，继续尝试 rename
        }
        Err(_) => {
            // 非权限错误也继续 rename 试一下
        }
    }

    // 2. rename 到 ~/.Trash/
    match try_rename_to_trash(path_str) {
        Ok(()) => TrashOutcome::Done(MoveResult {
            path: path_str.to_string(),
            success: true,
            error: None,
            size_bytes: size,
        }),
        Err(e) if is_permission_denied_io(&e) => TrashOutcome::NeedsAdmin { size },
        Err(e) => TrashOutcome::Done(MoveResult {
            path: path_str.to_string(),
            success: false,
            error: Some(format!("移动失败: {}", e)),
            size_bytes: 0,
        }),
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
fn try_rename_to_trash(path_str: &str) -> Result<(), std::io::Error> {
    let path = Path::new(path_str);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let trash_dir = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "无法获取用户主目录"))?
        .join(".Trash");

    let dest = trash_dir.join(&file_name);
    std::fs::rename(path, &dest)
}

/// 用 `do shell script ... with administrator privileges` 弹出系统授权框，
/// 一次输入密码即可批量将多个路径移动到 ~/.Trash/。
async fn trash_via_admin_batch(paths: &[&str]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let trash_dir = dirs::home_dir()
        .ok_or_else(|| "无法获取用户主目录".to_string())?
        .join(".Trash");
    let trash_str = trash_dir.to_string_lossy().to_string();

    // 拼接成 shell 命令：mv -f 'path1' 'trash/' ; mv -f 'path2' 'trash/' ; ...
    // 用 `;` 而不是 `&&` —— 单个失败不影响其他
    let mut shell_cmd = String::new();
    for (i, p) in paths.iter().enumerate() {
        if i > 0 {
            shell_cmd.push_str(" ; ");
        }
        shell_cmd.push_str(&format!(
            "/bin/mv -f {} {}",
            shell_single_quote(p),
            shell_single_quote(&trash_str)
        ));
    }

    // 转义为 AppleScript 字符串字面量
    let as_escaped = applescript_quote(&shell_cmd);
    let script = format!(
        r#"do shell script {} with administrator privileges"#,
        as_escaped
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

/// 用单引号包裹 shell 参数，路径中若有 `'` 替换为 `'\''`
fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 用双引号包裹 AppleScript 字符串字面量，转义 `\` 和 `"`
fn applescript_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn is_permission_denied_io(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(13)
}

fn is_permission_denied_msg(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("permission")
        || lower.contains("operation not permitted")
        || lower.contains("not authorized")
        || lower.contains("nscocoaerrordomain error 513")
        || lower.contains("nscocoaerrordomain error 257")
}

fn is_user_canceled(msg: &str) -> bool {
    // osascript 在用户取消授权时返回 errAEEventNotPermitted (-1743) 或 -128
    msg.contains("-128") || msg.contains("User canceled") || msg.contains("用户已取消")
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
