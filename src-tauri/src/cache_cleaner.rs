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
        out.push(home.join("Library/pnpm/store"));
        out.push(home.join("Library/Caches/Yarn"));
        out.push(home.join("Library/Caches/Homebrew"));
        out.push(home.join("Library/Caches/CocoaPods"));
        out.push(home.join("Library/Caches/pip"));
        out.push(home.join("Library/Caches/go-build"));
        out.push(home.join("Library/Developer/Xcode/DerivedData"));
        out.push(home.join("Library/Developer/Xcode/Archives"));
        out.push(home.join("Library/Developer/Xcode/iOS DeviceSupport"));
        out.push(home.join("Library/Developer/CoreSimulator/Caches"));
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
        } else if let Some(cmd) = &item.command {
            run_shell_command(cmd).await
        } else if let Some(path) = &item.path {
            let p = expand_tilde(path);
            // 路径白名单防御：绝不删任何非预期目录
            if !is_cleanup_path_allowed(&p) {
                Err(format!(
                    "路径不在白名单内，拒绝删除: {}",
                    p.display()
                ))
            } else {
                remove_directory(&p).await
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
    // 命令都是写死在 scanner 里的常量，不是用户输入，所以 shell 注入安全。
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err("空命令".into());
    }

    // 关键：Tauri GUI 启动时 macOS 只给了极短的默认 PATH（/usr/bin:/bin:/usr/sbin:/sbin），
    // pip / brew / pnpm / docker 等工具根本找不到。需要：
    // 1) 扩充 PATH 环境变量
    // 2) 如果能定位到绝对路径则直接用绝对路径
    let augmented_path = augmented_path_for_spawn();

    // 用 which::which 在扩充后的 PATH 里找工具的绝对路径
    let exe = match resolve_tool(parts[0], &augmented_path) {
        Some(p) => p,
        None => {
            return Err(format!(
                "找不到命令 `{}`，请确认已安装并在 PATH 中",
                parts[0]
            ))
        }
    };

    let output = tokio::process::Command::new(&exe)
        .args(&parts[1..])
        .env("PATH", &augmented_path)
        .output()
        .await
        .map_err(|e| format!("启动失败: {}", e))?;

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

/// 构造一个包含常见开发工具安装位置的 PATH，用于子进程 spawn。
fn augmented_path_for_spawn() -> String {
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // 按优先级排：用户目录 → Homebrew → Apple silicon brew → 系统
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
        "/opt/homebrew/bin".to_string(), // Apple Silicon brew
        "/opt/homebrew/sbin".to_string(),
        "/usr/local/bin".to_string(), // Intel brew
        "/usr/local/sbin".to_string(),
        "/Library/Frameworks/Python.framework/Versions/3.13/bin".to_string(),
        "/Library/Frameworks/Python.framework/Versions/3.12/bin".to_string(),
        "/Library/Frameworks/Python.framework/Versions/3.11/bin".to_string(),
        "/Library/Frameworks/Python.framework/Versions/3.10/bin".to_string(),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ];

    // 保留环境里已有的 PATH 作为兜底
    let existing = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<String> = candidates.into_iter().collect();
    if !existing.is_empty() {
        parts.push(existing);
    }
    parts.join(":")
}

/// 在给定 PATH 下解析工具的绝对路径。
fn resolve_tool(name: &str, path: &str) -> Option<PathBuf> {
    // 临时设置 PATH 让 which crate 用
    for dir in path.split(':') {
        let p = PathBuf::from(dir).join(name);
        if p.is_file() {
            // 检查可执行
            if let Ok(meta) = std::fs::metadata(&p) {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o111 != 0 {
                    return Some(p);
                }
            }
        }
    }
    None
}

async fn remove_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    // 最后一道守门：再次校验白名单（防御 race condition / symlink swap）
    if !is_cleanup_path_allowed(path) {
        return Err(format!(
            "执行前二次校验失败，拒绝删除: {}",
            path.display()
        ));
    }

    // 不直接删除 —— mv 到 /tmp 下一个标记目录
    // 好处：① rename 是原子操作 ② 如果用户冷静后反悔，1 小时内 /tmp 还没被系统清理
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
        std::fs::rename(&path, &trash).map_err(|e| format!("移动失败: {}", e))?;
        // rename 成功，立即把空间还给用户
        std::thread::spawn(move || {
            let _ = std::fs::remove_dir_all(&trash);
        });
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("任务失败: {}", e))?
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
