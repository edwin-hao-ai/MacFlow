use crate::scanner::SystemHealth;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

/// 持有菜单项引用，供 monitor 线程定期更新文字
pub struct TrayItems {
    pub cpu: Mutex<MenuItem<tauri::Wry>>,
    pub mem: Mutex<MenuItem<tauri::Wry>>,
    pub disk: Mutex<MenuItem<tauri::Wry>>,
    pub health_header: Mutex<MenuItem<tauri::Wry>>,
}

pub fn init_tray(app: &AppHandle) -> tauri::Result<()> {
    // 动态状态区（只读项，靠 monitor 线程刷新）
    let health_header =
        MenuItem::with_id(app, "health_header", "系统状态", false, None::<&str>)?;
    let cpu_item = MenuItem::with_id(app, "cpu_item", "  CPU:    —", false, None::<&str>)?;
    let mem_item = MenuItem::with_id(app, "mem_item", "  内存:   —", false, None::<&str>)?;
    let disk_item = MenuItem::with_id(app, "disk_item", "  磁盘:   —", false, None::<&str>)?;

    // 操作区
    let open_item = MenuItem::with_id(app, "open", "打开 MacFlow", true, None::<&str>)?;
    let scan_item = MenuItem::with_id(app, "scan", "立即扫描", true, None::<&str>)?;
    let optimize_item =
        MenuItem::with_id(app, "optimize", "一键优化（安全项）", true, None::<&str>)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let about_item =
        MenuItem::with_id(app, "about", "关于 MacFlow v0.1.0", false, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 MacFlow", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &health_header,
            &cpu_item,
            &mem_item,
            &disk_item,
            &sep1,
            &open_item,
            &scan_item,
            &optimize_item,
            &sep2,
            &about_item,
            &quit_item,
        ],
    )?;

    // 存入 app state 供 monitor 更新
    app.manage(TrayItems {
        cpu: Mutex::new(cpu_item),
        mem: Mutex::new(mem_item),
        disk: Mutex::new(disk_item),
        health_header: Mutex::new(health_header),
    });

    let _ = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .icon_as_template(true)
        .tooltip("MacFlow")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "scan" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("tray:scan", ());
                }
            }
            "optimize" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    // 前端监听 tray:optimize，执行默认选中项清理
                    let _ = window.emit("tray:optimize", ());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// 根据系统健康状态更新托盘的 title（菜单栏显示文字）+ tooltip + 菜单项。
/// 由 monitor::start_background_monitor 每 2 秒调用一次。
pub fn refresh_tray(app: &AppHandle, h: &SystemHealth) {
    // 1. 菜单栏 title：像 iStat Menus 一样显示 CPU%
    //    阈值：<60% 只显示图标不显示文字；>=60% 开始显示 CPU 数字；
    //    任一维度超过 90% 显示 "!" 警示
    let critical = h.cpu_percent >= 90.0
        || h.memory_percent >= 95.0
        || h.disk_percent >= 95.0;
    let warn = h.cpu_percent >= 60.0 || h.memory_percent >= 85.0 || h.disk_percent >= 90.0;

    let title_text = if critical {
        format!("! {:>3.0}%", h.cpu_percent.max(h.memory_percent))
    } else if warn {
        format!("{:>3.0}%", h.cpu_percent)
    } else {
        String::new() // 正常状态只显示图标
    };

    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_title(Some(title_text.clone()));
        let tip = format!(
            "MacFlow\nCPU {:.1}%   内存 {:.1}%   磁盘 {:.1}%",
            h.cpu_percent, h.memory_percent, h.disk_percent
        );
        let _ = tray.set_tooltip(Some(&tip));
    }

    // 2. 菜单项文字（带状态 emoji）
    if let Some(items) = app.try_state::<TrayItems>() {
        let state_dot = |p: f32, warn_at: f32, crit_at: f32| -> &'static str {
            if p >= crit_at {
                "●"
            } else if p >= warn_at {
                "●"
            } else {
                "●"
            }
        };
        // 用字符表示严重程度：不触发 emoji 渲染问题
        let fmt_line = |label: &str, pct: f32, warn_at: f32, crit_at: f32| -> String {
            let marker = state_dot(pct, warn_at, crit_at);
            format!("  {}  {}  {:>4.1}%", marker, label, pct)
        };

        if let Ok(i) = items.cpu.lock() {
            let _ = i.set_text(fmt_line("CPU  ", h.cpu_percent, 60.0, 90.0));
        }
        if let Ok(i) = items.mem.lock() {
            let _ = i.set_text(fmt_line("内存", h.memory_percent, 85.0, 95.0));
        }
        if let Ok(i) = items.disk.lock() {
            let _ = i.set_text(fmt_line("磁盘", h.disk_percent, 90.0, 95.0));
        }
        if let Ok(i) = items.health_header.lock() {
            let head = if critical {
                "系统状态 · 需要关注"
            } else if warn {
                "系统状态 · 运行中"
            } else {
                "系统状态 · 正常"
            };
            let _ = i.set_text(head);
        }
    }
}
