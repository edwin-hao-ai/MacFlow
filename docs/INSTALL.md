# 从源码构建 MacSlim

本文档针对想自己编译 MacSlim 或为项目贡献代码的用户。如果只想用，请直接下载 [Releases](https://github.com/edwin-hao-ai/MacSlim/releases)。

## 前置环境

| 工具 | 最低版本 | 安装 |
| :--- | :--- | :--- |
| macOS | 13.0 (Ventura) | — |
| Xcode Command Line Tools | 最新 | `xcode-select --install` |
| Rust | 1.80+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Bun | 1.3+ | `curl -fsSL https://bun.sh/install \| bash` |

可选（如要签名发布）：
- Apple Developer Program 账号（$99/年）
- 从 Keychain 已导入的 `Developer ID Application` 证书

## 获取源码

```bash
git clone https://github.com/edwin-hao-ai/MacSlim.git
cd macslim
bun install
```

## 开发模式

```bash
bun run tauri dev
```

第一次会下载 Tauri 依赖并编译 Rust crate（约 5 分钟）。之后增量编译通常 <10 秒。

## Release 构建

### 单架构

```bash
bun run bundle:arm       # Apple Silicon (M 系列)
bun run bundle:intel     # Intel
```

### Universal 2 （一个 DMG 同时支持 ARM + Intel）

```bash
rustup target add x86_64-apple-darwin   # 第一次需要加 target
rustup target add aarch64-apple-darwin
bun run bundle:universal
```

### 产物位置

```
src-tauri/target/{arch}/release/bundle/
├── macos/MacSlim.app                    # .app 包（含 macslim 主程序 + macslim-cli）
└── dmg/MacSlim_<version>_*.dmg          # DMG 分发包
```

## 使用 CLI

桌面 App 内已内置 `macslim-cli` 二进制。装完 DMG 后，CLI 在：

```
/Applications/MacSlim.app/Contents/MacOS/macslim-cli
```

为了在终端任意位置调用，建议软链到 PATH：

```bash
sudo ln -s /Applications/MacSlim.app/Contents/MacOS/macslim-cli /usr/local/bin/macslim-cli
```

常用命令：

```bash
macslim-cli              # 启动桌面 App
macslim-cli --scan       # 扫描全部缓存（不清理）
macslim-cli --cache      # 清理缓存（带交互确认）
macslim-cli --history    # 查看本地历史
macslim-cli --help       # 全部命令
```

CLI 与桌面 App 共享同一个 Rust core 和 SQLite 数据库（`~/Library/Application Support/MacSlim/macslim.db`），扫描规则、白名单、安全检查、历史记录全部互通——你在 CLI 里清的东西，桌面 App 的「历史记录」标签里也能看到。

> **解除软链**：`sudo rm /usr/local/bin/macslim-cli`

## 签名 + 公证（可选）

如果你有 Apple Developer ID：

```bash
# 首次：把 Apple ID + App-Specific Password 存进 Keychain（一次性）
xcrun notarytool store-credentials macslim-notary \
  --apple-id YOUR_APPLE_ID@example.com \
  --team-id YOUR_TEAM_ID \
  --password YOUR_APP_SPECIFIC_PASSWORD

# 然后
./scripts/sign.sh arm        # 签名 + 公证
```

`scripts/sign.sh` 会自动探测 Apple timestamp 服务是否可用：不可用时降级到无时间戳签名（本地可用，但无法通过公证），服务恢复后重跑即完成公证。

## 常见问题

**Q: 编译报错 `nix::Uid` 找不到？**  
A: 确保 `nix` 的 `user` feature 在 Cargo.toml 里打开。

**Q: `bun run tauri dev` 报 Port 1420 已被占用？**  
A: 有残留的前一次 dev 进程。`pkill -f "target/debug/macslim"; pkill -f "bin/vite"` 然后重跑。

**Q: 签名报 `The timestamp service is not available`？**  
A: Apple timestamp 服务偶发故障。可以：
1. 等 15-60 分钟重试
2. 或用 `scripts/sign.sh` 自动降级到无时间戳签名

**Q: 怎么跑测试？**  
A: `cd src-tauri && cargo test`。当前 16 条测试覆盖：缓存清理路径白名单、进程分类安全审计、实时系统扫描不变式。

**Q: 前端热更新？**  
A: 只修改 `src/` 的前端代码时，Vite 自动热更新，不需要重启 Tauri。

## 目录结构

```
macslim/
├── src/                  # SolidJS 前端
│   ├── views/            # 各视图（ScanView、CacheView、HistoryView、SettingsView）
│   ├── components/       # 可复用组件
│   └── lib/              # Tauri 绑定 + 格式化工具
├── src-tauri/            # Rust 后端
│   ├── src/
│   │   ├── lib.rs               # Tauri 入口 + commands
│   │   ├── scanner.rs           # 进程扫描 + 分类
│   │   ├── process_safety.rs    # 进程安全审计（多进程族白名单 / 父进程保护等）
│   │   ├── process_ops.rs       # SIGTERM/SIGKILL 优雅终止
│   │   ├── cache_scanner.rs     # NPM/Docker/Xcode... 缓存扫描
│   │   ├── cache_cleaner.rs     # 缓存清理 + 路径白名单防御
│   │   ├── ports.rs             # lsof 端口占用检测
│   │   ├── storage.rs           # SQLite 历史 + 白名单
│   │   ├── monitor.rs           # 后台健康监控（2s 一次）
│   │   ├── tray.rs              # 系统托盘
│   │   ├── whitelist.rs         # 系统核心进程白名单
│   │   └── bin/cli.rs           # CLI 入口
│   ├── Cargo.toml
│   └── entitlements.plist       # Hardened Runtime 权限
├── landing/              # 静态 landing page
├── scripts/              # 构建 / 签名脚本
└── assets/               # logo 源文件
```
