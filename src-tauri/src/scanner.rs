use crate::whitelist::is_whitelisted;
use serde::Serialize;
use sysinfo::{Disks, Pid, ProcessStatus, System};

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
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessKind {
    Residual,
    Duplicate,
    Idle,
    Hog,
    Dev,
    System,
    Foreground,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Safe,
    Low,
    Dev,
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

    // CPU: global utilization
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
    // 等一个采样周期让 CPU 读数有效
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

/// 进程分类规则（见 PRD §3.2.2）。
/// 规则驱动，无 AI。
fn classify_processes(sys: &System) -> Vec<ProcessInfo> {
    let mut name_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for proc in sys.processes().values() {
        let n = proc.name().to_string_lossy().to_string();
        *name_counts.entry(n).or_insert(0) += 1;
    }

    let mut out: Vec<ProcessInfo> = Vec::new();

    for (pid, proc) in sys.processes() {
        let name = proc.name().to_string_lossy().to_string();
        let exe = proc
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let cpu = proc.cpu_usage();
        let mem_mb = proc.memory() as f64 / 1024.0 / 1024.0;
        let status = proc.status();

        // 白名单 / 系统核心 —— 完全跳过
        if is_whitelisted(&name) {
            continue;
        }
        // PID <= 100 通常是系统进程
        if pid.as_u32() < 100 {
            continue;
        }

        let (kind, risk, default_select, reason) = classify_one(
            pid,
            &name,
            &exe,
            cpu,
            mem_mb,
            status,
            *name_counts.get(&name).unwrap_or(&1),
        );

        // hidden 风险的不展示
        if risk == Risk::Hidden {
            continue;
        }

        out.push(ProcessInfo {
            pid: pid.as_u32(),
            name,
            exe,
            cpu_percent: cpu,
            memory_mb: mem_mb,
            kind,
            risk,
            default_select,
            reason,
        });
    }

    // 排序：按内存降序，方便用户看到大户
    out.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap_or(std::cmp::Ordering::Equal));
    // 最多展示 40 项，避免信息过载
    out.truncate(40);
    out
}

fn classify_one(
    _pid: &Pid,
    name: &str,
    exe: &str,
    cpu: f32,
    mem_mb: f64,
    status: ProcessStatus,
    name_count: u32,
) -> (ProcessKind, Risk, bool, String) {
    // 僵尸 / 孤儿 -> residual (safe)
    if matches!(status, ProcessStatus::Zombie) {
        return (
            ProcessKind::Residual,
            Risk::Safe,
            true,
            "僵尸进程，已无父进程".into(),
        );
    }

    // 开发者工具相关
    let is_dev = exe.contains("/node_modules/")
        || name.contains("node")
        || name == "node"
        || name == "python"
        || name == "python3"
        || name == "java"
        || name.contains("docker")
        || name.contains("ruby")
        || name.contains("rustc")
        || name.contains("cargo");
    if is_dev && cpu < 1.0 && mem_mb > 50.0 {
        return (
            ProcessKind::Dev,
            Risk::Dev,
            false, // 开发进程不默认选中，让用户主动选
            format!("{} 开发进程闲置中（CPU < 1%）", name),
        );
    }

    // 重复进程
    if name_count >= 3 && cpu < 2.0 && mem_mb > 80.0 {
        return (
            ProcessKind::Duplicate,
            Risk::Safe,
            true,
            format!("发现 {} 个同名进程，可合并", name_count),
        );
    }

    // 高占用闲置（CPU 低但内存高 / 反之）
    if cpu > 20.0 && mem_mb < 50.0 {
        return (
            ProcessKind::Hog,
            Risk::Low,
            false,
            format!("CPU 占用 {:.1}% 且持续升高", cpu),
        );
    }
    if mem_mb > 500.0 && cpu < 0.5 {
        return (
            ProcessKind::Hog,
            Risk::Low,
            false,
            format!("内存占用 {:.0}MB，长时间无活动", mem_mb),
        );
    }

    // 长期闲置
    if cpu < 0.2 && mem_mb > 150.0 {
        return (
            ProcessKind::Idle,
            Risk::Low,
            false,
            format!("长期闲置，占用 {:.0}MB", mem_mb),
        );
    }

    // 其他 -> 隐藏（不展示给用户）
    (
        ProcessKind::Foreground,
        Risk::Hidden,
        false,
        String::new(),
    )
}
