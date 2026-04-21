use crate::scanner::{self, SystemHealth};
use std::sync::Arc;
use std::time::Duration;
use sysinfo::System;
use tauri::{AppHandle, Emitter};

/// 启动后台监控线程：每 2 秒刷新系统健康数据并：
/// 1) emit "health:update" 事件给前端更新任何订阅的 UI
/// 2) 更新托盘 tooltip
///
/// 2 秒的间隔是权衡：更频繁 → CPU 占用上升；更稀疏 → 托盘响应滞后
pub fn start_background_monitor(app: AppHandle) {
    let handle = Arc::new(app);
    std::thread::spawn(move || {
        let mut sys = System::new();
        // 预热：sysinfo CPU 计算需要至少两次采样间隔
        sys.refresh_cpu_all();
        std::thread::sleep(Duration::from_millis(300));
        loop {
            sys.refresh_cpu_all();
            sys.refresh_memory();
            let health = scanner::read_health(&mut sys);

            // 推送给前端
            let _ = handle.emit("health:update", &health);

            // 更新托盘 tooltip
            update_tray_tooltip(&handle, &health);

            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

fn update_tray_tooltip(app: &AppHandle, h: &SystemHealth) {
    let tip = format!(
        "MacFlow · CPU {:>4.1}%  内存 {:>4.1}%  磁盘 {:>4.1}%",
        h.cpu_percent, h.memory_percent, h.disk_percent
    );
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(&tip));
    }
}
