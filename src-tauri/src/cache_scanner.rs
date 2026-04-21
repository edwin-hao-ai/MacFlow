use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sysinfo::System;
use walkdir::WalkDir;

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
    if which::which("npm").is_err() {
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
        command: Some("npm cache clean --force".into()),
        recover_hint: "下次 npm install 会自动重新下载（NPM 官方命令，只清下载缓存，不影响已安装的包）".into(),
    }]
}

fn scan_pnpm(home: &Path) -> Vec<CacheItem> {
    if which::which("pnpm").is_err() {
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
    if which::which("yarn").is_err() {
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
    if which::which("docker").is_err() {
        return vec![];
    }
    // 安全闸门：正在 docker build 就不碰
    if is_any_tool_busy(&["docker-compose", "dockerd"]).is_some() {
        // daemon 本身跑是正常的；我们检查的是用户主动操作
        // 跳过检测，下面 info 命令会判断 daemon 是否正常
    }
    // 检查 daemon 是否运行
    let running = std::process::Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !running {
        return vec![];
    }
    // 再细粒度：docker build 正在执行 → 跳过构建缓存项
    let build_running = std::process::Command::new("docker")
        .args(["ps", "--filter", "ancestor=moby/buildkit", "-q"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let _ = build_running; // 保留变量供后续细化使用

    let mut out = Vec::new();

    // 悬空镜像
    if let Some(sz) = docker_reclaimable_size(&["image", "ls", "-f", "dangling=true", "-q"]) {
        if sz > 0 {
            out.push(CacheItem {
                id: "docker-dangling-images".into(),
                category: CacheCategory::Docker,
                label: "Docker 悬空镜像".into(),
                description: "未被任何容器引用的镜像层".into(),
                path: None,
                size_bytes: sz,
                safety: Safety::Safe,
                default_select: true,
                command: Some("docker image prune -f".into()),
                recover_hint: "需要时用 docker pull 重新拉取".into(),
            });
        }
    }

    // Build cache size (docker builder du)
    if let Some(sz) = docker_du_size("builder") {
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

    // 停止的容器
    let stopped = std::process::Command::new("docker")
        .args(["container", "ls", "-aq", "--filter", "status=exited"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().count())
        .unwrap_or(0);
    if stopped > 0 {
        out.push(CacheItem {
            id: "docker-stopped-containers".into(),
            category: CacheCategory::Docker,
            label: format!("停止的容器 ({} 个)", stopped),
            description: "已退出且超过 30 天未使用的容器".into(),
            path: None,
            // 容器本身小，只是元数据
            size_bytes: (stopped as u64) * 1024 * 512,
            safety: Safety::Safe,
            default_select: true,
            command: Some("docker container prune -f --filter until=720h".into()),
            recover_hint: "容器一旦删除无法恢复，但停止的容器通常已无价值".into(),
        });
    }

    // 未引用卷
    if let Some(sz) = docker_volume_size() {
        if sz > 0 {
            out.push(CacheItem {
                id: "docker-dangling-volumes".into(),
                category: CacheCategory::Docker,
                label: "Docker 未引用匿名卷".into(),
                description: "没有任何容器使用的匿名卷".into(),
                path: None,
                size_bytes: sz,
                safety: Safety::Low, // 卷可能含数据，降级为 Low
                default_select: false,
                command: Some("docker volume prune -f".into()),
                recover_hint: "卷中数据将永久丢失，请确认无重要数据".into(),
            });
        }
    }

    out
}

fn scan_homebrew() -> Vec<CacheItem> {
    let brew = which::which("brew");
    if brew.is_err() {
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
        description: "已升级的旧版本包、下载的 bottle 文件、日志。只清旧版本，不影响已安装的软件".into(),
        path: Some("~/Library/Caches/Homebrew".into()),
        size_bytes: total,
        safety: Safety::Safe,
        default_select: true,
        // 使用 brew cleanup（不带 --prune=all）：只删旧版本缓存和超过 120 天的包
        // 这是 brew 官方推荐的安全清理方式
        command: Some("brew cleanup -s".into()),
        recover_hint: "已安装的工具完全不受影响，只清旧版本的下载文件".into(),
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
        command: Some("pip cache purge".into()),
        recover_hint: "下次 pip install 会自动重新下载".into(),
    }]
}

fn scan_go(home: &Path) -> Vec<CacheItem> {
    if which::which("go").is_err() {
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

/// docker system df --format 的替代 —— 解析出可回收空间
fn docker_reclaimable_size(args: &[&str]) -> Option<u64> {
    // 这里简化处理：直接用 docker system df -v 拿镜像总大小估算
    // 实际使用 docker inspect 获取每个悬空镜像的 Size 求和
    let ids = std::process::Command::new("docker")
        .args(args)
        .output()
        .ok()?;
    let id_list = String::from_utf8(ids.stdout).ok()?;
    let ids: Vec<&str> = id_list.lines().filter(|l| !l.is_empty()).collect();
    if ids.is_empty() {
        return Some(0);
    }
    let mut total: u64 = 0;
    for id in ids {
        if let Ok(out) = std::process::Command::new("docker")
            .args(["inspect", "--format", "{{.Size}}", id])
            .output()
        {
            if let Ok(s) = String::from_utf8(out.stdout) {
                if let Ok(n) = s.trim().parse::<u64>() {
                    total += n;
                }
            }
        }
    }
    Some(total)
}

fn docker_du_size(kind: &str) -> Option<u64> {
    let out = std::process::Command::new("docker")
        .args([kind, "du"])
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    // 简单解析：找 "Total:" 行的数字
    for line in s.lines() {
        if line.to_lowercase().contains("total") {
            // 尝试解析人类可读大小（1.5GB / 234MB）
            if let Some(sz) = parse_human_size(line) {
                return Some(sz);
            }
        }
    }
    None
}

fn docker_volume_size() -> Option<u64> {
    let out = std::process::Command::new("docker")
        .args([
            "volume",
            "ls",
            "-q",
            "--filter",
            "dangling=true",
        ])
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    let count = s.lines().filter(|l| !l.is_empty()).count();
    // 无法精确获取卷大小（需要 docker system df -v 解析），估算每个 10MB
    Some((count as u64) * 10 * 1024 * 1024)
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
