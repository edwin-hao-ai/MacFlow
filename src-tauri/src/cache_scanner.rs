use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sysinfo::System;
use walkdir::WalkDir;

/// macOS 上常见的 CLI 工具安装路径（Tauri 打包后 PATH 只有 /usr/bin:/bin）
const EXTRA_PATHS: &[&str] = &[
    "/usr/local/bin",
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
];

/// 在标准 PATH + macOS 常见路径中查找可执行文件
fn find_tool(name: &str) -> Option<PathBuf> {
    // 先用 which（继承当前 PATH）
    if let Ok(p) = which::which(name) {
        return Some(p);
    }
    // 再查 macOS 常见路径
    for dir in EXTRA_PATHS {
        let p = PathBuf::from(dir).join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// 创建 Command 并注入扩展 PATH（确保 Tauri 沙箱内也能找到工具）
fn tool_command(name: &str) -> Option<std::process::Command> {
    let path = find_tool(name)?;
    let mut cmd = std::process::Command::new(path);
    // 把 EXTRA_PATHS 追加到 PATH 环境变量，让子进程也能找到依赖
    let current_path = std::env::var("PATH").unwrap_or_default();
    let extra = EXTRA_PATHS.join(":");
    cmd.env("PATH", format!("{}:{}", extra, current_path));
    Some(cmd)
}

/// 是否有任何一个指定名称的进程正在运行。
/// 用于避免在用户正在 npm install / cargo build / xcodebuild 时清理缓存。
pub fn is_any_tool_busy(names: &[&str]) -> Option<String> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for proc in sys.processes().values() {
        let pname = proc.name().to_string_lossy().to_lowercase();
        // 读 cmdline 第一个参数，匹配更准（比如 `node` 在跑 `npm install`）
        let cmd_first = proc
            .cmd()
            .first()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        for target in names {
            let t = target.to_lowercase();
            if pname == t || pname.starts_with(&format!("{}-", t)) {
                return Some(target.to_string());
            }
            // 如果 cmdline 第一段含工具名，也算
            if cmd_first.ends_with(&format!("/{}", t)) || cmd_first == t {
                return Some(target.to_string());
            }
            // 子命令匹配：node + npm/npx/pnpm 参数
            if (pname == "node" || pname.ends_with("/node"))
                && proc.cmd().iter().any(|a| {
                    let s = a.to_string_lossy().to_lowercase();
                    s.contains(&format!("/{}/", t)) || s.ends_with(&format!("/{}", t))
                })
            {
                return Some(target.to_string());
            }
        }
    }
    None
}

/// 缓存清理项 —— 代表一个「可以被清理的东西」。
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheItem {
    pub id: String,
    pub category: CacheCategory,
    pub label: String,
    pub description: String,
    pub path: Option<String>,
    pub size_bytes: u64,
    pub safety: Safety,
    pub default_select: bool,
    pub command: Option<String>,
    /// 恢复成本：清理后恢复需要做什么
    pub recover_hint: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheCategory {
    Npm,
    Pnpm,
    Yarn,
    Docker,
    Homebrew,
    Xcode,
    Cocoapods,
    Cargo,
    Pip,
    Go,
    System,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Safety {
    /// 100% 无风险，工具原生清理
    Safe,
    /// 低风险，通常不影响使用但需要重新下载
    Low,
    /// 中等风险，可能删除用户内容，默认不选
    Medium,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CacheScanResult {
    pub items: Vec<CacheItem>,
    pub total_bytes: u64,
    pub scanned_at_ms: u64,
}

pub async fn scan() -> CacheScanResult {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let home = Arc::new(home);

    // 并行扫描各类缓存
    let mut tasks = Vec::new();
    let h1 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_npm(&h1)));
    let h2 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_pnpm(&h2)));
    let h3 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_yarn(&h3)));
    tasks.push(tokio::task::spawn_blocking(scan_docker));
    tasks.push(tokio::task::spawn_blocking(scan_docker_stale_images));
    let h_nm = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || {
        scan_stale_node_modules(&h_nm)
    }));
    tasks.push(tokio::task::spawn_blocking(scan_homebrew));
    let h4 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_xcode(&h4)));
    let h5 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_cocoapods(&h5)));
    let h6 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_cargo(&h6)));
    let h7 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_pip(&h7)));
    let h8 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_go(&h8)));
    // 普通用户系统垃圾
    let h9 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_app_caches(&h9)));
    let h10 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_app_logs(&h10)));
    let h11 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_crash_reports(&h11)));
    let h12 = home.clone();
    tasks.push(tokio::task::spawn_blocking(move || scan_trash(&h12)));

    let mut items: Vec<CacheItem> = Vec::new();
    for t in tasks {
        if let Ok(batch) = t.await {
            items.extend(batch);
        }
    }

    // 过滤掉 size 为 0 的项（工具没装 / 没缓存）
    items.retain(|i| i.size_bytes > 0);

    // 按大小降序
    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    let total_bytes = items.iter().map(|i| i.size_bytes).sum();
    let scanned_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    CacheScanResult {
        items,
        total_bytes,
        scanned_at_ms,
    }
}

// ========== 各类扫描器 ==========

fn scan_npm(home: &Path) -> Vec<CacheItem> {
    if find_tool("npm").is_none() {
        return vec![];
    }
    // 安全闸门：如果 npm/npx 正在跑，绝对不动
    if is_any_tool_busy(&["npm", "npx"]).is_some() {
        return vec![];
    }
    let cache_dir = home.join(".npm");
    let size = dir_size(&cache_dir);
    if size == 0 {
        return vec![];
    }
    vec![CacheItem {
        id: "npm-cache".into(),
        category: CacheCategory::Npm,
        label: "NPM 全局缓存".into(),
        description: "npm 下载过的包的本地缓存，清理后首次安装会重新下载".into(),
        path: Some(cache_dir.display().to_string()),
        size_bytes: size,
        safety: Safety::Safe,
        default_select: true,
        command: None, // 直接删目录，比 npm cache clean 更彻底（后者不清 node-pre-gyp 缓存）
        recover_hint: "下次 npm install 会自动重新下载，不影响已安装的包".into(),
    }]
}

fn scan_pnpm(home: &Path) -> Vec<CacheItem> {
    if find_tool("pnpm").is_none() {
        return vec![];
    }
    if is_any_tool_busy(&["pnpm"]).is_some() {
        return vec![];
    }
    let store_dir = home.join("Library/pnpm/store");
    let size = dir_size(&store_dir);
    if size == 0 {
        return vec![];
    }
    vec![CacheItem {
        id: "pnpm-store".into(),
        category: CacheCategory::Pnpm,
        label: "PNPM Store".into(),
        description: "pnpm 全局 store，未被项目引用的包可安全剪枝".into(),
        path: Some(store_dir.display().to_string()),
        size_bytes: size,
        safety: Safety::Safe,
        default_select: true,
        command: Some("pnpm store prune".into()),
        recover_hint: "下次 pnpm install 会重新下载需要的包".into(),
    }]
}

fn scan_yarn(home: &Path) -> Vec<CacheItem> {
    if find_tool("yarn").is_none() {
        return vec![];
    }
    if is_any_tool_busy(&["yarn"]).is_some() {
        return vec![];
    }
    // Yarn v1 默认缓存路径
    let cache_dir = home.join("Library/Caches/Yarn");
    let size = dir_size(&cache_dir);
    if size == 0 {
        return vec![];
    }
    vec![CacheItem {
        id: "yarn-cache".into(),
        category: CacheCategory::Yarn,
        label: "Yarn 全局缓存".into(),
        description: "Yarn 下载过的包缓存".into(),
        path: Some(cache_dir.display().to_string()),
        size_bytes: size,
        safety: Safety::Safe,
        default_select: true,
        command: Some("yarn cache clean".into()),
        recover_hint: "下次 yarn install 会自动重新下载".into(),
    }]
}

fn scan_docker() -> Vec<CacheItem> {
    if find_tool("docker").is_none() {
        return vec![];
    }
    // 检查 daemon 是否运行
    let running = tool_command("docker")
        .and_then(|mut c| c.args(["info", "--format", "{{.ServerVersion}}"]).output().ok())
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !running {
        return vec![];
    }

    let mut out = Vec::new();
    let df = docker_system_df();

    // 镜像可回收空间（包含悬空 + 未使用的）
    if let Some(&sz) = df.get("images") {
        if sz > 0 {
            out.push(CacheItem {
                id: "docker-images-reclaimable".into(),
                category: CacheCategory::Docker,
                label: "Docker 可回收镜像".into(),
                description: "悬空镜像和未被容器引用的镜像层".into(),
                path: None,
                size_bytes: sz,
                safety: Safety::Low,
                default_select: false,
                command: Some("docker image prune -a -f".into()),
                recover_hint: "需要时用 docker pull 重新拉取".into(),
            });
        }
    }

    // 构建缓存
    if let Some(&sz) = df.get("build cache") {
        if sz > 0 {
            out.push(CacheItem {
                id: "docker-builder-cache".into(),
                category: CacheCategory::Docker,
                label: "Docker 构建缓存".into(),
                description: "docker build 产生的中间层缓存".into(),
                path: None,
                size_bytes: sz,
                safety: Safety::Safe,
                default_select: true,
                command: Some("docker builder prune -f".into()),
                recover_hint: "下次 docker build 会重新构建，首次较慢".into(),
            });
        }
    }

    // 容器可回收空间
    if let Some(&sz) = df.get("containers") {
        if sz > 0 {
            let stopped = tool_command("docker")
                .and_then(|mut c| c
                    .args(["container", "ls", "-aq", "--filter", "status=exited"])
                    .output().ok())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.lines().filter(|l| !l.is_empty()).count())
                .unwrap_or(0);
            if stopped > 0 {
                out.push(CacheItem {
                    id: "docker-stopped-containers".into(),
                    category: CacheCategory::Docker,
                    label: format!("停止的容器 ({} 个)", stopped),
                    description: "已退出的容器及其写入层".into(),
                    path: None,
                    size_bytes: sz,
                    safety: Safety::Safe,
                    default_select: true,
                    command: Some("docker container prune -f".into()),
                    recover_hint: "容器一旦删除无法恢复，但停止的容器通常已无价值".into(),
                });
            }
        }
    }

    // 未引用卷
    if let Some(&sz) = df.get("local volumes") {
        if sz > 0 {
            out.push(CacheItem {
                id: "docker-dangling-volumes".into(),
                category: CacheCategory::Docker,
                label: "Docker 未引用匿名卷".into(),
                description: "没有任何容器使用的匿名卷".into(),
                path: None,
                size_bytes: sz,
                safety: Safety::Low,
                default_select: false,
                command: Some("docker volume prune -f".into()),
                recover_hint: "卷中数据将永久丢失，请确认无重要数据".into(),
            });
        }
    }

    out
}

/// 3 个月（90 天）未使用的 Docker 镜像（非悬空）。
/// 这些镜像是用户拉下来用过、但近期没有容器引用过的 —— 大概率可安全删除。
/// 默认不选中（低风险），让用户自己勾。
fn scan_docker_stale_images() -> Vec<CacheItem> {
    if find_tool("docker").is_none() {
        return vec![];
    }
    let running = tool_command("docker")
        .and_then(|mut c| c.args(["info", "--format", "{{.ServerVersion}}"]).output().ok())
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !running {
        return vec![];
    }

    // docker images --format: id|repo|tag|created_at|size
    let out = match tool_command("docker") {
        Some(mut c) => c.args([
            "images",
            "--format",
            "{{.ID}}|{{.Repository}}|{{.Tag}}|{{.CreatedSince}}|{{.Size}}",
        ]).output(),
        None => return vec![],
    };
    let Ok(out) = out else {
        return vec![];
    };
    let Ok(text) = String::from_utf8(out.stdout) else {
        return vec![];
    };

    // 用 `docker ps -a --format {{.Image}}` 拿被容器引用的 image 集合
    let in_use_raw = tool_command("docker")
        .and_then(|mut c| c.args(["ps", "-a", "--format", "{{.Image}}"]).output().ok())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let in_use_set: std::collections::HashSet<String> =
        in_use_raw.lines().map(|s| s.trim().to_string()).collect();

    let mut stale_size: u64 = 0;
    let mut stale_count: u32 = 0;
    for line in text.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 5 {
            continue;
        }
        let id = parts[0].trim();
        let repo = parts[1].trim();
        let tag = parts[2].trim();
        let created_since = parts[3].trim().to_lowercase(); // e.g. "4 months ago"
        let size_str = parts[4].trim();

        // 跳过悬空镜像（交给 scan_docker 处理）
        if repo == "<none>" && tag == "<none>" {
            continue;
        }
        // 被引用就跳过
        if in_use_set.contains(&format!("{}:{}", repo, tag)) || in_use_set.contains(id) {
            continue;
        }

        // 时间筛选：「X months ago」或「X years ago」且 X 对应天数 > 90
        let days = parse_docker_age(&created_since);
        if days < 90 {
            continue;
        }

        stale_size += parse_human_size(size_str).unwrap_or(0);
        stale_count += 1;
    }

    if stale_count == 0 {
        return vec![];
    }

    vec![CacheItem {
        id: "docker-stale-images".into(),
        category: CacheCategory::Docker,
        label: format!("3 个月未使用 Docker 镜像 ({} 个)", stale_count),
        description: "非悬空、未被任何容器引用、创建或拉取时间超过 90 天的镜像".into(),
        path: None,
        size_bytes: stale_size,
        safety: Safety::Low,
        default_select: false,
        command: Some(
            "docker image prune -a --force --filter \"until=2160h\"".into(),
        ),
        recover_hint: "如需再使用，用 docker pull 重新拉取".into(),
    }]
}

/// 解析 Docker 的 CreatedSince 文本为天数
fn parse_docker_age(s: &str) -> u64 {
    // 格式：`N seconds/minutes/hours/days/weeks/months/years ago`
    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();
    if parts.len() < 2 {
        return 0;
    }
    let n: u64 = parts[0].parse().unwrap_or(0);
    match parts[1] {
        "second" | "seconds" => 0,
        "minute" | "minutes" => 0,
        "hour" | "hours" => 0,
        "day" | "days" => n,
        "week" | "weeks" => n * 7,
        "month" | "months" => n * 30,
        "year" | "years" => n * 365,
        _ => 0,
    }
}

/// 6 个月（180 天）以上未访问的 node_modules 目录
/// 扫描策略（避免扫整块磁盘，只找常见项目目录）：
///   ~/Projects、~/Code、~/Developer、~/workspace、~/repos、~/git、~/src、~/Desktop、~/Documents
/// 下的 **深度不超过 4 层** 的 node_modules
fn scan_stale_node_modules(home: &Path) -> Vec<CacheItem> {
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

    let mut stale_total: u64 = 0;
    let mut stale_count: u32 = 0;
    let mut examples: Vec<String> = Vec::new();

    for root in project_roots {
        let root_path = home.join(root);
        if !root_path.exists() {
            continue;
        }
        // 用 walkdir 限制深度 4，只找 node_modules 文件夹
        let walker = WalkDir::new(&root_path)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // 跳过 .git / 已经是 node_modules 内部
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "node_modules"
                    || e.depth() == 0 // 根目录不过滤
            });
        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_dir() {
                continue;
            }
            if entry.file_name() != "node_modules" {
                continue;
            }
            // 检查 node_modules 自身的访问时间
            let Ok(meta) = entry.metadata() else { continue };
            let accessed = meta.accessed().or_else(|_| meta.modified()).ok();
            let Some(t) = accessed else { continue };
            if t >= cutoff {
                continue;
            }
            // 计算大小
            let size = dir_size(entry.path());
            if size < 20 * 1024 * 1024 {
                // <20MB 就不值得了
                continue;
            }
            stale_total += size;
            stale_count += 1;
            if examples.len() < 5 {
                examples.push(entry.path().display().to_string());
            }
        }
    }

    if stale_count == 0 {
        return vec![];
    }

    let path_hint = if examples.len() > 3 {
        format!("{} 等", examples[0])
    } else {
        examples.join(", ")
    };

    vec![CacheItem {
        id: "stale-node-modules".into(),
        category: CacheCategory::Npm,
        label: format!("6 个月未访问的 node_modules ({} 个)", stale_count),
        description: format!("示例：{}", path_hint),
        path: None,
        size_bytes: stale_total,
        safety: Safety::Low,
        default_select: false,
        command: Some(format!(
            "__STALE_NODE_MODULES_CLEAN__:{}",
            home.display()
        )),
        recover_hint: "删除后需要在对应项目里重新 npm install / pnpm install".into(),
    }]
}

fn scan_homebrew() -> Vec<CacheItem> {
    let brew = find_tool("brew");
    if brew.is_none() {
        return vec![];
    }
    if is_any_tool_busy(&["brew"]).is_some() {
        return vec![];
    }
    // 两个可能的缓存位置
    let paths = [
        dirs::home_dir().map(|h| h.join("Library/Caches/Homebrew")),
        Some(PathBuf::from("/opt/homebrew/Library/Homebrew/cache")),
    ];
    let total: u64 = paths
        .iter()
        .filter_map(|p| p.as_ref())
        .map(|p| dir_size(p))
        .sum();
    if total == 0 {
        return vec![];
    }
    vec![CacheItem {
        id: "homebrew-cleanup".into(),
        category: CacheCategory::Homebrew,
        label: "Homebrew 旧包缓存".into(),
        description: "已下载的 bottle 文件和旧版本包，不影响已安装的软件".into(),
        path: Some("~/Library/Caches/Homebrew".into()),
        size_bytes: total,
        safety: Safety::Safe,
        default_select: true,
        command: None, // 直接删目录，比 brew cleanup 更彻底（后者只清有新版本的旧包）
        recover_hint: "已安装的工具完全不受影响，下次 brew install 会重新下载".into(),
    }]
}

fn scan_xcode(home: &Path) -> Vec<CacheItem> {
    // 安全闸门：Xcode 正在跑或 xcodebuild 正在执行 → 完全跳过
    if is_any_tool_busy(&["Xcode", "xcodebuild", "xcrun", "swift-frontend", "clang"]).is_some() {
        return vec![];
    }
    let derived = home.join("Library/Developer/Xcode/DerivedData");
    let archives = home.join("Library/Developer/Xcode/Archives");
    let simulator = home.join("Library/Developer/CoreSimulator/Caches");
    let ios_device = home.join("Library/Developer/Xcode/iOS DeviceSupport");

    let mut out = Vec::new();

    let dsz = dir_size(&derived);
    if dsz > 0 {
        out.push(CacheItem {
            id: "xcode-derived-data".into(),
            category: CacheCategory::Xcode,
            label: "Xcode DerivedData".into(),
            description: "Xcode 构建中间产物，重新打开项目会自动生成".into(),
            path: Some(derived.display().to_string()),
            size_bytes: dsz,
            safety: Safety::Safe,
            default_select: true,
            command: None, // 直接删除
            recover_hint: "Xcode 下次构建会重新生成".into(),
        });
    }

    let ssz = dir_size(&simulator);
    if ssz > 0 {
        out.push(CacheItem {
            id: "xcode-simulator-caches".into(),
            category: CacheCategory::Xcode,
            label: "iOS 模拟器缓存".into(),
            description: "CoreSimulator 的临时缓存".into(),
            path: Some(simulator.display().to_string()),
            size_bytes: ssz,
            safety: Safety::Safe,
            default_select: true,
            command: None,
            recover_hint: "模拟器重启后自动重建".into(),
        });
    }

    let isz = dir_size(&ios_device);
    if isz > 0 {
        out.push(CacheItem {
            id: "xcode-ios-devicesupport".into(),
            category: CacheCategory::Xcode,
            label: "iOS 设备支持文件".into(),
            description: "用于真机调试的 iOS 符号文件".into(),
            path: Some(ios_device.display().to_string()),
            size_bytes: isz,
            safety: Safety::Low,
            default_select: false,
            command: None,
            recover_hint: "下次连真机调试时 Xcode 会自动重新生成".into(),
        });
    }

    let asz = dir_size(&archives);
    if asz > 1024 * 1024 * 1024 {
        // Archives > 1GB 才列出，通常包含发布历史，用户应谨慎
        out.push(CacheItem {
            id: "xcode-archives".into(),
            category: CacheCategory::Xcode,
            label: "Xcode Archives".into(),
            description: "Archive 历史，可能包含生产包，清理前确认".into(),
            path: Some(archives.display().to_string()),
            size_bytes: asz,
            safety: Safety::Medium,
            default_select: false,
            command: None,
            recover_hint: "无法恢复，清理前请确认不再需要这些存档".into(),
        });
    }

    out
}

fn scan_cocoapods(home: &Path) -> Vec<CacheItem> {
    if is_any_tool_busy(&["pod"]).is_some() {
        return vec![];
    }
    let cache = home.join("Library/Caches/CocoaPods");
    let size = dir_size(&cache);
    if size == 0 {
        return vec![];
    }
    vec![CacheItem {
        id: "cocoapods-cache".into(),
        category: CacheCategory::Cocoapods,
        label: "CocoaPods 缓存".into(),
        description: "CocoaPods 下载的 spec 和 pod 缓存".into(),
        path: Some(cache.display().to_string()),
        size_bytes: size,
        safety: Safety::Safe,
        default_select: true,
        command: None,
        recover_hint: "下次 pod install 会自动重建".into(),
    }]
}

fn scan_cargo(home: &Path) -> Vec<CacheItem> {
    // 安全闸门：cargo / rustc 正在跑 → 跳过
    if is_any_tool_busy(&["cargo", "rustc", "rustup"]).is_some() {
        return vec![];
    }
    // 只清 registry/cache（.crate 压缩包）—— 这是最安全的
    // 不动 registry/src（解压后的源码，cargo 偶尔会直接读）
    // 不动 git（git 检出的依赖，重新拉取很慢且可能失败）
    let registry_cache = home.join(".cargo/registry/cache");
    let size = dir_size(&registry_cache);
    if size == 0 {
        return vec![];
    }
    vec![CacheItem {
        id: "cargo-registry-cache".into(),
        category: CacheCategory::Cargo,
        label: "Cargo 下载缓存".into(),
        description: "Cargo 下载的 .crate 压缩包，仅清缓存不碰源码".into(),
        path: Some("~/.cargo/registry/cache".into()),
        size_bytes: size,
        safety: Safety::Safe,
        default_select: true,
        command: None,
        recover_hint: "下次 cargo build 会自动重新下载。不影响已解压的源码，项目仍可离线构建".into(),
    }]
}

fn scan_pip(home: &Path) -> Vec<CacheItem> {
    if is_any_tool_busy(&["pip", "pip3"]).is_some() {
        return vec![];
    }
    let cache = home.join("Library/Caches/pip");
    let size = dir_size(&cache);
    if size == 0 {
        return vec![];
    }
    vec![CacheItem {
        id: "pip-cache".into(),
        category: CacheCategory::Pip,
        label: "Pip 缓存".into(),
        description: "pip 下载的 wheel 和源码包".into(),
        path: Some(cache.display().to_string()),
        size_bytes: size,
        safety: Safety::Safe,
        default_select: true,
        // 用特殊前缀 __PIP_CLEAN__ 让 cleaner 自己处理
        // 因为 `pip` 脚本的 shebang 经常是坏的（用户换了 Python 版本），
        // 我们要尝试多种方式
        command: Some("__PIP_CLEAN__".into()),
        recover_hint: "下次 pip install 会自动重新下载".into(),
    }]
}

fn scan_go(home: &Path) -> Vec<CacheItem> {
    if find_tool("go").is_none() {
        return vec![];
    }
    // 安全闸门：go build / go install / go test 正在跑 → 跳过
    if is_any_tool_busy(&["go", "gopls"]).is_some() {
        return vec![];
    }
    let build = home.join("Library/Caches/go-build");
    let size = dir_size(&build);
    if size == 0 {
        return vec![];
    }
    // 只清编译缓存（-cache），不动 modcache
    // modcache 清除后需要重新下载所有模块，对弱网用户风险大
    vec![CacheItem {
        id: "go-build-cache".into(),
        category: CacheCategory::Go,
        label: "Go 编译缓存".into(),
        description: "Go 编译产物缓存，不动下载的 modules".into(),
        path: Some("~/Library/Caches/go-build".into()),
        size_bytes: size,
        safety: Safety::Safe,
        default_select: true,
        command: Some("go clean -cache".into()),
        recover_hint: "下次 go build 会重新编译，首次慢一些，已下载的 modules 不受影响".into(),
    }]
}

// ========== 普通用户系统垃圾扫描 ==========

/// 扫描 ~/Library/Caches 下的应用缓存（按应用拆分，排除开发者工具和正在运行的应用）
fn scan_app_caches(home: &Path) -> Vec<CacheItem> {
    let caches_dir = home.join("Library/Caches");
    if !caches_dir.exists() {
        return vec![];
    }
    // 排除已被其他扫描器覆盖的目录
    let skip_contains = [
        "Homebrew", "pip", "go-build", "CocoaPods", "Yarn",
        "com.apple.DeveloperTools", "org.swift.swiftpm",
    ];
    // 排除 macOS 系统核心缓存（删了可能导致系统异常）
    let skip_prefix = [
        "com.apple.nsurlsessiond",
        "com.apple.Safari",  // Safari 缓存删了会丢已打开的标签页状态
        "com.apple.kernel",
        "com.apple.iconservices",
    ];

    // 获取正在运行的应用 bundle id 列表，避免清理正在使用的缓存
    let running_bundles = running_app_bundles();

    let entries = std::fs::read_dir(&caches_dir).ok();
    let Some(entries) = entries else { return vec![] };

    let mut items: Vec<CacheItem> = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if skip_contains.iter().any(|s| name.contains(s)) {
            continue;
        }
        if skip_prefix.iter().any(|s| name.starts_with(s)) {
            continue;
        }
        let sz = dir_size(&entry.path());
        if sz < 5 * 1024 * 1024 {
            continue; // < 5MB 不值得列出
        }

        // 正在运行的应用 → 标记为低风险且不默认选中
        let is_running = running_bundles.iter().any(|b| name.contains(b));
        let (safety, default_sel) = if is_running {
            (Safety::Low, false)
        } else {
            (Safety::Safe, true)
        };

        let label = friendly_app_name(&name);
        let running_hint = if is_running { "（正在运行）" } else { "" };

        items.push(CacheItem {
            id: format!("app-cache-{}", name),
            category: CacheCategory::System,
            label: format!("{} 缓存{}", label, running_hint),
            description: format!("应用本地缓存，清理后应用会自动重建"),
            path: Some(entry.path().display().to_string()),
            size_bytes: sz,
            safety,
            default_select: default_sel,
            command: None,
            recover_hint: "应用下次打开会自动重建缓存".into(),
        });
    }

    // 按大小降序，只保留前 20 个（避免列表太长）
    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    items.truncate(20);
    items
}

/// 获取正在运行的应用的 bundle identifier 列表
fn running_app_bundles() -> Vec<String> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut bundles = Vec::new();
    for proc in sys.processes().values() {
        let exe = proc.exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        // 从 /Applications/XXX.app/Contents/MacOS/xxx 提取 bundle id
        if exe.contains(".app/Contents/") {
            let name = proc.name().to_string_lossy().to_lowercase();
            bundles.push(name);
        }
    }
    bundles
}

/// 把 bundle id 风格的目录名转成友好的应用名
fn friendly_app_name(cache_dir_name: &str) -> String {
    // "com.google.Chrome" → "Chrome"
    // "com.tencent.xinWeChat" → "xinWeChat"
    // "org.mozilla.firefox" → "firefox"
    if let Some(last) = cache_dir_name.rsplit('.').next() {
        if last.len() > 1 {
            return last.to_string();
        }
    }
    cache_dir_name.to_string()
}

/// 扫描 ~/Library/Logs 下的应用日志
fn scan_app_logs(home: &Path) -> Vec<CacheItem> {
    let logs_dir = home.join("Library/Logs");
    if !logs_dir.exists() {
        return vec![];
    }
    let total = dir_size(&logs_dir);
    if total < 5 * 1024 * 1024 {
        return vec![]; // < 5MB 不值得
    }
    vec![CacheItem {
        id: "system-app-logs".into(),
        category: CacheCategory::System,
        label: "应用日志".into(),
        description: "各应用产生的日志文件，通常无需保留".into(),
        path: Some("~/Library/Logs".into()),
        size_bytes: total,
        safety: Safety::Safe,
        default_select: true,
        command: None,
        recover_hint: "日志会在应用运行时自动重新生成".into(),
    }]
}

/// 扫描崩溃报告
fn scan_crash_reports(home: &Path) -> Vec<CacheItem> {
    let dirs = [
        home.join("Library/Logs/DiagnosticReports"),
        home.join("Library/Application Support/CrashReporter"),
    ];
    let total: u64 = dirs.iter().map(|d| dir_size(d)).sum();
    if total < 1024 * 1024 {
        return vec![]; // < 1MB 不值得
    }
    vec![CacheItem {
        id: "system-crash-reports".into(),
        category: CacheCategory::System,
        label: "崩溃报告".into(),
        description: "应用崩溃时生成的诊断文件，通常已无调试价值".into(),
        path: Some("~/Library/Logs/DiagnosticReports".into()),
        size_bytes: total,
        safety: Safety::Safe,
        default_select: true,
        command: None,
        recover_hint: "崩溃报告删除后不影响任何功能".into(),
    }]
}

/// 扫描废纸篓大小
fn scan_trash(home: &Path) -> Vec<CacheItem> {
    let trash = home.join(".Trash");
    if !trash.exists() {
        return vec![];
    }
    let total = dir_size(&trash);
    if total < 10 * 1024 * 1024 {
        return vec![]; // < 10MB 不值得
    }
    vec![CacheItem {
        id: "system-trash".into(),
        category: CacheCategory::System,
        label: "废纸篓".into(),
        description: "已删除但未清空的文件，占用磁盘空间".into(),
        path: Some("~/.Trash".into()),
        size_bytes: total,
        safety: Safety::Safe,
        default_select: false, // 废纸篓默认不选，用户可能还想恢复
        command: None,
        recover_hint: "清空后无法恢复，请确认废纸篓中没有需要的文件".into(),
    }]
}

// ========== 辅助函数 ==========

/// 递归计算目录大小。失败或不存在返回 0。
fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// 通过 `docker system df --format` 获取各类资源的可回收空间
fn docker_system_df() -> std::collections::HashMap<String, u64> {
    let mut map = std::collections::HashMap::new();
    let out = match tool_command("docker") {
        Some(mut c) => c.args(["system", "df", "--format", "{{.Type}}\t{{.Reclaimable}}"]).output(),
        None => return map,
    };
    let Ok(out) = out else { return map };
    let Ok(text) = String::from_utf8(out.stdout) else { return map };
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 { continue; }
        let kind = parts[0].trim().to_lowercase();
        // Reclaimable 格式: "9.073GB (77%)" 或 "0B (0%)"
        let size_part = parts[1].split('(').next().unwrap_or("").trim();
        if let Some(sz) = parse_human_size(size_part) {
            map.insert(kind, sz);
        }
    }
    map
}

fn parse_human_size(s: &str) -> Option<u64> {
    // 寻找 "123.45 GB" / "234MB" 模式
    let re_chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < re_chars.len() {
        if re_chars[i].is_ascii_digit() {
            // 找数字起点
            let start = i;
            while i < re_chars.len() && (re_chars[i].is_ascii_digit() || re_chars[i] == '.') {
                i += 1;
            }
            let num_str: String = re_chars[start..i].iter().collect();
            // 跳空格
            while i < re_chars.len() && re_chars[i] == ' ' {
                i += 1;
            }
            // 单位
            let unit_start = i;
            while i < re_chars.len() && re_chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let unit: String = re_chars[unit_start..i].iter().collect();
            let mult: u64 = match unit.to_uppercase().as_str() {
                "B" => 1,
                "KB" | "K" => 1024,
                "MB" | "M" => 1024 * 1024,
                "GB" | "G" => 1024 * 1024 * 1024,
                "TB" | "T" => 1024u64.pow(4),
                _ => continue,
            };
            if let Ok(n) = num_str.parse::<f64>() {
                return Some((n * mult as f64) as u64);
            }
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_human_size 单元测试 ──

    #[test]
    fn parse_gb() {
        let v = parse_human_size("9.073GB").unwrap();
        // 浮点精度：允许 0.1% 误差
        assert!(v > 9_700_000_000 && v < 9_800_000_000, "got {}", v);
    }

    #[test]
    fn parse_gb_with_space() {
        let v = parse_human_size("9.073 GB").unwrap();
        assert!(v > 9_700_000_000 && v < 9_800_000_000, "got {}", v);
    }

    #[test]
    fn parse_mb() {
        let v = parse_human_size("43.64MB").unwrap();
        assert!(v > 45_000_000 && v < 46_000_000, "got {}", v);
    }

    #[test]
    fn parse_zero_b() {
        assert_eq!(parse_human_size("0B"), Some(0));
    }

    #[test]
    fn parse_kb() {
        assert_eq!(parse_human_size("512KB"), Some(524_288));
    }

    #[test]
    fn parse_with_paren_suffix() {
        let trimmed = "9.073GB (77%)".split('(').next().unwrap().trim();
        let v = parse_human_size(trimmed).unwrap();
        assert!(v > 9_700_000_000 && v < 9_800_000_000, "got {}", v);
    }

    #[test]
    fn parse_empty_returns_none() {
        assert_eq!(parse_human_size(""), None);
    }

    #[test]
    fn parse_no_unit_returns_none() {
        assert_eq!(parse_human_size("hello"), None);
    }

    // ── docker_system_df 集成测试（需要 Docker 运行） ──

    #[test]
    fn docker_system_df_returns_data_when_running() {
        // 跳过：Docker 没装或没运行
        let running = std::process::Command::new("docker")
            .args(["info", "--format", "{{.ServerVersion}}"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !running {
            eprintln!("跳过：Docker 未运行");
            return;
        }

        let df = docker_system_df();
        // docker system df 至少应该返回 images / containers / build cache / local volumes
        eprintln!("docker_system_df 结果: {:?}", df);
        assert!(
            !df.is_empty(),
            "Docker 正在运行但 docker_system_df 返回空 HashMap"
        );
        // 至少应该有 images 这个 key
        assert!(
            df.contains_key("images"),
            "缺少 images key，实际 keys: {:?}",
            df.keys().collect::<Vec<_>>()
        );
    }

    // ── docker system df --format 原始输出调试 ──

    #[test]
    fn docker_system_df_raw_output() {
        let running = std::process::Command::new("docker")
            .args(["info", "--format", "{{.ServerVersion}}"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !running {
            eprintln!("跳过：Docker 未运行");
            return;
        }

        let out = std::process::Command::new("docker")
            .args(["system", "df", "--format", "{{.Type}}\t{{.Reclaimable}}"])
            .output()
            .expect("docker system df 执行失败");

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        eprintln!("=== stdout ===\n{}", stdout);
        eprintln!("=== stderr ===\n{}", stderr);
        eprintln!("=== exit code: {:?} ===", out.status.code());

        assert!(
            out.status.success(),
            "docker system df 命令失败: {}",
            stderr
        );
        assert!(
            !stdout.is_empty(),
            "docker system df stdout 为空，stderr: {}",
            stderr
        );
    }

    // ── scan_docker 集成测试 ──

    #[test]
    fn scan_docker_finds_items_when_running() {
        let running = std::process::Command::new("docker")
            .args(["info", "--format", "{{.ServerVersion}}"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !running {
            eprintln!("跳过：Docker 未运行");
            return;
        }

        let items = scan_docker();
        eprintln!("scan_docker 返回 {} 项:", items.len());
        for item in &items {
            eprintln!(
                "  - {} | {} | {} bytes",
                item.id, item.label, item.size_bytes
            );
        }
        // Docker 正在运行时，至少应该有构建缓存或镜像可回收
        // （你的机器上有 9GB 镜像 + 3.2GB 构建缓存）
        assert!(
            !items.is_empty(),
            "Docker 正在运行且有缓存，但 scan_docker 返回空"
        );
    }

    // ── scan_docker_stale_images 集成测试 ──

    #[test]
    fn scan_docker_stale_images_does_not_panic() {
        // 不要求有结果，只要求不 panic
        let items = scan_docker_stale_images();
        eprintln!("scan_docker_stale_images 返回 {} 项", items.len());
        for item in &items {
            eprintln!("  - {} | {} bytes", item.label, item.size_bytes);
        }
    }

    // ── 完整 scan() 集成测试 ──

    #[test]
    fn full_cache_scan_returns_results() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(scan());
        eprintln!(
            "完整缓存扫描: {} 项, 总计 {} bytes",
            result.items.len(),
            result.total_bytes
        );
        for item in &result.items {
            eprintln!(
                "  [{:?}] {} — {} bytes (safety={:?}, default={})",
                item.category, item.label, item.size_bytes, item.safety, item.default_select
            );
        }
        // 你的机器上至少有 Cargo 缓存
        assert!(!result.items.is_empty(), "缓存扫描结果为空");
    }
}
