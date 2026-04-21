use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::thread::sleep;
use std::time::Duration;

/// 优雅终止：先 SIGTERM，等 3 秒看进程是否退出；未退出则 SIGKILL。
/// 这就是原生 API（POSIX kill(2)）—— nix crate 只是 Rust 封装。
pub fn graceful_kill(pid: u32) -> bool {
    let p = Pid::from_raw(pid as i32);

    // 1. SIGTERM
    if kill(p, Signal::SIGTERM).is_err() {
        // 进程可能已经不在了 —— 视为成功
        return true;
    }

    // 2. 等 3 秒，分三次探测
    for _ in 0..6 {
        sleep(Duration::from_millis(500));
        // kill(pid, 0) 用于探测进程是否存在；Err(ESRCH) 表示进程已消失
        if kill(p, None).is_err() {
            return true;
        }
    }

    // 3. SIGKILL 强杀
    kill(p, Signal::SIGKILL).is_ok()
}
