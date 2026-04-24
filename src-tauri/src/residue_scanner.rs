// 残留文件扫描器：根据 Bundle ID 和应用名在 ~/Library/ 中查找关联残留
use crate::app_scanner::dir_size;
use crate::dev_tool_rules::get_dev_tool_rules;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// 需要扫描的 ~/Library/ 子目录列表
const LIBRARY_SUBDIRS: &[&str] = &[
    "Application Support",
    "Caches",
    "Preferences",
    "Logs",
    "Containers",
    "Group Containers",
    "Saved Application State",
    "HTTPStorages",
    "WebKit",
];

/// 残留文件条目
#[derive(Serialize, Clone, Debug)]
pub struct ResidueItem {
    pub path: String,
    pub category: String,
    pub size_bytes: u64,
    pub is_dev_tool: bool,
    pub selected: bool,
}

/// 单个应用的残留扫描结果
#[derive(Serialize, Clone, Debug)]
pub struct AppResidue {
    pub bundle_id: String,
    pub app_name: String,
    pub items: Vec<ResidueItem>,
    pub total_bytes: u64,
    pub scan_complete: bool,
}

/// 扫描指定应用的残留文件
pub fn scan_residues(bundle_id: &str, app_name: &str) -> AppResidue {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return empty_result(bundle_id, app_name, false),
    };
    let library = home.join("Library");
    if !library.exists() {
        return empty_result(bundle_id, app_name, false);
    }

    let scan_complete = !bundle_id.is_empty();
    let name_lower = app_name.to_lowercase();
    let mut items = Vec::new();

    // 扫描 ~/Library/ 下 9 个子目录
    for subdir in LIBRARY_SUBDIRS {
        let dir = library.join(subdir);
        if !dir.exists() {
            continue;
        }
        scan_directory(&dir, subdir, bundle_id, &name_lower, &mut items);
    }

    // 集成开发者工具规则的额外路径
    if !bundle_id.is_empty() {
        scan_dev_tool_paths(bundle_id, &home, &mut items);
    }

    let total_bytes = items.iter().map(|i| i.size_bytes).sum();
    AppResidue {
        bundle_id: bundle_id.to_string(),
        app_name: app_name.to_string(),
        items,
        total_bytes,
        scan_complete,
    }
}

/// 扫描单个 ~/Library/ 子目录，查找匹配的残留
fn scan_directory(
    dir: &Path,
    category: &str,
    bundle_id: &str,
    name_lower: &str,
    items: &mut Vec<ResidueItem>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let entry_name = entry.file_name().to_string_lossy().to_string();
        if !matches_residue(&entry_name, bundle_id, name_lower) {
            continue;
        }
        let path = entry.path();
        let abs_path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => path.clone(),
        };
        let size = compute_entry_size(&abs_path);
        items.push(ResidueItem {
            path: abs_path.to_string_lossy().to_string(),
            category: category.to_string(),
            size_bytes: size,
            is_dev_tool: false,
            selected: true,
        });
    }
}

/// 扫描开发者工具规则定义的额外路径
fn scan_dev_tool_paths(
    bundle_id: &str,
    home: &Path,
    items: &mut Vec<ResidueItem>,
) {
    let rules = get_dev_tool_rules(bundle_id);
    for rule in rules {
        for extra in &rule.extra_paths {
            let expanded = expand_tilde(extra, home);
            if !expanded.exists() {
                continue;
            }
            let abs_path = expanded.canonicalize().unwrap_or(expanded);
            // 避免与已扫描的路径重复
            let path_str = abs_path.to_string_lossy().to_string();
            if items.iter().any(|i| i.path == path_str) {
                continue;
            }
            let size = compute_entry_size(&abs_path);
            items.push(ResidueItem {
                path: path_str,
                category: rule.label.clone(),
                size_bytes: size,
                is_dev_tool: true,
                selected: true,
            });
        }
    }
}

/// 判断条目名称是否匹配 Bundle ID 或应用名称
fn matches_residue(entry_name: &str, bundle_id: &str, name_lower: &str) -> bool {
    // Bundle ID 精确子串匹配
    if !bundle_id.is_empty() && entry_name.contains(bundle_id) {
        return true;
    }
    // 应用名称大小写不敏感匹配
    if !name_lower.is_empty() && entry_name.to_lowercase().contains(name_lower) {
        return true;
    }
    false
}

/// 计算文件或目录大小
fn compute_entry_size(path: &Path) -> u64 {
    if path.is_dir() {
        dir_size(path)
    } else {
        path.metadata().map(|m| m.len()).unwrap_or(0)
    }
}

/// 展开路径中的 ~ 为用户主目录
fn expand_tilde(path: &str, home: &Path) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else if path == "~" {
        home.to_path_buf()
    } else {
        PathBuf::from(path)
    }
}

/// 构造空的扫描结果
fn empty_result(bundle_id: &str, app_name: &str, scan_complete: bool) -> AppResidue {
    AppResidue {
        bundle_id: bundle_id.to_string(),
        app_name: app_name.to_string(),
        items: Vec::new(),
        total_bytes: 0,
        scan_complete,
    }
}
