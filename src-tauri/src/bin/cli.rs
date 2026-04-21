//! MacFlow CLI —— 与桌面端共享核心逻辑
//!
//! 用法：
//!   macflow-cli           扫描并一键清理所有安全项
//!   macflow-cli --scan    仅扫描，不清理
//!   macflow-cli --cache   仅清理开发者缓存
//!   macflow-cli --help    帮助

use macflow_lib::{cache_cleaner_clean, cache_scanner_scan, run_tauri, scanner_read_health};
use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        run_tauri();
        return;
    }

    match args[0].as_str() {
        "--help" | "-h" => print_help(),
        "--version" | "-V" => {
            println!("macflow-cli {}", env!("CARGO_PKG_VERSION"));
        }
        "--scan" => {
            run_scan(false);
        }
        "--cache" => {
            run_scan(true);
        }
        other => {
            eprintln!("未知参数: {}\n", other);
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        r#"MacFlow CLI {}

用法:
  macflow-cli              打开桌面应用（无参数）
  macflow-cli --scan       仅扫描，输出结果不清理
  macflow-cli --cache      扫描并清理安全的开发缓存
  macflow-cli --help       显示本帮助
  macflow-cli --version    显示版本"#,
        env!("CARGO_PKG_VERSION")
    );
}

fn run_scan(clean: bool) {
    println!("MacFlow CLI  v{}", env!("CARGO_PKG_VERSION"));
    println!("==========================================");

    // 系统健康
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    let h = scanner_read_health(&mut sys);
    println!(
        "系统状态:  CPU {:>4.1}%   内存 {:>4.1}%   磁盘 {:>4.1}%",
        h.cpu_percent, h.memory_percent, h.disk_percent
    );
    println!();

    // 缓存扫描
    println!("扫描开发者缓存...");
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let result = rt.block_on(cache_scanner_scan());

    if result.items.is_empty() {
        println!("  没有可清理的缓存 —— 系统已经很干净了");
        return;
    }

    let total_gb = result.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    println!();
    println!(
        "发现 {} 项可优化内容，共计 {:.2} GB",
        result.items.len(),
        total_gb
    );
    println!("------------------------------------------");
    for item in &result.items {
        let size_mb = item.size_bytes as f64 / 1024.0 / 1024.0;
        let mark = if item.default_select { "[*]" } else { "[ ]" };
        println!(
            "{} {:>8.0} MB  {}",
            mark, size_mb, item.label
        );
    }
    println!();

    if !clean {
        println!("提示: 加 --cache 参数可直接清理已标记 [*] 的项");
        return;
    }

    // 执行清理（仅默认选中项）
    let to_clean: Vec<_> = result
        .items
        .iter()
        .filter(|i| i.default_select)
        .cloned()
        .collect();

    if to_clean.is_empty() {
        println!("没有默认选中的安全项可清理");
        return;
    }

    println!("正在清理 {} 项...", to_clean.len());
    let summary = rt.block_on(cache_cleaner_clean(to_clean));

    let freed_gb = summary.total_freed_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    println!("------------------------------------------");
    println!(
        "成功 {} 项  失败 {} 项  共释放 {:.2} GB",
        summary.success_count, summary.fail_count, freed_gb
    );
    for r in &summary.reports {
        let status = if r.success { "OK  " } else { "FAIL" };
        println!(
            "  {} {:>6} ms  {}",
            status, r.duration_ms, r.label
        );
        if let Some(err) = &r.error {
            println!("            {}", err);
        }
    }
}

// 让编译器闭嘴：主二进制入口也来自 lib
#[allow(dead_code)]
fn _unused_main_entry() {
    run_tauri();
}
