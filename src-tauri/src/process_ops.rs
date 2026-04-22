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
    Restarted, // SIGTERM 成功但进程还在（可能有 supervisor）
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
                "权限不足（通常是 root 或系统进程，MacFlow 不应该看到这类进程）".into()
            }
            KillOutcome::Restarted => {
                "已发送终止信号，但进程立刻被 supervisor 重启（如 npm / pm2 / launchd）。\
                 请从上游进程（例如 npm / pnpm / pm2 / launchctl）终止。"
                    .into()
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

    // 如果进程已经不存在，直接返回
    let target = Pid::from_raw(pid as i32);
    if !process_exists(target) {
        return KillOutcome::AlreadyGone;
    }

    // 收集整棵子树（含自身）
    let tree = collect_descendants(&sys, pid);

    // 第一轮：SIGTERM（从叶子往上发）
    let mut ordered: Vec<u32> = tree.iter().copied().collect();
    ordered.sort_by(|a, b| {
        let da = depth(&sys, *a);
        let db = depth(&sys, *b);
        db.cmp(&da) // 更深的（叶子）先杀
    });

    for p in &ordered {
        let np = Pid::from_raw(*p as i32);
        match kill(np, Signal::SIGTERM) {
            Ok(_) | Err(Errno::ESRCH) => {}
            Err(Errno::EPERM) => return KillOutcome::PermissionDenied,
            Err(e) => {
                // 其他错误记录但继续试
                let _ = e;
            }
        }
    }

    // 等 3 秒（分 6 次采样判断是否已清干净）
    for _ in 0..6 {
        sleep(Duration::from_millis(500));
        if !process_exists(target) {
            return KillOutcome::Success;
        }
    }

    // 第二轮：SIGKILL
    for p in &ordered {
        let np = Pid::from_raw(*p as i32);
        let _ = kill(np, Signal::SIGKILL);
    }

    // 最后再等 1 秒确认
    sleep(Duration::from_secs(1));
    if !process_exists(target) {
        return KillOutcome::Success;
    }

    // 都杀了还活着 → 99% 是被 supervisor 重启了
    if let Err(e) = kill(target, Signal::SIGKILL) {
        return KillOutcome::Failed(format!("SIGKILL 失败: {}", e));
    }

    // 最后再查一次：如果仍然在，只能是 supervisor restart
    sleep(Duration::from_millis(500));
    if process_exists(target) {
        KillOutcome::Restarted
    } else {
        KillOutcome::Success
    }
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
