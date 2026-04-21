# MacFlow

> Mac 专属的一键式系统运维工具 · DMG 5.4 MB

MacFlow 清理冗余进程和开发者缓存（NPM / Docker / Xcode / Homebrew / Cargo / Go 等），让 Mac 保持轻快。

- 🪶 **极致轻量**：DMG 5.4 MB，比 CleanMyMac 小 100 倍，比 Electron 应用小 20 倍
- 🛡️ **三重安全防御**：工具占用检测 + 路径白名单 + 执行前二次校验
- 🎯 **规则驱动**：不接 AI，所有分类用规则，可审计
- 🔒 **零数据上传**：本地 SQLite 存储，离线运行
- ⚡ **双形态**：桌面应用 + CLI 共享 Rust 核心

## 快速开始

### 下载

访问 [macflow.app](https://macflow.app) 或从 [Releases](https://github.com/edwinhao/macflow/releases) 下载最新 DMG。

### 从源码构建

**前置**：
- Rust 1.80+（`curl https://sh.rustup.rs | sh`）
- Bun 1.3+（`curl https://bun.sh/install | bash`）
- macOS 13+ & Xcode Command Line Tools

**构建**：

```bash
git clone https://github.com/edwinhao/macflow.git
cd macflow
bun install
bun run tauri dev      # 开发模式
bun run tauri build    # Release 打包
```

带签名的 Release 构建：

```bash
./scripts/release.sh arm        # Apple Silicon
./scripts/release.sh intel      # Intel
./scripts/release.sh universal  # 通用二进制
```

## CLI

MacFlow 自带 `macflow-cli` 二进制：

```bash
macflow-cli --scan     # 扫描，不执行清理
macflow-cli --cache    # 扫描并清理所有安全项
macflow-cli --help     # 完整帮助
```

## 技术栈

| 层 | 选择 | 版本 |
| :--- | :--- | :--- |
| 桌面框架 | Tauri | v2.10 |
| 前端 | SolidJS + TypeScript | 1.9 |
| 样式 | Tailwind CSS | v4 |
| 后端 | Rust | 1.95 |
| 系统监控 | sysinfo | 0.33 |
| 进程信号 | nix (POSIX kill) | 0.29 |
| 持久化 | rusqlite | 0.32 |

## 清理范围

| 类别 | 命令 | 默认选中 |
| :--- | :--- | :--- |
| NPM 全局缓存 | `npm cache clean --force` | ✅ |
| PNPM Store | `pnpm store prune` | ✅ |
| Yarn 缓存 | `yarn cache clean` | ✅ |
| Docker 悬空镜像 | `docker image prune -f` | ✅ |
| Docker 构建缓存 | `docker builder prune -f` | ✅ |
| Docker 30 天停止容器 | `docker container prune -f --filter until=720h` | ✅ |
| Docker 匿名卷 | `docker volume prune -f` | ⚠️ 默认不选 |
| Homebrew 旧包缓存 | `brew cleanup -s` | ✅ |
| Xcode DerivedData | 直接删除 | ✅ |
| Xcode iOS 模拟器缓存 | 直接删除 | ✅ |
| Xcode iOS 设备支持 | 直接删除 | ⚠️ 默认不选 |
| CocoaPods 缓存 | 直接删除 | ✅ |
| Cargo 下载缓存 | 直接删除 `~/.cargo/registry/cache` | ✅ |
| Pip 缓存 | `pip cache purge` | ✅ |
| Go 编译缓存 | `go clean -cache` | ✅ |

## 安全保证

1. **工具占用检测**：npm / pnpm / yarn / brew / xcodebuild / cargo / go / pip 任一正在运行时，对应缓存不会出现在扫描列表
2. **路径白名单硬编码**：12 条允许路径，`canonicalize` 后比对，拒绝 `/`、`$HOME`、`~/Documents` 等
3. **执行前二次校验**：清理瞬间再次检查工具是否启动
4. **原生命令优先**：使用官方 cleanup 命令而非手动删除
5. **单元测试覆盖**：`cargo test` 验证白名单防御

## 里程碑

- [x] **M1** 项目骨架：Tauri + SolidJS + Tailwind v4
- [x] **M2** 缓存清理 + SQLite + 托盘 + CLI
- [x] **M3** 安全加固 + 后台监控 + 通知 + 开机启动 + 端口检测 + 欢迎页
- [ ] **M4** 公测：官网 landing、Universal 2 打包、notarization 自动化

## 许可

MIT License

## 非目标

MacFlow **不做**以下事情：

- Windows / Linux 版本
- 杀毒、防火墙
- 应用卸载、大文件扫描、重复文件清理
- 任何 AI / LLM 调用
- 广告、推送、遥测
- 云端同步（Pro 版除外，正在设计中）

---

Made with 🫀 in Beijing · Rust 1.95 · Tauri v2
