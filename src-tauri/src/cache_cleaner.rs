use crate::cache_scanner::{is_any_tool_busy, CacheItem};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 允许被直接删除的路径前缀白名单。
/// 任何 remove_directory 调用必须命中其中一个，否则拒绝执行。
/// 这是防止 bug 或恶意输入导致误删家目录 / 系统目录的最后一道防线。
fn allowed_cleanup_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        // 只允许这几个非常具体的缓存子路径
        out.push(home.join(".npm"));
        out.push(home.join(".cargo/registry/cache"));
        out.push(home.join(".Trash"));
        out.push(home.join("Library/pnpm/store"));
        out.push(home.join("Library/Caches")); // 覆盖所有应用缓存子目录
        out.push(home.join("Library/Logs")); // 应用日志
        out.push(home.join("Library/Developer/Xcode/DerivedData"));
        out.push(home.join("Library/Developer/Xcode/Archives"));
        out.push(home.join("Library/Developer/Xcode/iOS DeviceSupport"));
        out.push(home.join("Library/Developer/CoreSimulator/Caches"));
        out.push(home.join("Library/Application Support/CrashReporter"));
    }
    out.push(PathBuf::from("/opt/homebrew/Library/Homebrew/cache"));
    out
}

/// 检查给定路径是否位于允许清理的白名单下。
/// 使用规范化路径比对，防止 `..` 或符号链接绕过。
fn is_cleanup_path_allowed(path: &Path) -> bool {
    // 规范化为绝对路径
    let canon = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return false, // 路径不存在或读不到 → 拒绝
    };

    // 额外防御：拒绝明显危险的目录
    let dangerous_literal = [
        "/",
        "/usr",
        "/etc",
        "/var",
        "/bin",
        "/sbin",
        "/System",
        "/Library",
        "/Applications",
        "/private",
    ];
    if let Some(s) = canon.to_str() {
        if dangerous_literal.iter().any(|d| s == *d) {
            return false;
        }
        // 绝对不能是 home 根
        if let Some(home) = dirs::home_dir() {
            if Path::new(s) == home {
                return false;
            }
        }
    }

    for root in allowed_cleanup_roots() {
        if let Ok(root_canon) = root.canonicalize() {
            if canon.starts_with(&root_canon) {
                return true;
            }
        } else {
            // 白名单路径不存在 → 跟目标比较前缀仍有效（对 starts_with 是按路径组件）
            if canon.starts_with(&root) {
                return true;
            }
        }
    }
    false
}

/// 根据 item 判断清理前需要什么工具没在跑。返回 Some(tool) 表示被占用。
fn busy_check_for(item: &CacheItem) -> Option<String> {
    use crate::cache_scanner::CacheCategory::*;
    let tools: &[&str] = match item.category {
        Npm => &["npm", "npx"],
        Pnpm => &["pnpm"],
        Yarn => &["yarn"],
        Docker => &[], // Docker daemon 本身在跑是正常的
        Homebrew => &["brew"],
        Xcode => &["Xcode", "xcodebuild", "xcrun", "swift-frontend", "clang"],
        Cocoapods => &["pod"],
        Cargo => &["cargo", "rustc", "rustup"],
        Pip => &["pip", "pip3"],
        Go => &["go", "gopls"],
        System => &[],
    };
    if tools.is_empty() {
        return None;
    }
    is_any_tool_busy(tools)
}

#[derive(Serialize, Clone, Debug)]
pub struct CleanReport {
    pub id: String,
    pub label: String,
    pub success: bool,
    pub freed_bytes: u64,
    pub duration_ms: u64,
    pub command: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct CleanSummary {
    pub reports: Vec<CleanReport>,
    pub total_freed_bytes: u64,
    pub success_count: usize,
    pub fail_count: usize,
}

pub async fn clean(items: Vec<CacheItem>) -> CleanSummary {
    let mut reports = Vec::with_capacity(items.len());

    for item in items {
        let start = Instant::now();
        let before = item.size_bytes;

        // 双重保护：清理前再次检测工具是否在用
        let result = if let Some(busy) = busy_check_for(&item) {
            Err(format!(
                "检测到 {} 正在运行，已跳过清理以防止损坏",
                busy
            ))
        } else if item.command.as_deref() == Some("__PIP_CLEAN__") {
            // Pip 缓存清理特殊处理：绕开坏 shebang 的 pip 脚本
            clean_pip_cache(&item).await
        } else if let Some(cmd) = item
            .command
            .as_deref()
            .and_then(|s| s.strip_prefix("__STALE_NODE_MODULES_CLEAN__:"))
        {
            clean_stale_node_modules(cmd).await
        } else if let Some(cmd) = &item.command {
            // npm / pnpm / yarn 特殊处理：命令失败时回退到直接删目录
            // 原因：nvm 管理的 npm 需要 nvm 初始化才能激活，login shell 不一定能找到
            let fallback_ids = ["npm-cache", "pnpm-store", "yarn-cache"];
            let cmd_result = run_shell_command(cmd).await;
            if cmd_result.is_err() && fallback_ids.contains(&item.id.as_str()) {
                if let Some(path) = &item.path {
                    let p = expand_tilde(path);
                    if is_cleanup_path_allowed(&p) {
                        remove_directory(&p).await
                    } else {
                        cmd_result
                    }
                } else {
                    cmd_result
                }
            } else {
                cmd_result
            }
        } else if let Some(path) = &item.path {
            let p = expand_tilde(path);
            if !is_cleanup_path_allowed(&p) {
                Err(format!(
                    "路径不在白名单内，拒绝删除: {}",
                    p.display()
                ))
            } else {
                // 检查是否是 root 所有的目录，需要 sudo
                let needs_sudo = p.metadata()
                    .map(|m| {
                        use std::os::unix::fs::MetadataExt;
                        m.uid() == 0
                    })
                    .unwrap_or(false);
                if needs_sudo {
                    remove_directory_sudo(&p).await
                } else {
                    remove_directory(&p).await
                }
            }
        } else {
            Err("既无命令也无路径".into())
        };

        let (success, error) = match result {
            Ok(_) => (true, None),
            Err(e) => (false, Some(e)),
        };

        reports.push(CleanReport {
            id: item.id.clone(),
            label: item.label.clone(),
            success,
            freed_bytes: if success { before } else { 0 },
            duration_ms: start.elapsed().as_millis() as u64,
            command: item.command.clone(),
            error,
        });
    }

    let total_freed_bytes = reports.iter().map(|r| r.freed_bytes).sum();
    let success_count = reports.iter().filter(|r| r.success).count();
    let fail_count = reports.len() - success_count;

    CleanSummary {
        reports,
        total_freed_bytes,
        success_count,
        fail_count,
    }
}

async fn run_shell_command(cmd: &str) -> Result<(), String> {
    // 关键：Tauri GUI 启动时 macOS 只给了极短的默认 PATH（/usr/bin:/bin:/usr/sbin:/sbin），
    // 而 pip / brew / pnpm / docker 等工具都在用户自己装的地方。
    //
    // 最靠谱的做法：用 login shell（-l）执行命令。
    // login shell 会读用户的 ~/.zshrc / ~/.bash_profile / ~/.profile 等配置，
    // 自动拿到 Homebrew / Anaconda / rustup / cargo / pnpm / nvm 等所有工具的 PATH。
    //
    // 这就是 macOS 菜单栏应用运行命令的标准做法。

    let shell = detect_user_shell();
    let augmented = augmented_path_for_spawn();

    // -l: login shell，读配置; -c: 执行命令字符串
    let output = tokio::process::Command::new(&shell)
        .args(["-l", "-c", cmd])
        .env("PATH", &augmented)
        // 确保 login shell 能读到 HOME / TERM 等
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .env("TERM", "xterm-256color")
        .output()
        .await
        .map_err(|e| format!("启动失败 ({}): {}", shell, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let msg = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("命令退出码 {:?}", output.status.code())
        };
        return Err(msg);
    }
    Ok(())
}

/// Pip 缓存清理 —— 绕开用户坏掉的 pip shebang。
///
/// 常见问题：用户装的 `/Users/xxx/Library/Python/3.9/bin/pip` 脚本第一行是
/// `#!/Applications/Xcode.app/Contents/Developer/usr/bin/python3`
/// 但 Xcode 卸载 / 升级后这条路径消失，pip 直接无法启动。
///
/// 解法：优先用 `python3 -m pip cache purge` —— 只要有任何一个可用的 python3
/// 就能执行。多级回退：
///   1) python3 -m pip cache purge
///   2) python -m pip cache purge
///   3) pip3 cache purge
///   4) pip cache purge
///   5) 直接删除 ~/Library/Caches/pip 目录（保底）
async fn clean_pip_cache(item: &CacheItem) -> Result<(), String> {
    let attempts = [
        "python3 -m pip cache purge",
        "python -m pip cache purge",
        "pip3 cache purge",
        "pip cache purge",
    ];
    let mut last_err = String::new();
    for cmd in attempts {
        match run_shell_command(cmd).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                // 保留最后一个错误供诊断
                last_err = format!("`{}` 失败: {}", cmd, e);
            }
        }
    }

    // 所有 python/pip 方式都失败 → 回退到直接删除目录（同 Xcode DerivedData 的做法）
    if let Some(path) = &item.path {
        let p = expand_tilde(path);
        if is_cleanup_path_allowed(&p) {
            if let Err(e) = remove_directory(&p).await {
                return Err(format!(
                    "{}；回退到直接删除也失败: {}",
                    last_err, e
                ));
            }
            return Ok(());
        }
    }

    Err(format!(
        "Pip 环境损坏（shebang 指向已卸载的 Python）：{}。\n请手动执行 `python3 -m pip cache purge`，或重新安装 pip。",
        last_err
    ))
}

/// 6 个月未访问 node_modules 的实际清理：重新扫描目录 + 移动到 /tmp
async fn clean_stale_node_modules(home_str: &str) -> Result<(), String> {
    let home = std::path::PathBuf::from(home_str);
    let project_roots = [
        "Projects",
        "projects",
        "Code",
        "code",
        "Developer",
        "developer",
        "workspace",
        "repos",
        "Repos",
        "git",
        "src",
        "Desktop",
        "Documents",
    ];

    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(180 * 24 * 3600))
        .unwrap_or(std::time::UNIX_EPOCH);

    // 在 blocking task 里扫描 + 清理
    let home_clone = home.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut cleaned = 0u32;
        let mut errors: Vec<String> = Vec::new();

        for root in project_roots {
            let root_path = home_clone.join(root);
            if !root_path.exists() {
                continue;
            }
            let walker = walkdir::WalkDir::new(&root_path)
                .max_depth(5)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    !name.starts_with('.') && name != "node_modules" || e.depth() == 0
                });
            for entry in walker.filter_map(|e| e.ok()) {
                if !entry.file_type().is_dir() {
                    continue;
                }
                if entry.file_name() != "node_modules" {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                let accessed = meta.accessed().or_else(|_| meta.modified()).ok();
                let Some(t) = accessed else { continue };
                if t >= cutoff {
                    continue;
                }

                // 移动到 /tmp
                let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                let parent_name = entry
                    .path()
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".into());
                let trash = std::env::temp_dir().join(format!(
                    "macflow-nodemodules-{}-{}-{}",
                    ts,
                    parent_name,
                    cleaned
                ));
                match std::fs::rename(entry.path(), &trash) {
                    Ok(_) => {
                        cleaned += 1;
                        // 后台异步实际删除
                        std::thread::spawn(move || {
                            let _ = std::fs::remove_dir_all(&trash);
                        });
                    }
                    Err(e) => {
                        errors.push(format!("{}: {}", entry.path().display(), e));
                    }
                }
            }
        }

        if cleaned == 0 && !errors.is_empty() {
            return Err(errors.join("; "));
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("扫描任务失败: {}", e))?
}

/// 检测用户使用的 shell。优先级：$SHELL > /bin/zsh > /bin/bash > /bin/sh
fn detect_user_shell() -> String {
    if let Ok(s) = std::env::var("SHELL") {
        if !s.is_empty() && std::path::Path::new(&s).exists() {
            return s;
        }
    }
    // macOS 10.15+ 默认 zsh
    for candidate in ["/bin/zsh", "/bin/bash", "/bin/sh"] {
        if std::path::Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    "/bin/sh".to_string()
}

/// 构造增强 PATH（兜底用，login shell 为主力）。
fn augmented_path_for_spawn() -> String {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let candidates = [
        format!("{}/.cargo/bin", home),
        format!("{}/.npm-global/bin", home),
        format!("{}/.yarn/bin", home),
        format!("{}/.bun/bin", home),
        format!("{}/Library/pnpm", home),
        format!("{}/Library/Python/3.9/bin", home),
        format!("{}/Library/Python/3.10/bin", home),
        format!("{}/Library/Python/3.11/bin", home),
        format!("{}/Library/Python/3.12/bin", home),
        format!("{}/Library/Python/3.13/bin", home),
        format!("{}/anaconda3/bin", home),
        format!("{}/miniconda3/bin", home),
        format!("{}/.local/bin", home),
        format!("{}/go/bin", home),
        "/opt/homebrew/bin".to_string(),
        "/opt/homebrew/sbin".to_string(),
        "/usr/local/bin".to_string(),
        "/usr/local/sbin".to_string(),
        "/Library/Frameworks/Python.framework/Versions/3.13/bin".to_string(),
        "/Library/Frameworks/Python.framework/Versions/3.12/bin".to_string(),
        "/Library/Frameworks/Python.framework/Versions/3.11/bin".to_string(),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ];

    let existing = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<String> = candidates.into_iter().collect();
    if !existing.is_empty() {
        parts.push(existing);
    }
    parts.join(":")
}

async fn remove_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if !is_cleanup_path_allowed(path) {
        return Err(format!(
            "执行前二次校验失败，拒绝删除: {}",
            path.display()
        ));
    }

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let trash = std::env::temp_dir().join(format!(
        "macflow-trash-{}-{}",
        timestamp,
        path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    ));

    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        // 先尝试 rename（原子操作，快）
        // 如果跨 volume 或权限不足，回退到逐文件删除
        match std::fs::rename(&path, &trash) {
            Ok(_) => {
                std::thread::spawn(move || {
                    let _ = std::fs::remove_dir_all(&trash);
                });
                Ok(())
            }
            Err(_) => {
                // rename 失败 → 直接递归删除（不经过 /tmp）
                std::fs::remove_dir_all(&path)
                    .map_err(|e| format!("删除失败: {}", e))
            }
        }
    })
    .await
    .map_err(|e| format!("任务失败: {}", e))?
}

/// 用 osascript 弹出系统密码框，以 sudo 权限删除 root 所有的目录
async fn remove_directory_sudo(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if !is_cleanup_path_allowed(path) {
        return Err(format!("路径不在白名单内，拒绝删除: {}", path.display()));
    }

    let path_str = path.to_string_lossy().to_string();
    // osascript 弹出系统原生密码框，用户授权后执行 sudo rm -rf
    let script = format!(
        r#"do shell script "rm -rf '{}'" with administrator privileges"#,
        path_str.replace('\'', "'\\''")
    );

    let output = tokio::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
        .map_err(|e| format!("osascript 启动失败: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // 用户取消授权时 osascript 返回 "User canceled"
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            Err("用户取消了授权".into())
        } else {
            Err(format!("需要管理员权限才能删除此目录: {}", stderr.trim()))
        }
    }
}

/// 展开 ~ 为家目录。
fn expand_tilde(s: &str) -> PathBuf {
    if let Some(stripped) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    if s == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(s));
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reject_root() {
        assert!(!is_cleanup_path_allowed(Path::new("/")));
    }

    #[test]
    fn reject_system_dirs() {
        for p in ["/usr", "/etc", "/var", "/bin", "/System", "/Applications"] {
            assert!(
                !is_cleanup_path_allowed(Path::new(p)),
                "必须拒绝 {}",
                p
            );
        }
    }

    #[test]
    fn reject_home() {
        if let Some(home) = dirs::home_dir() {
            assert!(!is_cleanup_path_allowed(&home), "绝不能允许删 home");
        }
    }

    #[test]
    fn reject_documents_or_downloads() {
        if let Some(home) = dirs::home_dir() {
            assert!(!is_cleanup_path_allowed(&home.join("Documents")));
            assert!(!is_cleanup_path_allowed(&home.join("Downloads")));
            assert!(!is_cleanup_path_allowed(&home.join("Desktop")));
        }
    }

    #[test]
    fn reject_nonexistent_random_path() {
        assert!(!is_cleanup_path_allowed(Path::new(
            "/this/does/not/exist/anywhere"
        )));
    }

    #[test]
    fn accept_npm_cache_when_exists() {
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".npm");
            if p.exists() {
                assert!(is_cleanup_path_allowed(&p), "~/.npm 应在白名单内");
            }
        }
    }

    #[test]
    fn expand_tilde_works() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_tilde("~/.npm"), home.join(".npm"));
        assert_eq!(
            expand_tilde("/absolute/path"),
            PathBuf::from("/absolute/path")
        );
    }
}
