# 变更日志

## v0.2.2 · 2026-04-30

### 修复
- **应用卸载权限**：卸载 `/Applications/` 下的应用时，遇到「Permission denied (os error 13)」不再直接失败。现在通过 `do shell script with administrator privileges` 弹出系统授权框，一次输入密码即可批量将多个应用与残留文件移至 `~/.Trash/`。
- 用户取消授权时给出友好中文提示「用户取消授权」，不再透出 osascript 原始错误码。

### 实现细节
- `try_trash_user` / `trash_via_admin_batch` 双层流程：先用户权限尝试 NSFileManager + rename，权限不足的路径汇集后**单次**批量授权处理，避免每个文件弹一次密码框。
- shell + AppleScript 双层转义：`shell_single_quote` 处理单引号，`applescript_quote` 处理 `\` 和 `"`。

## v0.1.0 · 2026-04-21 (里程碑 1-4 合并)

### 国际化 · Auto-update（本轮新增）
- 全 UI 中英双语（zh-CN / en），自动跟随系统 / 手动切换
- `@solid-primitives/i18n` 模式，`src/i18n/` 目录统一管理
- Tauri Updater 插件接入，基于 minisign 签名
- `scripts/publish-update.sh`：生成 manifest + 签名 DMG
- Updater 公钥已嵌入 `tauri.conf.json`，私钥在 `~/.tauri/macslim-updater.key`（不入库）
- 设置页新增「检查更新」按钮，支持进度显示 + 自动重启

### 核心能力
- 首版桌面应用：Tauri v2 + SolidJS 1.9 + Tailwind CSS v4
- Rust 核心：sysinfo 进程监控 + nix 信号 + rusqlite 持久化
- CLI 二进制：`macslim-cli --scan` / `--cache` / `--help`

### 进程管理
- 规则引擎识别：软件关闭残留、重复冗余、长期闲置、高占用、开发残留
- 优雅终止：SIGTERM → 3 秒探测 → SIGKILL
- macOS 系统核心白名单（40+ 进程，SIP 保护项完全隐藏）
- 端口占用检测：lsof 解析正在监听的 TCP 端口
- 监听端口的进程自动降级为「疑似运行中的服务」，默认不选

### 缓存清理（10 类工具）
- NPM / PNPM / Yarn / Docker / Homebrew / Xcode / CocoaPods / Cargo / Pip / Go
- 并行扫描（tokio spawn_blocking）
- 优先使用工具原生命令（`npm cache clean --force` 等）
- 无原生命令项 mv 到 /tmp 后后台删除

### 安全加固
- 工具占用检测：npm / pnpm / yarn / brew / xcodebuild / cargo / go / pip 运行中完全跳过
- 路径白名单硬编码（12 条），`canonicalize` 后比对防 symlink 绕过
- 拒绝 `/`、`/usr`、`/System`、`$HOME`、`~/Documents` 等危险路径
- 执行前二次校验 busy_check + 白名单
- Homebrew 去掉 `--prune=all` 过激参数
- Cargo 只清 `registry/cache`，不动 `src/` 和 `git/`
- Go 只清 `-cache`，不动 `-modcache`（避免弱网重新下载）
- 7 条单元测试覆盖白名单防御

### UI / UX（CleanMyMac 风）
- 首次启动欢迎页（三大能力介绍）
- 主界面：Sidebar 导航 + 环形进度卡片 + 分组列表
- macOS 原生毛玻璃（`NSVisualEffectMaterial::Sidebar`）
- 原生 titlebar overlay（红黄绿按钮、可拖动）
- 深浅色跟随系统
- 进程列表「加白名单」盾牌按钮

### 系统集成
- 后台监控线程（2 秒刷新 CPU/内存/磁盘，emit 事件 + 更新托盘 tooltip）
- macOS 系统托盘（菜单 + 图标 template）
- 通知中心（清理完成 toast）
- 开机启动开关（tauri-plugin-autostart）
- 自定义白名单 UI（进程名 / 路径，CRUD + 扫描联动）
- 历史记录视图（SQLite 查询）

### 打包与分发
- Release profile: LTO + strip + codegen-units=1 + opt-level=s
- Apple Developer ID 代码签名（Team 5XNDF727Y6, Beijing VGO Co., Ltd.）
- Hardened Runtime + entitlements.plist
- 两阶段构建：`scripts/release.sh`（编译 + 签名 + 公证）
- 签名探活：`scripts/sign.sh` 自动检测 Apple timestamp 服务可用性，不可用时降级到无时间戳签名（本地可用，公证可后补）
- 当前状态：DMG 已签名（Developer ID 可识别），公证待 Apple timestamp 服务恢复后执行

### 实测数字
- DMG 体积：**8.0 MB**（含 updater + i18n + 完整 Apple Developer ID 签名）
- .app bundle：约 17 MB
- 单二进制：7.8 MB（启用 LTO + strip）
- 前端 bundle：120 KB（gzip 34 KB，含 i18n 字典）
- 冷启动 RSS：~98 MB
- 首次扫描：< 3 秒
- 真实环境扫到：4.65 GB 可清理缓存（我的 Mac）

### 文档
- PRD.md 完整产品需求
- CLAUDE.md 项目级 Claude Code 规则
- README.md 项目说明
- CHANGELOG.md（本文件）
- Landing page（`landing/index.html`）

### 删除 / 作废
- ❌ 移除所有 AI / LLM 相关规划（成本、隐私、过度工程）
- ❌ 放弃原 PRD 的 ASCII-only UI 设计
- ❌ 撤销「一键回滚」的过度承诺，改为「操作日志 + 重新获取指引」
- ❌ 取消「每天限 3 次清理」的定价模型
