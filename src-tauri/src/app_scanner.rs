// 应用扫描器：枚举已安装应用，计算大小，读取元数据
use crate::applications::{extract_plist_string, read_plist_metadata};
use serde::Serialize;
use std::path::{Path, PathBuf};
use sysinfo::System;

/// 系统核心应用白名单（Bundle ID），默认隐藏不出现在卸载列表
const SYSTEM_CORE_APPS: &[&str] = &[
    "com.apple.finder",
    "com.apple.Safari",
    "com.apple.AppStore",
    "com.apple.systempreferences",
    "com.apple.SystemPreferences",
    "com.apple.Terminal",
    "com.apple.mail",
    "com.apple.iCal",
    "com.apple.AddressBook",
    "com.apple.Photos",
    "com.apple.iWork.Keynote",
    "com.apple.iWork.Pages",
    "com.apple.iWork.Numbers",
    "com.apple.FaceTime",
    "com.apple.MobileSMS",
    "com.apple.Music",
    "com.apple.TV",
    "com.apple.Podcasts",
    "com.apple.Maps",
    "com.apple.Notes",
    "com.apple.reminders",
    "com.apple.stocks",
    "com.apple.weather",
    "com.apple.calculator",
    "com.apple.Preview",
    "com.apple.TextEdit",
    "com.apple.ActivityMonitor",
    "com.apple.DiskUtility",
    "com.apple.Console",
    "com.apple.Automator",
    "com.apple.ScriptEditor2",
    "com.apple.ScreenSharing",
    "com.apple.keychainaccess",
];

/// 已安装应用信息（磁盘上的 .app bundle）
#[derive(Serialize, Clone, Debug)]
pub struct InstalledApp {
    pub bundle_path: String,
    pub name: String,
    pub bundle_id: String,
    pub icon_base64: Option<String>,
    pub bundle_size_bytes: u64,
    pub is_system: bool,
    pub is_running: bool,
    pub estimated_residue_bytes: u64,
}

/// 判断 Bundle ID 是否为系统核心应用
pub fn is_system_app(bundle_id: &str) -> bool {
    if bundle_id.is_empty() {
        return false;
    }
    SYSTEM_CORE_APPS.iter().any(|&id| id == bundle_id)
}

/// 从 .app bundle 读取图标并转为 base64 PNG
/// 流程：Info.plist → CFBundleIconFile → .icns 路径 → sips 转 PNG → base64
fn read_icon_base64(app_path: &Path, plist_path: &Path) -> Option<String> {
    let icon_name = read_icon_name(plist_path)?;
    let resources = app_path.join("Contents/Resources");
    // 图标文件可能带 .icns 后缀也可能不带
    let icns_path = if icon_name.ends_with(".icns") {
        resources.join(&icon_name)
    } else {
        resources.join(format!("{}.icns", icon_name))
    };
    if !icns_path.exists() {
        return None;
    }
    icns_to_base64_png(&icns_path)
}

/// 供其他模块（如 applications.rs）调用的公共接口
pub fn read_icon_base64_for_bundle(bundle_path: &Path) -> Option<String> {
    let plist_path = bundle_path.join("Contents/Info.plist");
    if !plist_path.exists() {
        return None;
    }
    read_icon_base64(bundle_path, &plist_path)
}

/// 从 Info.plist 读取图标文件名
fn read_icon_name(plist_path: &Path) -> Option<String> {
    let bytes = std::fs::read(plist_path).ok()?;
    let text = if bytes.starts_with(b"bplist") {
        let output = std::process::Command::new("plutil")
            .args(["-convert", "xml1", "-o", "-"])
            .arg(plist_path)
            .output()
            .ok()?;
        if !output.status.success() { return None; }
        String::from_utf8(output.stdout).ok()?
    } else {
        std::str::from_utf8(&bytes).ok()?.to_string()
    };
    extract_plist_string(&text, "CFBundleIconFile")
        .or_else(|| extract_plist_string(&text, "CFBundleIconName"))
}

/// 用 sips 将 .icns 转为 64x64 PNG 并返回 base64 编码
fn icns_to_base64_png(icns_path: &Path) -> Option<String> {
    use base64::Engine;
    let tmp = std::env::temp_dir().join(format!(
        "macflow_icon_{}.png",
        icns_path.file_stem()?.to_string_lossy()
    ));
    let output = std::process::Command::new("sips")
        .args(["-s", "format", "png", "-z", "64", "64"])
        .arg(icns_path)
        .arg("--out")
        .arg(&tmp)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let png_bytes = std::fs::read(&tmp).ok()?;
    let _ = std::fs::remove_file(&tmp);
    Some(base64::engine::general_purpose::STANDARD.encode(&png_bytes))
}

/// 计算目录总大小（字节）
pub(crate) fn dir_size(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// 从 .app 路径构建 InstalledApp 信息
fn build_installed_app(
    app_path: &Path,
    running_bundles: &std::collections::HashSet<String>,
) -> Option<InstalledApp> {
    let plist_path = app_path.join("Contents/Info.plist");
    let (name, bundle_id) = if plist_path.exists() {
        read_plist_metadata(&plist_path)
    } else {
        (None, None)
    };

    // 从文件名推断名称（兜底）
    let display_name = name.unwrap_or_else(|| {
        app_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "未知应用".to_string())
    });

    let bid = bundle_id.unwrap_or_default();
    let icon = read_icon_base64(app_path, &plist_path);
    let bundle_size = dir_size(app_path);
    let is_system = is_system_app(&bid);
    let is_running = if bid.is_empty() {
        false
    } else {
        running_bundles.contains(&bid)
    };

    Some(InstalledApp {
        bundle_path: app_path.to_string_lossy().to_string(),
        name: display_name,
        bundle_id: bid,
        icon_base64: icon,
        bundle_size_bytes: bundle_size,
        is_system,
        is_running,
        estimated_residue_bytes: 0, // 快速扫描阶段不计算残留
    })
}

/// 收集当前运行中应用的 Bundle ID 集合
fn collect_running_bundle_ids(sys: &mut System) -> std::collections::HashSet<String> {
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut ids = std::collections::HashSet::new();
    for (_pid, proc) in sys.processes() {
        if let Some(exe) = proc.exe() {
            if let Some(bundle) = crate::applications::find_app_bundle(exe) {
                let plist = bundle.join("Contents/Info.plist");
                if plist.exists() {
                    let (_, bid) = read_plist_metadata(&plist);
                    if let Some(id) = bid {
                        ids.insert(id);
                    }
                }
            }
        }
    }
    ids
}

/// 枚举指定目录下的所有 .app bundle
fn enumerate_apps(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|ext| ext == "app")
                .unwrap_or(false)
        })
        .collect()
}

/// 扫描已安装应用列表
/// 扫描 /Applications 和 ~/Applications，排除 /System/Applications
pub fn scan_installed_apps(sys: &mut System) -> Vec<InstalledApp> {
    let running = collect_running_bundle_ids(sys);

    let mut scan_dirs: Vec<PathBuf> = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        let user_apps = home.join("Applications");
        if user_apps.exists() {
            scan_dirs.push(user_apps);
        }
    }

    let mut apps: Vec<InstalledApp> = scan_dirs
        .iter()
        .flat_map(|dir| enumerate_apps(dir))
        .filter_map(|path| build_installed_app(&path, &running))
        .collect();

    // 按 bundle 大小降序排列
    apps.sort_by(|a, b| b.bundle_size_bytes.cmp(&a.bundle_size_bytes));
    apps
}
