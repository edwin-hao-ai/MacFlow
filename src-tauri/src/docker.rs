//! Docker 深度视图 —— 列举镜像 / 容器 / 卷 / 构建缓存，支持单项删除。
//!
//! 使用 `docker` CLI（与 CacheView 的批量清理互补）。
//! 所有命令走 tokio::process 并显式设 PATH，避免 GUI 启动没继承 shell PATH 的问题。

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct DockerImage {
    pub id: String,         // short id
    pub repository: String, // nginx
    pub tag: String,        // latest / <none>
    pub size_bytes: u64,
    pub created: String,    // 2026-01-15 10:20:30 +0800 CST
    pub dangling: bool,     // repository/tag 为 <none>
    pub in_use: bool,       // 是否被某个容器引用
}

#[derive(Serialize, Clone, Debug)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,   // running / exited (0) 2 days ago
    pub running: bool,
    pub size_bytes: u64,  // RW 层大小
    pub created: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct DockerVolume {
    pub name: String,
    pub driver: String,
    pub size_bytes: u64,
    pub in_use: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct DockerBuilderCache {
    pub total_bytes: u64,
    pub reclaimable_bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct DockerInventory {
    pub daemon_running: bool,
    pub images: Vec<DockerImage>,
    pub containers: Vec<DockerContainer>,
    pub volumes: Vec<DockerVolume>,
    pub builder: DockerBuilderCache,
    /// 总可回收大小
    pub reclaimable_bytes: u64,
}

/// Docker CLI 是否在 PATH + daemon 是否在运行
pub async fn is_available() -> bool {
    if which::which("docker").is_err() {
        return false;
    }
    run_docker(&["info", "--format", "{{.ServerVersion}}"])
        .await
        .map(|out| !out.trim().is_empty())
        .unwrap_or(false)
}

pub async fn inventory() -> Result<DockerInventory, String> {
    let available = is_available().await;
    if !available {
        return Ok(DockerInventory {
            daemon_running: false,
            images: vec![],
            containers: vec![],
            volumes: vec![],
            builder: DockerBuilderCache {
                total_bytes: 0,
                reclaimable_bytes: 0,
            },
            reclaimable_bytes: 0,
        });
    }

    let images = list_images().await.unwrap_or_default();
    let containers = list_containers().await.unwrap_or_default();
    let volumes = list_volumes().await.unwrap_or_default();
    let builder = builder_cache().await.unwrap_or(DockerBuilderCache {
        total_bytes: 0,
        reclaimable_bytes: 0,
    });

    let mut reclaimable = builder.reclaimable_bytes;
    // 悬空镜像 100% 可回收
    reclaimable += images
        .iter()
        .filter(|i| i.dangling)
        .map(|i| i.size_bytes)
        .sum::<u64>();
    // 已停止容器
    reclaimable += containers
        .iter()
        .filter(|c| !c.running)
        .map(|c| c.size_bytes)
        .sum::<u64>();
    // 未被容器引用的卷
    reclaimable += volumes
        .iter()
        .filter(|v| !v.in_use)
        .map(|v| v.size_bytes)
        .sum::<u64>();

    Ok(DockerInventory {
        daemon_running: true,
        images,
        containers,
        volumes,
        builder,
        reclaimable_bytes: reclaimable,
    })
}

async fn list_images() -> Result<Vec<DockerImage>, String> {
    // 用 Go template 拿结构化数据：id|repo|tag|size|created|dangling
    let out = run_docker(&[
        "images",
        "-a",
        "--no-trunc",
        "--format",
        "{{.ID}}|{{.Repository}}|{{.Tag}}|{{.Size}}|{{.CreatedAt}}|{{.Digest}}",
    ])
    .await?;

    // 收集被容器引用的镜像 ID（完整 digest）
    let in_use_ids = run_docker(&["ps", "-a", "--format", "{{.Image}}"])
        .await
        .unwrap_or_default();
    let in_use_set: std::collections::HashSet<String> = in_use_ids
        .lines()
        .map(|s| s.trim().to_string())
        .collect();

    let mut images = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 5 {
            continue;
        }
        let id_full = parts[0].trim().trim_start_matches("sha256:");
        let id_short = id_full.chars().take(12).collect::<String>();
        let repo = parts[1].trim().to_string();
        let tag = parts[2].trim().to_string();
        let size = parse_human_size(parts[3].trim());
        let created = parts[4].trim().to_string();
        let dangling = repo == "<none>" && tag == "<none>";
        let in_use = in_use_set.contains(&format!("{}:{}", repo, tag))
            || in_use_set.contains(id_full)
            || in_use_set.contains(&id_short);

        images.push(DockerImage {
            id: id_short,
            repository: repo,
            tag,
            size_bytes: size,
            created,
            dangling,
            in_use,
        });
    }
    Ok(images)
}

async fn list_containers() -> Result<Vec<DockerContainer>, String> {
    let out = run_docker(&[
        "ps",
        "-a",
        "--size",
        "--format",
        "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.State}}|{{.Size}}|{{.CreatedAt}}",
    ])
    .await?;

    let mut containers = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 7 {
            continue;
        }
        let id = parts[0].trim().chars().take(12).collect::<String>();
        let name = parts[1].trim().to_string();
        let image = parts[2].trim().to_string();
        let status = parts[3].trim().to_string();
        let state = parts[4].trim().to_string();
        let size_str = parts[5].trim();
        // Size 形如 "1.2MB (virtual 150MB)"，取第一部分
        let size = parse_human_size(size_str.split('(').next().unwrap_or("0").trim());
        let created = parts[6].trim().to_string();
        containers.push(DockerContainer {
            id,
            name,
            image,
            status,
            running: state == "running",
            size_bytes: size,
            created,
        });
    }
    Ok(containers)
}

async fn list_volumes() -> Result<Vec<DockerVolume>, String> {
    // 基础信息
    let out = run_docker(&["volume", "ls", "--format", "{{.Name}}|{{.Driver}}"]).await?;

    // 哪些卷被容器引用
    let in_use_raw = run_docker(&[
        "ps",
        "-a",
        "--format",
        "{{.Mounts}}",
    ])
    .await
    .unwrap_or_default();
    let mut in_use_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in in_use_raw.lines() {
        for m in line.split(',') {
            let m = m.trim();
            if !m.is_empty() {
                in_use_set.insert(m.to_string());
            }
        }
    }

    let mut volumes = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0].trim().to_string();
        let driver = parts[1].trim().to_string();
        let in_use = in_use_set.contains(&name);
        // size via inspect + du -sh（太慢），这里给 0，上游用 docker system df 估算
        volumes.push(DockerVolume {
            name,
            driver,
            size_bytes: 0,
            in_use,
        });
    }
    Ok(volumes)
}

async fn builder_cache() -> Result<DockerBuilderCache, String> {
    // docker system df --format table 不够好，用 docker builder du
    let out = run_docker(&["builder", "du"]).await?;
    // 简单解析：最后一行 "Reclaimable: 1.5GB"
    let mut total = 0u64;
    let mut reclaimable = 0u64;
    for line in out.lines() {
        let line = line.trim().to_lowercase();
        if line.starts_with("reclaimable space:") || line.starts_with("reclaimable:") {
            reclaimable = parse_human_size(line.split(':').nth(1).unwrap_or("0").trim());
        } else if line.starts_with("shared space:") || line.starts_with("total:") {
            total = parse_human_size(line.split(':').nth(1).unwrap_or("0").trim());
        }
    }
    Ok(DockerBuilderCache {
        total_bytes: total,
        reclaimable_bytes: reclaimable,
    })
}

pub async fn remove_image(id: &str) -> Result<(), String> {
    // -f 强制（镜像可能被停止容器引用）
    run_docker(&["image", "rm", "-f", id]).await.map(|_| ())
}

pub async fn remove_container(id: &str) -> Result<(), String> {
    run_docker(&["rm", "-f", id]).await.map(|_| ())
}

pub async fn remove_volume(name: &str) -> Result<(), String> {
    run_docker(&["volume", "rm", "-f", name]).await.map(|_| ())
}

/// `docker system prune -f --volumes` —— 一键删除悬空镜像 + 停止容器 + 构建缓存 + 未引用卷
pub async fn prune_all() -> Result<String, String> {
    run_docker(&["system", "prune", "-f", "--volumes"]).await
}

// ---- 辅助 ----

async fn run_docker(args: &[&str]) -> Result<String, String> {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();
    let path = format!(
        "/opt/homebrew/bin:/usr/local/bin:{}/.docker/bin:/Applications/Docker.app/Contents/Resources/bin:{}",
        home,
        std::env::var("PATH").unwrap_or_default()
    );

    let output = tokio::process::Command::new("docker")
        .args(args)
        .env("PATH", &path)
        .output()
        .await
        .map_err(|e| format!("启动 docker 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_human_size(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() || s == "0" || s == "0B" {
        return 0;
    }
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
        i += 1;
    }
    let num: f64 = chars[..i].iter().collect::<String>().parse().unwrap_or(0.0);
    let unit: String = chars[i..]
        .iter()
        .collect::<String>()
        .trim()
        .to_uppercase()
        .replace('B', "");
    let mult: u64 = match unit.as_str() {
        "" | "B" => 1,
        "K" | "KB" | "KIB" => 1024,
        "M" | "MB" | "MIB" => 1024 * 1024,
        "G" | "GB" | "GIB" => 1024u64.pow(3),
        "T" | "TB" | "TIB" => 1024u64.pow(4),
        _ => 1,
    };
    (num * mult as f64) as u64
}
