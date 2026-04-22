//! 应用程序管理 —— 把进程按 `.app` bundle 聚合，展示用户视角的「运行中的应用」
//!
//! macOS 的进程和应用是分离的：一个 Chrome「应用」可能对应 20+ 个进程
//! （main + helper + renderer + gpu + plugin ...）。用户看活动监视器只关心
//! 「我开了哪些应用、各占多少内存」，这个模块就做这个。

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use sysinfo::System;

#[derive(Serialize, Clone, Debug)]
pub struct AppInfo {
    /// 应用 bundle 路径，如 /Applications/Safari.app
    pub bundle_path: String,
    /// 显示名（从 Info.plist 或从路径推断）
    pub name: String,
    /// bundle id，如 com.apple.Safari（可能为空）
    pub bundle_id: String,
    /// 主进程 PID（通常是路径最短的那个）
    pub main_pid: u32,
    /// 所有相关进程（主进程 + helper）
    pub all_pids: Vec<u32>,
    /// 总内存（所有相关进程求和，单位 MB）
    pub memory_mb: f64,
    /// 总 CPU（所有相关进程求和）
    pub cpu_percent: f32,
    /// 运行时长（秒，取最早启动的那个进程）
    pub uptime_secs: u64,
    /// 监听的端口（如果有）
    pub ports: Vec<u16>,
    /// 是否为系统应用（位于 /System / /Library / 等）
    pub is_system: bool,
}

/// 列出所有运行中的 .app 应用
pub fn list_running_apps(sys: &mut System) -> Vec<AppInfo> {
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(150));
    sys.refresh_cpu_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    // Step 1: 按 bundle path 分组进程
    let mut groups: HashMap<String, Vec<(u32, &sysinfo::Process)>> = HashMap::new();
    for (pid, proc) in sys.processes() {
        // 只看当前用户的（跨用户的 root 服务属于系统，前端不关心）
        if !crate::process_safety::is_same_user(proc) {
            continue;
        }
        let exe = match proc.exe() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        // 提取 .app bundle 路径：
        //   exe 形如 /Applications/Safari.app/Contents/MacOS/Safari
        //   找第一个 .app 结尾的祖先目录
        let bundle = match find_app_bundle(&exe) {
            Some(b) => b,
            None => continue,
        };
        let key = bundle.to_string_lossy().to_string();
        groups.entry(key).or_default().push((pid.as_u32(), proc));
    }

    // Step 2: 为每组生成 AppInfo
    let mut apps: Vec<AppInfo> = groups
        .into_iter()
        .filter_map(|(bundle_path, procs)| build_app_info(&bundle_path, &procs, sys))
        .collect();

    // 端口注入
    let all_pids: Vec<u32> = apps.iter().flat_map(|a| a.all_pids.clone()).collect();
    let port_map = crate::ports::ports_by_pid(&all_pids);
    for app in apps.iter_mut() {
        let mut ports: Vec<u16> = Vec::new();
        for pid in &app.all_pids {
            if let Some(list) = port_map.get(pid) {
                ports.extend(list.iter().copied());
            }
        }
        ports.sort();
        ports.dedup();
        app.ports = ports;
    }

    // 按内存降序
    apps.sort_by(|a, b| {
        b.memory_mb
            .partial_cmp(&a.memory_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    apps
}

/// 从 exe 路径往上找 .app 结尾的目录
fn find_app_bundle(exe: &std::path::Path) -> Option<PathBuf> {
    for ancestor in exe.ancestors() {
        if let Some(name) = ancestor.file_name() {
            if name.to_string_lossy().ends_with(".app") {
                return Some(ancestor.to_path_buf());
            }
        }
    }
    None
}

fn build_app_info(
    bundle_path: &str,
    procs: &[(u32, &sysinfo::Process)],
    _sys: &System,
) -> Option<AppInfo> {
    if procs.is_empty() {
        return None;
    }
    let bundle = PathBuf::from(bundle_path);
    let info_plist = bundle.join("Contents/Info.plist");

    let (name_from_plist, bundle_id) = read_plist_metadata(&info_plist);

    // 从路径推断名字：/Applications/Safari.app → Safari
    let name_from_path = bundle
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".into());
    let name = name_from_plist.unwrap_or(name_from_path);

    // 主进程：路径最短（最接近 MacOS/X 的）或 PID 最小
    let main = procs
        .iter()
        .min_by_key(|(pid, proc)| {
            let path_len = proc
                .exe()
                .map(|p| p.to_string_lossy().len())
                .unwrap_or(usize::MAX);
            (path_len, *pid)
        })
        .cloned();

    let (main_pid, _main_proc) = main?;

    let all_pids: Vec<u32> = procs.iter().map(|(p, _)| *p).collect();
    let memory_mb: f64 = procs
        .iter()
        .map(|(_, p)| p.memory() as f64 / 1024.0 / 1024.0)
        .sum();
    let cpu_percent: f32 = procs.iter().map(|(_, p)| p.cpu_usage()).sum();

    // 取最早启动的 start_time（最老的 uptime）
    let earliest_start = procs
        .iter()
        .map(|(_, p)| p.start_time())
        .min()
        .unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let uptime_secs = now.saturating_sub(earliest_start);

    let is_system = bundle_path.starts_with("/System/")
        || bundle_path.starts_with("/Library/CoreServices/")
        || bundle_path.starts_with("/usr/libexec/");

    Some(AppInfo {
        bundle_path: bundle_path.to_string(),
        name,
        bundle_id: bundle_id.unwrap_or_default(),
        main_pid,
        all_pids,
        memory_mb,
        cpu_percent,
        uptime_secs,
        ports: Vec::new(),
        is_system,
    })
}

/// 读 .app/Contents/Info.plist，提取 CFBundleName / CFBundleIdentifier
/// 用最朴素的文本解析（Info.plist 多是 XML 格式；二进制 plist 我们不解析，
/// 返回 None 由 caller 走路径推断兜底）
fn read_plist_metadata(path: &std::path::Path) -> (Option<String>, Option<String>) {
    let Ok(bytes) = std::fs::read(path) else {
        return (None, None);
    };
    // 快速判别：二进制 plist 以 "bplist" 开头
    if bytes.starts_with(b"bplist") {
        // 二进制 plist 不解析，回退
        return (None, None);
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return (None, None);
    };
    let name = extract_plist_string(text, "CFBundleDisplayName")
        .or_else(|| extract_plist_string(text, "CFBundleName"));
    let id = extract_plist_string(text, "CFBundleIdentifier");
    (name, id)
}

fn extract_plist_string(text: &str, key: &str) -> Option<String> {
    // 找 <key>KEY</key>...<string>VALUE</string>
    let key_tag = format!("<key>{}</key>", key);
    let idx = text.find(&key_tag)?;
    let rest = &text[idx + key_tag.len()..];
    let start = rest.find("<string>")? + "<string>".len();
    let end = rest[start..].find("</string>")?;
    let val = rest[start..start + end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

/// 优雅退出应用 —— 用 osascript 发 `tell application "X" to quit`
/// 这会触发 macOS 的标准退出流程（保存未存文件、确认对话框等）
pub async fn graceful_quit_app(bundle_name: &str) -> Result<(), String> {
    // 反引号保护防止名字里有空格或特殊字符
    let script = format!(
        r#"tell application "{}" to quit"#,
        bundle_name.replace('"', "\\\"")
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

/// 强制退出：对 app 的所有 PID 调 graceful_kill（会走进程树清理）
pub fn force_quit_app(all_pids: &[u32]) -> Vec<(u32, crate::process_ops::KillOutcome)> {
    all_pids
        .iter()
        .map(|pid| (*pid, crate::process_ops::graceful_kill(*pid)))
        .collect()
}
