use crate::cache_scanner::CacheItem;
use serde::Serialize;
use std::path::Path;
use std::time::Instant;

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

        let result = if let Some(cmd) = &item.command {
            run_shell_command(cmd).await
        } else if let Some(path) = &item.path {
            // 无原生命令 —— 移动到 Trash 式的删除
            remove_directory(Path::new(path)).await
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
    // 把命令串拆成 args。这里要小心 shell 注入 ——
    // 我们的命令都是写死在 scanner 里的常量，不是用户输入，所以安全。
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err("空命令".into());
    }

    let output = tokio::process::Command::new(parts[0])
        .args(&parts[1..])
        .output()
        .await
        .map_err(|e| format!("启动失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }
    Ok(())
}

async fn remove_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    // 不直接删除用户目录 —— 用 mv 到 /tmp 下一个标记目录，系统重启会清理
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let trash = std::env::temp_dir().join(format!(
        "macflow-trash-{}-{}",
        timestamp,
        path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
    ));

    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::rename(&path, &trash).map_err(|e| format!("移动失败: {}", e))?;
        // rename 成功即返回，真正删除由系统在 /tmp 自动清理时完成
        // 但为了立即释放空间，我们还是后台删一下
        std::thread::spawn(move || {
            let _ = std::fs::remove_dir_all(&trash);
        });
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("任务失败: {}", e))?
}
