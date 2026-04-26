//! MacSlim CLI —— 与桌面端共享核心逻辑
//!
//! 当前补齐的命令：
//!   macslim-cli --scan
//!   macslim-cli --cache
//!   macslim-cli --disk
//!   macslim-cli --npm
//!   macslim-cli --xcode
//!   macslim-cli --process [--scan]
//!   macslim-cli --list
//!   macslim-cli --docker [--scan]
//!   macslim-cli --history
//!   macslim-cli --whitelist list
//!   macslim-cli --whitelist add <kind> <value> [note]
//!   macslim-cli --whitelist remove <id>

use macslim_lib::cache_scanner::{CacheCategory, CacheItem};
use macslim_lib::process_ops::graceful_kill;
use macslim_lib::scanner::{self, ProcessInfo};
use macslim_lib::storage::{HistoryEntry, Storage};
use macslim_lib::{cache_cleaner_clean, cache_scanner_scan, scanner_read_health, run_tauri};
use std::env;

enum Command {
    Help,
    Version,
    Scan,
    Cache,
    Disk {
        scan_only: bool,
    },
    Npm {
        scan_only: bool,
    },
    Xcode {
        scan_only: bool,
    },
    Process { scan_only: bool },
    List,
    Docker { scan_only: bool },
    History,
    Whitelist(WhitelistCommand),
}

enum WhitelistCommand {
    List,
    Add {
        kind: String,
        value: String,
        note: String,
    },
    Remove {
        id: i64,
    },
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        run_tauri();
        return;
    }

    let command = match parse_args(&args) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("{}\n", err);
            print_help();
            std::process::exit(1);
        }
    };

    match command {
        Command::Help => print_help(),
        Command::Version => println!("macslim-cli {}", env!("CARGO_PKG_VERSION")),
        Command::Scan => run_cache_scan(None, false),
        Command::Cache => run_cache_scan(None, true),
        Command::Disk { scan_only } => run_cache_scan(None, !scan_only),
        Command::Npm { scan_only } => run_cache_scan(Some(CacheFilter::NodeFamily), !scan_only),
        Command::Xcode { scan_only } => run_cache_scan(Some(CacheFilter::Xcode), !scan_only),
        Command::Process { scan_only } => run_process(scan_only),
        Command::List => run_process(true),
        Command::Docker { scan_only } => {
            run_cache_scan(Some(CacheFilter::Single(CacheCategory::Docker)), !scan_only)
        }
        Command::History => run_history(),
        Command::Whitelist(cmd) => run_whitelist(cmd),
    }
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let has_scan = args.iter().any(|arg| arg == "--scan");
    match args[0].as_str() {
        "--help" | "-h" => Ok(Command::Help),
        "--version" | "-V" => Ok(Command::Version),
        "--scan" => Ok(Command::Scan),
        "--cache" => Ok(Command::Cache),
        "--disk" => Ok(Command::Disk {
            scan_only: has_scan,
        }),
        "--npm" => Ok(Command::Npm {
            scan_only: has_scan,
        }),
        "--xcode" => Ok(Command::Xcode {
            scan_only: has_scan,
        }),
        "--process" => Ok(Command::Process {
            scan_only: has_scan,
        }),
        "--list" => Ok(Command::List),
        "--docker" => Ok(Command::Docker {
            scan_only: has_scan,
        }),
        "--history" => Ok(Command::History),
        "--whitelist" => parse_whitelist_args(&args[1..]),
        other => Err(format!("未知参数: {}", other)),
    }
}

fn parse_whitelist_args(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Whitelist(WhitelistCommand::List));
    }

    match args[0].as_str() {
        "list" => Ok(Command::Whitelist(WhitelistCommand::List)),
        "add" => {
            if args.len() < 3 {
                return Err("用法：--whitelist add <kind> <value> [note]".into());
            }
            let kind = args[1].clone();
            let value = args[2].clone();
            let note = if args.len() > 3 {
                args[3..].join(" ")
            } else {
                String::new()
            };
            Ok(Command::Whitelist(WhitelistCommand::Add { kind, value, note }))
        }
        "remove" => {
            if args.len() < 2 {
                return Err("用法：--whitelist remove <id>".into());
            }
            let id = args[1]
                .parse::<i64>()
                .map_err(|_| format!("白名单 id 无效: {}", args[1]))?;
            Ok(Command::Whitelist(WhitelistCommand::Remove { id }))
        }
        other => Err(format!("未知的 whitelist 子命令: {}", other)),
    }
}

fn print_help() {
    println!(
        r#"MacSlim CLI {}

用法:
  macslim-cli              打开桌面应用（无参数）
  macslim-cli --scan       扫描全部缓存项，不执行清理
  macslim-cli --cache      清理默认选中的缓存项
  macslim-cli --disk       清理默认选中的硬盘类缓存项
  macslim-cli --disk --scan
                           仅扫描硬盘类缓存项
  macslim-cli --npm        仅扫描/清理 Node 生态缓存（NPM/PNPM/Yarn/node_modules）
  macslim-cli --npm --scan
                           仅扫描 Node 生态缓存
  macslim-cli --xcode      仅扫描/清理 Xcode 缓存
  macslim-cli --xcode --scan
                           仅扫描 Xcode 缓存
  macslim-cli --process    扫描并清理默认选中的进程项
  macslim-cli --process --scan
                           仅扫描进程项
  macslim-cli --list       列出所有可优化进程（等价于 --process --scan）
  macslim-cli --docker     扫描并清理默认选中的 Docker 项
  macslim-cli --docker --scan
                           仅扫描 Docker 项
  macslim-cli --history    查看最近清理历史
  macslim-cli --whitelist list
                           查看白名单
  macslim-cli --whitelist add <kind> <value> [note]
                           添加白名单（kind 通常为 process）
  macslim-cli --whitelist remove <id>
                           删除白名单
  macslim-cli --version    显示版本
  macslim-cli --help       显示帮助"#,
        env!("CARGO_PKG_VERSION")
    );
}

enum CacheFilter {
    Single(CacheCategory),
    NodeFamily,
    Xcode,
}

fn run_cache_scan(filter: Option<CacheFilter>, clean: bool) {
    print_header();
    print_health();

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut result = rt.block_on(cache_scanner_scan());
    if let Some(filter) = filter {
        result.items.retain(|item| cache_filter_match(item, &filter));
        result.total_bytes = result.items.iter().map(|item| item.size_bytes).sum();
    }

    if result.items.is_empty() {
        println!("没有匹配的可清理项。");
        return;
    }

    print_cache_items(&result.items);

    if !clean {
        println!("当前为仅扫描模式。");
        return;
    }

    let to_clean: Vec<CacheItem> = result
        .items
        .iter()
        .filter(|item| item.default_select)
        .cloned()
        .collect();

    if to_clean.is_empty() {
        println!("没有默认选中的安全项可清理。");
        return;
    }

    println!("开始清理 {} 项默认安全项...", to_clean.len());
    let summary = rt.block_on(cache_cleaner_clean(to_clean));
    let freed_gb = summary.total_freed_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    println!(
        "完成：成功 {} 项，失败 {} 项，估算释放 {:.2} GB",
        summary.success_count, summary.fail_count, freed_gb
    );
    for report in summary.reports {
        let status = if report.success { "OK" } else { "FAIL" };
        println!("  [{}] {} · {} ms", status, report.label, report.duration_ms);
        if let Some(error) = report.error {
            println!("       {}", error);
        }
    }
}

fn cache_filter_match(item: &CacheItem, filter: &CacheFilter) -> bool {
    match filter {
        CacheFilter::Single(category) => item.category == *category,
        CacheFilter::NodeFamily => matches!(
            item.category,
            CacheCategory::Npm | CacheCategory::Pnpm | CacheCategory::Yarn
        ) || item.id == "stale-node-modules",
        CacheFilter::Xcode => item.category == CacheCategory::Xcode,
    }
}

fn run_process(scan_only: bool) {
    print_header();
    print_health();

    let storage = Storage::open().ok();
    let mut sys = sysinfo::System::new_all();
    let mut result = scanner::scan(&mut sys);
    if let Some(storage) = &storage {
        result
            .processes
            .retain(|process| !storage.is_whitelisted("process", &process.name));
    }

    if result.processes.is_empty() {
        println!("没有匹配的进程优化项。");
        return;
    }

    print_process_items(&result.processes);

    if scan_only {
        println!("当前为仅扫描模式。");
        return;
    }

    let targets: Vec<ProcessInfo> = result
        .processes
        .iter()
        .filter(|process| process.default_select)
        .cloned()
        .collect();

    if targets.is_empty() {
        println!("没有默认选中的安全进程可终止。");
        return;
    }

    println!("开始处理 {} 个默认安全进程...", targets.len());
    let storage = storage;
    let mut success = 0usize;
    let mut failed = 0usize;

    for process in targets {
        let outcome = graceful_kill(process.pid);
        let ok = outcome.is_ok();
        let message = outcome.message();
        if ok {
            success += 1;
        } else {
            failed += 1;
        }
        println!(
            "  [{}] {} (PID {}) · {}",
            if ok { "OK" } else { "FAIL" },
            process.name,
            process.pid,
            message
        );
        if let Some(storage) = &storage {
            let _ = storage.log_history(
                "process_kill",
                &format!("{} (PID {})", process.name, process.pid),
                0,
                ok,
                &message,
            );
        }
    }

    println!("完成：成功 {} 个，失败 {} 个。", success, failed);
}

fn run_whitelist(command: WhitelistCommand) {
    let storage = match Storage::open() {
        Ok(storage) => storage,
        Err(err) => {
            eprintln!("打开存储失败: {}", err);
            std::process::exit(1);
        }
    };

    match command {
        WhitelistCommand::List => match storage.list_whitelist() {
            Ok(entries) if entries.is_empty() => println!("白名单为空。"),
            Ok(entries) => {
                println!("当前白名单：");
                for entry in entries {
                    println!(
                        "  [{}] {} = {}{}",
                        entry.id,
                        entry.kind,
                        entry.value,
                        if entry.note.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", entry.note)
                        }
                    );
                }
            }
            Err(err) => {
                eprintln!("读取白名单失败: {}", err);
                std::process::exit(1);
            }
        },
        WhitelistCommand::Add { kind, value, note } => {
            if let Err(err) = storage.add_whitelist(&kind, &value, &note) {
                eprintln!("添加白名单失败: {}", err);
                std::process::exit(1);
            }
            println!("已添加白名单：{} = {}", kind, value);
        }
        WhitelistCommand::Remove { id } => {
            if let Err(err) = storage.remove_whitelist(id) {
                eprintln!("删除白名单失败: {}", err);
                std::process::exit(1);
            }
            println!("已删除白名单项 {}", id);
        }
    }
}

fn run_history() {
    let storage = match Storage::open() {
        Ok(storage) => storage,
        Err(err) => {
            eprintln!("打开存储失败: {}", err);
            std::process::exit(1);
        }
    };

    match storage.recent_history(30) {
        Ok(entries) if entries.is_empty() => println!("还没有历史记录。"),
        Ok(entries) => print_history_entries(&entries),
        Err(err) => {
            eprintln!("读取历史失败: {}", err);
            std::process::exit(1);
        }
    }
}

fn print_header() {
    println!("MacSlim CLI v{}", env!("CARGO_PKG_VERSION"));
    println!("==========================================");
}

fn print_health() {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    let h = scanner_read_health(&mut sys);
    println!(
        "系统状态: CPU {:>4.1}%   内存 {:>4.1}%   磁盘 {:>4.1}%",
        h.cpu_percent, h.memory_percent, h.disk_percent
    );
    println!();
}

fn print_cache_items(items: &[CacheItem]) {
    let total_gb = items.iter().map(|item| item.size_bytes).sum::<u64>() as f64
        / 1024.0
        / 1024.0
        / 1024.0;
    println!("发现 {} 项，共计 {:.2} GB", items.len(), total_gb);
    println!("------------------------------------------");
    for item in items {
        let size_mb = item.size_bytes as f64 / 1024.0 / 1024.0;
        let mark = if item.default_select { "[*]" } else { "[ ]" };
        println!("{} {:>8.0} MB  {}", mark, size_mb, item.label);
        println!("       风险: {} · {}", safety_label(&item.safety), item.description);
    }
    println!();
}

fn print_process_items(items: &[ProcessInfo]) {
    println!("发现 {} 个进程优化项", items.len());
    println!("------------------------------------------");
    for process in items {
        let mark = if process.default_select { "[*]" } else { "[ ]" };
        println!(
            "{} PID {:>6}  {:>6.0} MB  {:>5.1}%  {}",
            mark,
            process.pid,
            process.memory_mb,
            process.cpu_percent,
            process.name
        );
        println!("       风险: {} · {}", risk_label(&process.risk), process.reason);
    }
    println!();
}

fn print_history_entries(entries: &[HistoryEntry]) {
    print_header();
    println!("最近 {} 条历史：", entries.len());
    println!("------------------------------------------");
    for entry in entries {
        println!(
            "[{}] {} · {}",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
            history_operation_label(&entry.operation),
            entry.target
        );
        println!(
            "       {}{}",
            if entry.success { "成功" } else { "失败" },
            if entry.freed_bytes > 0 {
                format!(" · 释放 {}", format_bytes(entry.freed_bytes))
            } else {
                String::new()
            }
        );
        if !entry.detail.is_empty() {
            println!("       {}", entry.detail);
        }
    }
}

fn safety_label(safety: &macslim_lib::cache_scanner::Safety) -> &'static str {
    match safety {
        macslim_lib::cache_scanner::Safety::Safe => "安全",
        macslim_lib::cache_scanner::Safety::Low => "低风险",
        macslim_lib::cache_scanner::Safety::Medium => "谨慎",
    }
}

fn history_operation_label(op: &str) -> &'static str {
    match op {
        "process_kill" => "进程清理",
        "cache_clean" => "缓存清理",
        _ => "未知操作",
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn risk_label(risk: &scanner::Risk) -> &'static str {
    match risk {
        scanner::Risk::Safe => "安全",
        scanner::Risk::Low => "低风险",
        scanner::Risk::Dev => "开发者确认",
        scanner::Risk::Hidden => "隐藏",
    }
}

#[allow(dead_code)]
fn _unused_main_entry() {
    run_tauri();
}
