<div align="center">

<img src="assets/logo.svg" width="80" height="80" alt="MacFlow" />

# MacFlow

**The Mac cleaner that developers actually trust.**

One-click cleanup for app caches, developer tools, Docker, and more — built with Rust and Tauri, not Electron.

[Download for macOS →](https://github.com/edwinhao/macflow/releases)

![macOS](https://img.shields.io/badge/macOS-13%2B-black?logo=apple)
![Tauri](https://img.shields.io/badge/Tauri-v2-blue)
![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange)
![License](https://img.shields.io/badge/license-MIT-green)

</div>

---

## Why MacFlow?

Most Mac cleaners are either too simple (just empty the trash) or too bloated (Electron apps heavier than what they clean). MacFlow is different:

- **Tiny**: ~12 MB DMG. CleanMyMac is 200 MB+.
- **Fast**: Rust backend, sub-second scans.
- **Honest**: Every item shows exactly what will be deleted and why. No dark patterns.
- **Developer-aware**: Understands npm, pnpm, Docker, Xcode, Cargo, Homebrew — not just `~/Library/Caches`.
- **Safe**: Triple-layer protection. Won't touch files that tools are actively using.
- **Private**: Zero telemetry. No account required. All data stays on your Mac.

---

## What it cleans

| Category | Items | Safe to clean? |
| :--- | :--- | :--- |
| **System** | App caches, logs, crash reports, Trash | ✅ Always |
| **npm / pnpm / Yarn** | Download caches, store | ✅ Always |
| **Docker** | Build cache, dangling images, stopped containers | ✅ / ⚠️ |
| **Homebrew** | Downloaded bottles, old versions | ✅ Always |
| **Xcode** | DerivedData, simulator caches, device support | ✅ / ⚠️ |
| **Cargo** | Registry download cache | ✅ Always |
| **Pip / Go** | Build and download caches | ✅ Always |

Items marked ⚠️ are shown but not selected by default.

---

## Screenshots

> *(coming soon — PRs welcome)*

---

## Getting Started

### Download

Grab the latest `.dmg` from [Releases](https://github.com/edwinhao/macflow/releases) and drag MacFlow to your Applications folder.

**Requirements**: macOS 13 Ventura or later, Apple Silicon or Intel.

### Build from source

**Prerequisites**

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Bun (package manager)
curl -fsSL https://bun.sh/install | bash

# Xcode Command Line Tools
xcode-select --install
```

**Run in development**

```bash
git clone https://github.com/edwinhao/macflow.git
cd macflow
bun install
bun run tauri dev
```

**Build a release**

```bash
# Apple Silicon
bun run bundle:arm

# Intel
bun run bundle:intel

# Universal binary (both architectures)
bun run bundle:universal
```

The `.app` and `.dmg` will be in `src-tauri/target/<arch>/release/bundle/`.

---

## Tech Stack

| Layer | Choice | Why |
| :--- | :--- | :--- |
| Desktop framework | Tauri v2 | Native WebView, no Chromium bundled |
| Frontend | SolidJS + TypeScript | 3–5× smaller bundle than React, no Virtual DOM |
| Styling | Tailwind CSS v4 | Utility-first, zero runtime |
| Backend | Rust | Memory-safe, fast, great macOS APIs |
| System info | sysinfo 0.33 | Cross-platform process/memory/disk |
| Process signals | nix (POSIX) | Native `kill()`, no shell subprocess |
| Storage | rusqlite 0.32 | Embedded SQLite, no server |

---

## Safety Model

MacFlow uses a three-layer safety system before deleting anything:

1. **Tool busy check** — if `npm`, `cargo`, `xcodebuild`, etc. are running, their caches are hidden from the scan entirely.
2. **Path allowlist** — only a hardcoded set of safe paths can ever be deleted. Attempts to delete `/`, `$HOME`, `~/Documents`, or anything outside the allowlist are rejected.
3. **Pre-execution re-check** — right before deletion, the tool-busy check runs again to guard against race conditions.

All cleanup operations are logged to a local SQLite database. You can review every action in the History tab.

---

## Project Structure

```
macflow/
├── src/                  # SolidJS frontend
│   ├── views/            # Page components (Scan, Cache, Process, Uninstaller…)
│   ├── components/       # Shared UI components
│   ├── i18n/             # Translations (zh-CN, en)
│   └── lib/              # Tauri IPC wrappers, utilities
├── src-tauri/            # Rust backend
│   └── src/
│       ├── scanner.rs        # Process scanner & classifier
│       ├── cache_scanner.rs  # Cache discovery
│       ├── cache_cleaner.rs  # Safe deletion logic
│       ├── process_ops.rs    # Graceful kill with respawn detection
│       ├── process_safety.rs # Safety allowlist & veto rules
│       ├── applications.rs   # Running app management (bundle aggregation)
│       ├── app_scanner.rs    # Installed app scanner (with icon extraction)
│       ├── residue_scanner.rs # App residue file scanner
│       ├── dev_tool_rules.rs # Developer tool cleanup rules
│       ├── uninstaller.rs    # App uninstall executor (move to Trash)
│       ├── docker.rs         # Docker image/container/volume management
│       └── storage.rs        # SQLite history & whitelist
├── assets/               # App icons and marketing assets
└── scripts/              # Release & signing scripts
```

---

## Contributing

Contributions are welcome. Please open an issue before submitting a large PR so we can discuss the approach.

```bash
# Run Rust tests
cargo test --manifest-path src-tauri/Cargo.toml

# Type-check frontend
bun run build
```

---

## Roadmap

- [x] Process management (kill zombie/idle processes)
- [x] Cache cleanup (npm, Docker, Xcode, Homebrew, Cargo, Pip, Go)
- [x] System cache cleanup (app caches, logs, crash reports)
- [x] System health dashboard (CPU, memory, disk)
- [x] Application management (view running apps with real icons)
- [x] App uninstaller (remove apps + residue files, dev tool deep clean)
- [x] Docker deep view (images, containers, volumes management)
- [x] Operation history & audit log
- [x] Custom whitelist
- [x] Menu bar tray
- [x] i18n (English + 中文)
- [x] CLI tool (bundled in .app, auto-installed)
- [x] Apple notarized release (signed DMG)
- [ ] Auto-cleanup scheduler (Pro)
- [ ] Multi-device whitelist sync (Pro)

---

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

## 中文说明

</div>

MacFlow 是一款 Mac 专属的系统清理工具，面向开发者和普通用户。

**核心特点：**

- 清理应用缓存、日志、崩溃报告等系统垃圾
- 深度支持开发者工具：npm / pnpm / Docker / Xcode / Homebrew / Cargo / Pip / Go
- 进程管理：识别并终止僵尸进程、长期闲置进程
- 应用程序管理：按 .app 聚合进程，显示真实应用图标
- 应用卸载：完整卸载应用 + 残留文件清理，内置开发者工具深度规则
- Docker 深度管理：镜像 / 容器 / 卷细粒度管理
- 系统健康监控：CPU / 内存 / 磁盘实时显示
- 操作历史：每次清理都有日志，可审计
- CLI 工具：内置命令行，终端一键清理
- 完全本地运行，零数据上传，无需账号

**安装方式：**

从 [Releases](https://github.com/edwinhao/macflow/releases) 下载最新 `.dmg`，拖入应用程序文件夹即可。

**从源码构建：**

```bash
git clone https://github.com/edwinhao/macflow.git
cd macflow
bun install
bun run tauri dev      # 开发模式
bun run bundle:arm     # 打包 Apple Silicon
bun run bundle:intel   # 打包 Intel
```

**技术栈：** Tauri v2 · SolidJS · Rust · Tailwind CSS v4

---

<div align="center">
Made with ♥ in Beijing &nbsp;·&nbsp; <a href="https://github.com/edwinhao/macflow/issues">Report a bug</a> &nbsp;·&nbsp; <a href="https://github.com/edwinhao/macflow/discussions">Discussions</a>
</div>
