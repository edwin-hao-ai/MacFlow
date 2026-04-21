use crate::process_safety::{
    collect_parent_pids, is_multiprocess_family, is_same_user, safety_veto,
};
use crate::whitelist::is_whitelisted;
use serde::Serialize;
use sysinfo::{Disks, Process, ProcessStatus, System};

#[derive(Serialize, Clone, Debug)]
pub struct SystemHealth {
    pub cpu_percent: f32,
    pub memory_used_mb: f64,
    pub memory_total_mb: f64,
    pub memory_percent: f32,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub disk_percent: f32,
}

#[derive(Serialize, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub kind: ProcessKind,
    pub risk: Risk,
    pub default_select: bool,
    pub reason: String,
    pub ports: Vec<u16>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ProcessKind {
    /// 僵尸进程（父进程已退出，内核未回收）
    Zombie,
    /// 长期闲置（低 CPU + 中等内存 + 老进程）
    Idle,
    /// 资源大户（CPU 或内存显著）
    Hog,
    /// 开发工具闲置
    Dev,
    System,
    Foreground,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// 100% 安全 —— 只有僵尸进程能拿到这个等级
    Safe,
    /// 低风险 —— 大概率可清理，但保守起见不默认选中
    Low,
    /// 开发工具 —— 用户需主动判断
    Dev,
    /// 不显示给用户
    Hidden,
}

#[derive(Serialize, Clone, Debug)]
pub struct ScanResult {
    pub health: SystemHealth,
    pub processes: Vec<ProcessInfo>,
    pub scanned_at_ms: u64,
}

pub fn read_health(sys: &mut System) -> SystemHealth {
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let cpu_percent = sys.global_cpu_usage();
    let mem_total = sys.total_memory() as f64 / 1024.0 / 1024.0;
    let mem_used = sys.used_memory() as f64 / 1024.0 / 1024.0;
    let mem_pct = if mem_total > 0.0 {
        (mem_used / mem_total * 100.0) as f32
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let (disk_total, disk_avail) = disks
        .iter()
        .filter(|d| d.mount_point().to_string_lossy() == "/")
        .map(|d| (d.total_space(), d.available_space()))
        .next()
        .unwrap_or((0, 0));
    let disk_used = disk_total.saturating_sub(disk_avail);
    let gb = 1024u64.pow(3) as f64;
    let disk_pct = if disk_total > 0 {
        (disk_used as f64 / disk_total as f64 * 100.0) as f32
    } else {
        0.0
    };

    SystemHealth {
        cpu_percent,
        memory_used_mb: mem_used,
        memory_total_mb: mem_total,
        memory_percent: mem_pct,
        disk_used_gb: disk_used as f64 / gb,
        disk_total_gb: disk_total as f64 / gb,
        disk_percent: disk_pct,
    }
}

pub fn scan(sys: &mut System) -> ScanResult {
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let health = read_health(sys);
    let processes = classify_processes(sys);

    let scanned_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    ScanResult {
        health,
        processes,
        scanned_at_ms,
    }
}

/// 进程分类（保守原则）
///
/// 流程：
/// 1. 跳过系统核心、白名单、低 PID、非当前用户的进程
/// 2. 进入安全审计层（process_safety::safety_veto）—— 多进程族 / 父进程 / 年轻进程全部淘汰
/// 3. 剩余进程按状态分类：僵尸 > 开发工具闲置 > 资源大户 > 长期闲置
/// 4. **只有僵尸进程**能 default_select = true
fn classify_processes(sys: &System) -> Vec<ProcessInfo> {
    let parent_pids = collect_parent_pids(sys);
    let mut out: Vec<ProcessInfo> = Vec::new();

    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_string();

        // 闸门 1：核心白名单（系统关键进程）
        if is_whitelisted(&name) {
            continue;
        }
        // 闸门 2：低 PID（通常是系统进程）
        if pid.as_u32() < 100 {
            continue;
        }
        // 闸门 3：跨用户进程不碰（root 后台服务等）
        if !is_same_user(proc) {
            continue;
        }

        let classification = classify_one(proc, &name, &parent_pids);
        // Hidden 不展示
        if classification.risk == Risk::Hidden {
            continue;
        }

        let exe = proc
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        out.push(ProcessInfo {
            pid: pid.as_u32(),
            name,
            exe,
            cpu_percent: proc.cpu_usage(),
            memory_mb: proc.memory() as f64 / 1024.0 / 1024.0,
            kind: classification.kind,
            risk: classification.risk,
            default_select: classification.default_select,
            reason: classification.reason,
            ports: Vec::new(),
        });
    }

    // 按内存降序
    out.sort_by(|a, b| {
        b.memory_mb
            .partial_cmp(&a.memory_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(40);

    // 端口占用
    let pids: Vec<u32> = out.iter().map(|p| p.pid).collect();
    let port_map = crate::ports::ports_by_pid(&pids);
    for p in out.iter_mut() {
        if let Some(ports) = port_map.get(&p.pid) {
            p.ports = ports.clone();
            // 铁律：有监听端口的一律不默认选中（可能是运行中的服务）
            p.default_select = false;
            p.reason = format!(
                "{} · 端口 {}（运行中的服务，请确认）",
                p.reason,
                ports_preview(ports)
            );
        }
    }

    out
}

struct Classification {
    kind: ProcessKind,
    risk: Risk,
    default_select: bool,
    reason: String,
}

fn classify_one(
    proc: &Process,
    name: &str,
    parent_pids: &std::collections::HashSet<u32>,
) -> Classification {
    let cpu = proc.cpu_usage();
    let mem_mb = proc.memory() as f64 / 1024.0 / 1024.0;
    let status = proc.status();

    // —— 第一优先：僵尸进程，100% 可清理 ——
    // 即使僵尸进程的「名字」匹配多进程族，也已经死了，仍安全
    if matches!(status, ProcessStatus::Zombie) {
        return Classification {
            kind: ProcessKind::Zombie,
            risk: Risk::Safe,
            default_select: true,
            reason: "僵尸进程（已退出，等待父进程回收）".into(),
        };
    }

    // —— 安全审计：任意一条命中 → 绝对不碰 ——
    if let Some(reason) = safety_veto(proc, name, parent_pids) {
        // 多进程族应用 Helper 类型的，完全隐藏不烦用户
        if is_multiprocess_family(name) {
            return Classification {
                kind: ProcessKind::Foreground,
                risk: Risk::Hidden,
                default_select: false,
                reason: reason.to_string(),
            };
        }
        // 其他被否决的（年轻 / 父进程）也隐藏
        let _ = reason;
        return Classification {
            kind: ProcessKind::Foreground,
            risk: Risk::Hidden,
            default_select: false,
            reason: String::new(),
        };
    }

    // —— 以下都通过了安全审计 ——

    // 开发工具闲置：node / python / ruby / java 等
    let exe = proc
        .exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let is_dev = exe.contains("/node_modules/")
        || matches!(
            name,
            "node" | "python" | "python3" | "ruby" | "java" | "rustc" | "go" | "bun" | "deno"
        );
    if is_dev {
        // 开发工具即使闲置也默认不选 —— 可能是用户的 dev server
        return Classification {
            kind: ProcessKind::Dev,
            risk: Risk::Dev,
            default_select: false,
            reason: format!("{} 开发工具进程（建议手动确认）", name),
        };
    }

    // 资源大户：显著 CPU 或内存占用 —— Low 风险，不默认选中
    if cpu > 30.0 {
        return Classification {
            kind: ProcessKind::Hog,
            risk: Risk::Low,
            default_select: false,
            reason: format!("CPU 占用 {:.1}%", cpu),
        };
    }
    if mem_mb > 800.0 {
        return Classification {
            kind: ProcessKind::Hog,
            risk: Risk::Low,
            default_select: false,
            reason: format!("内存占用 {:.0}MB", mem_mb),
        };
    }

    // 长期闲置：低活动 + 中等内存 + 已运行较久
    // 这里要求运行时间 > 30 分钟（比 safety_veto 的 10 分钟更严）
    let uptime_min = crate::process_safety::process_uptime_secs(proc) / 60;
    if cpu < 0.2 && mem_mb > 200.0 && uptime_min > 30 {
        return Classification {
            kind: ProcessKind::Idle,
            risk: Risk::Low,
            default_select: false,
            reason: format!("已运行 {} 分钟，近期无活动", uptime_min),
        };
    }

    // 其他都隐藏
    Classification {
        kind: ProcessKind::Foreground,
        risk: Risk::Hidden,
        default_select: false,
        reason: String::new(),
    }
}

fn ports_preview(ports: &[u16]) -> String {
    if ports.len() <= 3 {
        ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("/")
    } else {
        format!(
            "{}/{}/{} 等 {} 个",
            ports[0],
            ports[1],
            ports[2],
            ports.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 对真实系统扫描结果的关键不变式检查 —— 在 CI 上跑，自证不误杀。
    #[test]
    fn real_scan_has_no_default_selected_non_zombie() {
        let mut sys = System::new_all();
        let result = scan(&mut sys);

        for p in &result.processes {
            if p.default_select {
                assert_eq!(
                    p.kind,
                    ProcessKind::Zombie,
                    "PID {} ({}) 被默认选中但不是僵尸进程！这会误杀用户进程！",
                    p.pid,
                    p.name
                );
            }
        }
    }

    #[test]
    fn real_scan_has_no_chrome_helpers_shown() {
        let mut sys = System::new_all();
        let result = scan(&mut sys);

        for p in &result.processes {
            assert!(
                !p.name.contains("Chrome Helper"),
                "Chrome Helper 不应出现在扫描结果里：{}",
                p.name
            );
            assert!(
                !p.name.contains("Slack Helper"),
                "Slack Helper 不应出现在扫描结果里：{}",
                p.name
            );
            assert!(
                !p.name.starts_with("Code Helper"),
                "VS Code Helper 不应出现：{}",
                p.name
            );
        }
    }

    #[test]
    fn real_scan_has_no_listening_port_processes_default_selected() {
        let mut sys = System::new_all();
        let result = scan(&mut sys);
        for p in &result.processes {
            if !p.ports.is_empty() {
                assert!(
                    !p.default_select,
                    "PID {} 监听端口 {:?} 但被默认选中！",
                    p.pid, p.ports
                );
            }
        }
    }

    #[test]
    fn real_scan_results_are_reasonable() {
        let mut sys = System::new_all();
        let result = scan(&mut sys);
        // 结果上限 40
        assert!(result.processes.len() <= 40);
        // 每个进程都有 name
        for p in &result.processes {
            assert!(!p.name.is_empty());
            assert!(p.pid > 0);
        }
    }
}
