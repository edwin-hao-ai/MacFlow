//! 进程安全审计层 —— 所有会导致「误杀用户进程」的规则集中在这里。
//!
//! 设计原则：
//! 1. 「看起来像垃圾」≠「可以终止」。宁可保守放过一百个，也绝不错杀一个。
//! 2. 只有**完全没用**的进程才允许默认选中（目前只有僵尸 + 僵死孤儿）。
//! 3. 多进程族应用（Chrome / Electron / IDE 等）的子进程永远隐藏或标为 Low 风险。
//! 4. 有子进程的进程绝不碰 —— 它是某个活动应用的主进程。
//! 5. 新启动（< 10 分钟）的进程一律跳过 —— 用户刚启动的工具不算「残留」。

use std::collections::HashSet;
use sysinfo::{Process, System};

/// 已知多进程架构的应用族 —— 名字中包含这些子串的都是正常的多进程设计，
/// 绝不能因为「同名多实例」就当重复清理。
///
/// 数据基于 2026 年 macOS 上主流应用的进程命名惯例。
pub const MULTIPROCESS_FAMILIES: &[&str] = &[
    // Chrome / Chromium 系（包含所有基于 Chromium 的浏览器和 Electron 应用）
    "Google Chrome Helper",
    "Google Chrome Helper (Renderer)",
    "Google Chrome Helper (GPU)",
    "Google Chrome Helper (Plugin)",
    "Chromium Helper",
    "Microsoft Edge Helper",
    "Brave Browser Helper",
    "Arc Helper",
    "Opera Helper",
    "Vivaldi Helper",
    // Electron 通用
    "Electron Helper",
    "Electron Helper (Renderer)",
    "Electron Helper (GPU)",
    "Electron Helper (Plugin)",
    // VS Code / Cursor / Windsurf / Fork / Zed
    "Code Helper",
    "Code Helper (Renderer)",
    "Code Helper (GPU)",
    "Code Helper (Plugin)",
    "Code - Insiders Helper",
    "Cursor Helper",
    "Windsurf Helper",
    "Zed Helper",
    // 通讯类 Electron 应用
    "Slack Helper",
    "Slack Helper (Renderer)",
    "Slack Helper (GPU)",
    "Discord Helper",
    "Discord Helper (Renderer)",
    "Discord Helper (GPU)",
    "WhatsApp Helper",
    "Telegram",
    "QQ",
    "WeChat",
    "WeWorkMac",
    "DingTalk",
    "Lark",
    "Feishu",
    "飞书",
    // 笔记 / 文档类
    "Notion Helper",
    "Obsidian Helper",
    "Logseq Helper",
    "Craft Helper",
    "Linear Helper",
    "Figma Helper",
    "Raycast",
    "Alfred",
    // AI 客户端
    "ChatGPT Helper",
    "Claude Helper",
    "Perplexity Helper",
    // 浏览器主进程本身（虽然是单例，但不可当成残留）
    "Google Chrome",
    "Chromium",
    "Microsoft Edge",
    "Brave Browser",
    "Safari",
    "Arc",
    "Firefox",
    // 其他 Apple/macOS 多进程但不在核心白名单里的
    "com.apple.WebKit.WebContent",
    "com.apple.WebKit.Networking",
    "com.apple.WebKit.GPU",
    // 开发工具
    "docker",
    "com.docker.backend",
    "com.docker.build",
    "com.docker.dev-envs",
    "com.docker.virtualization",
    "Docker Desktop",
    "Docker Desktop Backend",
    // 音视频 / 娱乐
    "Spotify",
    "Spotify Helper",
    "Music",
    "Apple TV",
    // 开发语言运行时（可能被用户正在跑）
    "node",
    "python",
    "python3",
    "ruby",
    "java",
    "php",
    "perl",
    "bun",
    "deno",
    "rustc",
    "go",
    "dotnet",
];

/// 判断进程是否属于多进程族（Helper / 子进程架构）。
pub fn is_multiprocess_family(name: &str) -> bool {
    MULTIPROCESS_FAMILIES.iter().any(|f| {
        // 精准匹配 + 子串匹配（兼容 "Google Chrome Helper (Renderer)" 和 "Google Chrome Helper"）
        name == *f || name.starts_with(f) || name.contains(f)
    })
}

/// 给定 System 快照，返回所有有子进程的 PID 集合。
/// 「父进程」= 在当前快照里，有任何其他进程的 parent_pid 指向它。
pub fn collect_parent_pids(sys: &System) -> HashSet<u32> {
    let mut out = HashSet::new();
    for proc in sys.processes().values() {
        if let Some(ppid) = proc.parent() {
            out.insert(ppid.as_u32());
        }
    }
    out
}

/// 获取进程的运行时长（秒）。失败返回 0。
pub fn process_uptime_secs(proc: &Process) -> u64 {
    let started = proc.start_time(); // Unix epoch seconds
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(started)
}

/// 「年轻」进程 —— 运行 < 10 分钟的任何进程都视为用户刚启动的东西，不清理。
pub fn is_young_process(proc: &Process) -> bool {
    process_uptime_secs(proc) < 600
}

/// 最终安全判断 —— 任意一条命中就表示「不能默认选中」。
///
/// 返回 Some(原因) 表示必须降级/隐藏，None 表示通过安全审计。
pub fn safety_veto(
    proc: &Process,
    name: &str,
    parent_pids: &HashSet<u32>,
) -> Option<&'static str> {
    // 1. 该进程是别人的父进程 —— 说明它是某应用的主进程，永远不碰
    if parent_pids.contains(&proc.pid().as_u32()) {
        return Some("是其他进程的父进程（某应用的主进程）");
    }

    // 2. 多进程架构应用的 Helper —— Chrome / Electron / IDE 等
    if is_multiprocess_family(name) {
        return Some("已知多进程架构应用的组件，属正常设计");
    }

    // 3. 运行时间过短 —— 可能是用户刚启动的
    if is_young_process(proc) {
        return Some("进程刚启动不足 10 分钟，可能是用户正在使用");
    }

    // 4. 有活跃 IO / 文件句柄 太多 —— sysinfo 无直接支持；交给 port 检测层做类似效果

    None
}

/// PID 是否属于当前用户 —— 跨用户的进程一律不碰。
pub fn is_same_user(proc: &Process) -> bool {
    let current = nix::unistd::Uid::effective().as_raw();
    match proc.user_id() {
        Some(uid) => **uid == current,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_helper_is_multiprocess() {
        assert!(is_multiprocess_family("Google Chrome Helper"));
        assert!(is_multiprocess_family("Google Chrome Helper (Renderer)"));
        assert!(is_multiprocess_family("Google Chrome Helper (GPU)"));
    }

    #[test]
    fn electron_apps_are_multiprocess() {
        assert!(is_multiprocess_family("Slack Helper"));
        assert!(is_multiprocess_family("Discord Helper (Renderer)"));
        assert!(is_multiprocess_family("Code Helper"));
        assert!(is_multiprocess_family("Cursor Helper"));
    }

    #[test]
    fn cjk_communications_apps() {
        assert!(is_multiprocess_family("Lark"));
        assert!(is_multiprocess_family("Feishu"));
        assert!(is_multiprocess_family("WeChat"));
        assert!(is_multiprocess_family("飞书"));
    }

    #[test]
    fn random_process_not_multiprocess() {
        assert!(!is_multiprocess_family("my-custom-daemon"));
        assert!(!is_multiprocess_family("random-script"));
    }

    #[test]
    fn browser_main_process_also_protected() {
        assert!(is_multiprocess_family("Google Chrome"));
        assert!(is_multiprocess_family("Safari"));
        assert!(is_multiprocess_family("Firefox"));
    }
}
