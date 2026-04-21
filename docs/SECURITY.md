# 安全政策

## 漏洞披露

如果你发现 MacFlow 的任何安全问题 —— 特别是会导致**误删用户数据**的 bug —— 请通过以下方式私下报告：

- 邮件：`security@macflow.app`（或者首个 GitHub issue 标题加 `[SECURITY]`，我们会立即移至私人讨论）
- 请**不要**直接开公开 Issue，我们希望先修复再披露

我们承诺：
- 24 小时内响应
- 7 天内给出修复时间表
- 修复后在 CHANGELOG 致谢（可选）

## 威胁模型

MacFlow 的核心风险不是网络攻击（我们没有网络），而是**误删**。三个威胁向量：

### 1. 清理错误路径 → 用户数据丢失

**防御**：
- `cache_cleaner::is_cleanup_path_allowed()` 硬编码 12 条允许路径
- 所有路径经 `canonicalize` 再比对，防 symlink 指到 $HOME 或系统目录
- 显式拒绝 `/`、`/usr`、`/etc`、`/System`、`/Library`、`$HOME`、`~/Documents`、`~/Downloads`、`~/Desktop`
- 7 条单元测试（`cargo test --lib cache_cleaner`）验证各种攻击路径都被拒

### 2. 终止错误进程 → 应用崩溃 / 数据丢失

**防御**：
- `process_safety::safety_veto()` 四层审计：
  - 是其他进程的父进程 → 拒绝
  - 在多进程族白名单（Chrome / Electron / IDE / 通讯类）→ 拒绝
  - 运行时间 < 10 分钟 → 拒绝
  - 跨用户（root / 其他用户）→ 拒绝
- 默认选中仅限 `ProcessStatus::Zombie`（僵尸进程，已退出）
- 监听任何端口的进程 → 取消默认选中
- 9 条单元测试验证 Helper 类进程不出现、监听端口进程不被默认选中

### 3. 清理进行中被用户打断 → 文件系统不一致

**防御**：
- 所有直接删除先 `rename` 到 `/tmp`（原子操作），再后台异步 `remove_dir_all`
- 即使用户 Ctrl+C，已 rename 的目录也不会留半死状态

## 不在威胁模型内

以下情况**不是** MacFlow 的责任：

- 用户手动把自己的数据目录加到白名单后被误删（用户主动授权）
- Apple 官方工具（npm / docker / brew / go / cargo）自身的命令行为
- 用户关闭 Gatekeeper 后安装第三方篡改过的 DMG
- macOS 系统本身的漏洞

## 代码签名与完整性

- 发布的 DMG 由 **Developer ID Application: Beijing VGO Co., Ltd. (Team 5XNDF727Y6)** 签名
- Hardened Runtime 开启
- 从源码自建的二进制不带签名，会触发 Gatekeeper 提示，右键「打开」绕过

验证签名：
```bash
codesign -dvvv /Applications/MacFlow.app
# 应看到: Authority=Developer ID Application: Beijing VGO Co;Ltd (5XNDF727Y6)
```

## 依赖审计

我们定期用 `cargo audit` 审计 Rust 依赖漏洞。如果你发现某个依赖报告了 RUSTSEC 公告，请提 Issue。

## 数据隐私

MacFlow **不做**以下任何一件事：

- 任何形式的 telemetry / 分析 / 用户行为上报
- 任何形式的错误 / 崩溃上报（Sentry 等）
- 发送任何数据到远程服务器
- 账号登录 / 用户追踪
- 第三方广告 / 推广 SDK

唯一的网络活动：
- 从 App Store 外下载时，macOS Gatekeeper 自动请求 `ocsp.apple.com` 校验签名
- 未来的软件更新（会在实现前征求用户意见）

想自己验证？用 Little Snitch 监控网络，打开 MacFlow，确认除了 Apple 的 OCSP 检查外**没有任何出站连接**。
