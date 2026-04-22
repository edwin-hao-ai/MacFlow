use crate::scanner::{self, SystemHealth};
use crate::tray;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::System;
use tauri::{AppHandle, Emitter};

/// 启动后台监控线程：每 2 秒刷新系统健康数据并：
/// 1) emit "health:update" 事件给前端更新任何订阅的 UI
/// 2) 更新托盘 title（CPU%）、tooltip、菜单项
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

            // 前端更新
            let _ = handle.emit("health:update", &health);

            // 托盘更新（title + tooltip + 菜单项）
            tray::refresh_tray(&handle, &health);

            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

#[allow(dead_code)]
fn _type_check(_: &SystemHealth) {}
