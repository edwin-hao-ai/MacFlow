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

/// 进程管理视图用的行数据，信息比 scan 更丰富
#[derive(Serialize, Clone, Debug)]
pub struct ProcessRow {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub exe: String,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub uptime_secs: u64,
    pub status: String,
    pub ports: Vec<u16>,
    /// 是否受保护（系统核心 / 多进程族父进程 / 有子进程 / 跨用户）
    /// 受保护的进程用户能看到但终止按钮禁用
    pub protected: bool,
    pub protected_reason: Option<String>,
    /// 是否在用户白名单（基于名字）
    pub whitelisted: bool,
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

/// 列出当前用户所有进程，不做分类过滤，供进程管理页使用。
pub fn list_all(sys: &mut System) -> Vec<ProcessRow> {
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_all();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let parent_pids = collect_parent_pids(sys);
    let mut out: Vec<ProcessRow> = Vec::new();

    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_string();

        // 跨用户（如 root 服务）跳过 —— 用户根本无权终止
        if !is_same_user(proc) {
            continue;
        }
        // 低 PID 系统进程跳过
        if pid.as_u32() < 50 {
            continue;
        }

        let whitelisted = is_whitelisted(&name);
        let veto = safety_veto(proc, &name, &parent_pids);
        let (protected, protected_reason) = if whitelisted {
            (true, Some("系统核心 / 白名单，禁止终止".to_string()))
        } else if let Some(r) = veto {
            (true, Some(r.to_string()))
        } else {
            (false, None)
        };

        let exe = proc
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let status = match proc.status() {
            ProcessStatus::Idle => "闲置",
            ProcessStatus::Run => "运行",
            ProcessStatus::Sleep => "睡眠",
            ProcessStatus::Stop => "停止",
            ProcessStatus::Zombie => "僵尸",
            ProcessStatus::Tracing => "被追踪",
            ProcessStatus::Dead => "已死",
            ProcessStatus::Wakekill => "唤醒中",
            ProcessStatus::Waking => "唤醒",
            ProcessStatus::Parked => "暂停",
            ProcessStatus::LockBlocked => "锁阻塞",
            ProcessStatus::UninterruptibleDiskSleep => "磁盘睡眠",
            _ => "未知",
        }
        .to_string();

        out.push(ProcessRow {
            pid: pid.as_u32(),
            parent_pid: proc.parent().map(|p| p.as_u32()),
            name,
            exe,
            cpu_percent: proc.cpu_usage(),
            memory_mb: proc.memory() as f64 / 1024.0 / 1024.0,
            uptime_secs: crate::process_safety::process_uptime_secs(proc),
            status,
            ports: Vec::new(),
            protected,
            protected_reason,
            whitelisted,
        });
    }

    // 附加端口信息
    let pids: Vec<u32> = out.iter().map(|p| p.pid).collect();
    let port_map = crate::ports::ports_by_pid(&pids);
    for p in out.iter_mut() {
        if let Some(ports) = port_map.get(&p.pid) {
            p.ports = ports.clone();
        }
    }

    // 默认按内存降序 —— 让用户一眼看到大户
    out.sort_by(|a, b| {
        b.memory_mb
            .partial_cmp(&a.memory_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
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
    let total_mem_mb = sys.total_memory() as f64 / 1024.0 / 1024.0;
    let mut out: Vec<ProcessInfo> = Vec::new();

    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_string();

        if is_whitelisted(&name) {
            continue;
        }
        if pid.as_u32() < 100 {
            continue;
        }
        if !is_same_user(proc) {
            continue;
        }

        let classification = classify_one(proc, &name, &parent_pids, total_mem_mb);
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
    total_mem_mb: f64,
) -> Classification {
    let cpu = proc.cpu_usage();
    let mem_mb = proc.memory() as f64 / 1024.0 / 1024.0;
    let status = proc.status();

    // 动态门槛：按系统总内存的百分比（避免在 8GB 机器上门槛过高）
    let hog_mem_threshold = (total_mem_mb * 0.05).max(400.0); // 8GB→400MB, 32GB→1.6GB
    let idle_mem_threshold = (total_mem_mb * 0.02).max(150.0); // 8GB→163MB

    // —— 第一优先：僵尸进程，100% 可清理 ——
    if matches!(status, ProcessStatus::Zombie) {
        return Classification {
            kind: ProcessKind::Zombie,
            risk: Risk::Safe,
            default_select: true,
            reason: "僵尸进程（已退出，等待父进程回收）".into(),
        };
    }

    // —— 安全审计：触发 veto 的降级为「展示但不可默认选中」而非完全隐藏 ——
    //    这样用户能看到 Chrome / Electron 应用的内存占用，但不会误杀
    let vetoed = safety_veto(proc, name, parent_pids);
    let is_mp_family = is_multiprocess_family(name);

    if let Some(veto_reason) = vetoed {
        // 年轻进程 / 跨用户 —— 不可能是清理目标，隐藏
        if veto_reason.contains("不足") || veto_reason.contains("进程刚启动") {
            return Classification {
                kind: ProcessKind::Foreground,
                risk: Risk::Hidden,
                default_select: false,
                reason: String::new(),
            };
        }

        // 父进程 / 多进程族 —— 仍然展示（用户需要感知内存占用），但固定 Low 风险 + 不默认选中
        // 只有显著的才显示（避免一堆小的 Chrome Helper 刷屏）
        if mem_mb >= hog_mem_threshold || cpu >= 20.0 {
            let hint = if is_mp_family {
                "多进程应用组件"
            } else {
                "应用主进程"
            };
            return Classification {
                kind: ProcessKind::Hog,
                risk: Risk::Low,
                default_select: false,
                reason: format!(
                    "{} · {:.0}MB · {:.1}% CPU（{}，仅供参考，清理会导致应用崩溃）",
                    name, mem_mb, cpu, hint
                ),
            };
        }
        return Classification {
            kind: ProcessKind::Foreground,
            risk: Risk::Hidden,
            default_select: false,
            reason: String::new(),
        };
    }

    // —— 以下都通过了安全审计 ——

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
        return Classification {
            kind: ProcessKind::Dev,
            risk: Risk::Dev,
            default_select: false,
            reason: format!("{} 开发工具进程（建议手动确认）", name),
        };
    }

    // 资源大户：显著 CPU 或内存占用 —— Low 风险，不默认选中
    if cpu > 20.0 {
        return Classification {
            kind: ProcessKind::Hog,
            risk: Risk::Low,
            default_select: false,
            reason: format!("CPU 占用 {:.1}%", cpu),
        };
    }
    if mem_mb >= hog_mem_threshold {
        return Classification {
            kind: ProcessKind::Hog,
            risk: Risk::Low,
            default_select: false,
            reason: format!("内存占用 {:.0}MB", mem_mb),
        };
    }

    // 长期闲置：低活动 + 中等内存 + 已运行较久（门槛降到 20 分钟）
    let uptime_min = crate::process_safety::process_uptime_secs(proc) / 60;
    if cpu < 0.5 && mem_mb >= idle_mem_threshold && uptime_min > 20 {
        return Classification {
            kind: ProcessKind::Idle,
            risk: Risk::Low,
            default_select: false,
            reason: format!("已运行 {} 分钟 · {:.0}MB 无活动", uptime_min, mem_mb),
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
    fn chrome_helpers_never_default_selected() {
        // 注：多进程族进程现在允许显示（作为 Low 风险，方便用户看到内存占用）
        // 但绝不允许默认选中。
        let mut sys = System::new_all();
        let result = scan(&mut sys);

        for p in &result.processes {
            let is_mp = p.name.contains("Chrome Helper")
                || p.name.contains("Slack Helper")
                || p.name.starts_with("Code Helper")
                || p.name.contains("Electron Helper");
            if is_mp {
                assert!(
                    !p.default_select,
                    "多进程族 Helper {} 不应默认选中",
                    p.name
                );
                assert_ne!(
                    p.risk,
                    Risk::Safe,
                    "多进程族 Helper {} 不应标记为 Safe",
                    p.name
                );
            }
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
