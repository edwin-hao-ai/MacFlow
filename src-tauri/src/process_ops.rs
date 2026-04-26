use nix::errno::Errno;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::collections::HashSet;
use std::thread::sleep;
use std::time::Duration;
use sysinfo::System;

/// 终止一个进程的结果
#[derive(Debug, Clone)]
pub enum KillOutcome {
    Success,
    AlreadyGone,
    PermissionDenied,
    /// 目标 PID 已死，但同名同路径的进程以新 PID 重新出现 —— 说明有 supervisor 守护
    RespawnedAs {
        new_pid: u32,
        name: String,
    },
    /// SIGKILL 发了进程还在（非常罕见，一般只有僵死或受保护才会这样）
    StillAlive,
    Failed(String),
}

impl KillOutcome {
    pub fn is_ok(&self) -> bool {
        matches!(self, KillOutcome::Success | KillOutcome::AlreadyGone)
    }
    pub fn message(&self) -> String {
        match self {
            KillOutcome::Success => "已终止".into(),
            KillOutcome::AlreadyGone => "进程已不存在".into(),
            KillOutcome::PermissionDenied => {
                "权限不足（通常是 root 或系统进程，MacSlim 不应该看到这类进程）".into()
            }
            KillOutcome::RespawnedAs { new_pid, name } => format!(
                "原进程已终止，但一个 supervisor 立刻以新 PID {} 重启了 `{}`。\
                 请从上游启动器（launchd agent / pm2 / nvm / Cursor / VS Code 等）停止，\
                 或把此进程名加入白名单屏蔽显示。",
                new_pid, name
            ),
            KillOutcome::StillAlive => {
                "SIGKILL 已发送，但系统报告进程仍存活。可能是僵死进程或受内核保护。".into()
            }
            KillOutcome::Failed(e) => format!("失败: {}", e),
        }
    }
}

/// 优雅终止：
/// 1. 收集整个进程子树（pid 自己 + 所有后代）
/// 2. 从叶子往上 SIGTERM，避免父进程重启子进程
/// 3. 等 3 秒
/// 4. 仍存活的再 SIGKILL
///
/// 这是 macOS / Linux 通用的「杀进程树」做法。npm / pnpm / vite / pm2 等
/// supervisor 启动的 node，单独杀 node 会被立刻 respawn，必须连 supervisor
/// 一起杀（或者用户要求）。这里我们选择**杀整棵子树**而保留父进程 —— 这样
/// 父进程得到 SIGCHLD 之后就会自然退出或进入等待态，不会反弹。
pub fn graceful_kill(pid: u32) -> KillOutcome {
    let mut sys = System::new();
    sys.refresh_all();

    let target = Pid::from_raw(pid as i32);
    if !process_exists(target) {
        return KillOutcome::AlreadyGone;
    }

    // 记下目标进程的 name / exe，之后用来判断是否被 supervisor 重启
    // 注意：sysinfo::Pid 不同于 nix::unistd::Pid
    let target_name: Option<String>;
    let target_exe: Option<std::path::PathBuf>;
    {
        let p = sys.process(sysinfo::Pid::from_u32(pid));
        target_name = p.map(|proc| proc.name().to_string_lossy().to_string());
        target_exe = p.and_then(|proc| proc.exe().map(|e| e.to_path_buf()));
    }

    // 收集整棵子树（含自身）
    let tree = collect_descendants(&sys, pid);
    let mut ordered: Vec<u32> = tree.iter().copied().collect();
    ordered.sort_by(|a, b| {
        let da = depth(&sys, *a);
        let db = depth(&sys, *b);
        db.cmp(&da)
    });

    // 第一轮：SIGTERM（从叶子往上发）
    for p in &ordered {
        let np = Pid::from_raw(*p as i32);
        match kill(np, Signal::SIGTERM) {
            Ok(_) | Err(Errno::ESRCH) => {}
            Err(Errno::EPERM) => return KillOutcome::PermissionDenied,
            Err(_) => {}
        }
    }

    // 等 3 秒判断目标是否消失
    let mut target_gone = false;
    for _ in 0..6 {
        sleep(Duration::from_millis(500));
        if !process_exists(target) {
            target_gone = true;
            break;
        }
    }

    // 目标还没死 → SIGKILL 子树
    if !target_gone {
        for p in &ordered {
            let np = Pid::from_raw(*p as i32);
            let _ = kill(np, Signal::SIGKILL);
        }
        sleep(Duration::from_secs(1));
        target_gone = !process_exists(target);
    }

    if !target_gone {
        // SIGKILL 都失败了（极罕见）
        return KillOutcome::StillAlive;
    }

    // 目标进程确实死了 —— 但看 supervisor 有没有立刻以新 PID 拉起同名进程
    // 给 supervisor 一点时间（2 秒）复活
    sleep(Duration::from_millis(1200));
    let respawn = detect_respawn(pid, target_name.as_deref(), target_exe.as_deref());
    if let Some((new_pid, name)) = respawn {
        return KillOutcome::RespawnedAs { new_pid, name };
    }

    KillOutcome::Success
}

/// 判断是否有同名同路径的新进程冒出来（supervisor 重启）
fn detect_respawn(
    old_pid: u32,
    name: Option<&str>,
    exe: Option<&std::path::Path>,
) -> Option<(u32, String)> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    for (pid, proc) in sys.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 == old_pid {
            continue; // 同 PID 不算重启
        }
        let pname = proc.name().to_string_lossy().to_string();
        let pexe = proc.exe();

        // name + exe 都一致才算重启（避免把普通同名进程误判为重启）
        let name_match = name.map(|n| n == pname).unwrap_or(false);
        let exe_match = match (exe, pexe) {
            (Some(a), Some(b)) => a == b,
            // 有一个没路径信息 → 只匹配 name 也算
            _ => name_match,
        };

        if name_match && exe_match {
            // 为避免匹配到已经运行很久的同名进程（巧合），
            // 要求新进程启动时间距「kill 时刻」很近（最近 3 秒内）
            let started = proc.start_time();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now.saturating_sub(started) < 3 {
                return Some((pid_u32, pname));
            }
        }
    }
    None
}

/// 非破坏性探测：进程是否存在
fn process_exists(pid: Pid) -> bool {
    // kill(pid, 0) 是 POSIX 标准用法：不发信号但做权限检查
    // Ok(()) = 存在且有权限； ESRCH = 不存在； EPERM = 存在但无权限
    matches!(
        kill(pid, None),
        Ok(()) | Err(Errno::EPERM)
    )
}

/// 对给定 pid，收集它和它所有后代进程的 pid 集合
fn collect_descendants(sys: &System, root: u32) -> HashSet<u32> {
    let mut out = HashSet::new();
    out.insert(root);

    // BFS 扫父子关系
    let mut frontier = vec![root];
    while let Some(parent) = frontier.pop() {
        for proc in sys.processes().values() {
            if let Some(ppid) = proc.parent() {
                let ppid = ppid.as_u32();
                let child = proc.pid().as_u32();
                if ppid == parent && !out.contains(&child) {
                    out.insert(child);
                    frontier.push(child);
                }
            }
        }
    }
    out
}

/// 返回进程到「某祖先」的深度（粗略用）
fn depth(sys: &System, pid: u32) -> usize {
    let mut d = 0usize;
    let mut cur = pid;
    for _ in 0..32 {
        // 最多往上追 32 层
        let mut next: Option<u32> = None;
        for proc in sys.processes().values() {
            if proc.pid().as_u32() == cur {
                if let Some(p) = proc.parent() {
                    next = Some(p.as_u32());
                }
                break;
            }
        }
        match next {
            Some(p) if p != 0 && p != 1 => {
                cur = p;
                d += 1;
            }
            _ => break,
        }
    }
    d
}
