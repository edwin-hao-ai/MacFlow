# 变更日志

## v0.1.0 · 2026-04-21 (里程碑 1-3 合并)

### 核心能力
- 首版桌面应用：Tauri v2 + SolidJS 1.9 + Tailwind CSS v4
- Rust 核心：sysinfo 进程监控 + nix 信号 + rusqlite 持久化
- CLI 二进制：`macflow-cli --scan` / `--cache` / `--help`

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
- DMG 体积：**6.5 MB**
- .app bundle：13 MB
- 单二进制：6.6 MB
- 前端 bundle：97 KB（gzip 28 KB）
- 冷启动 RSS：~98 MB
- 首次扫描：< 3 秒
- 真实环境扫到：4.78 GB 可清理缓存（我的 Mac）

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
