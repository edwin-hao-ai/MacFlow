# 贡献指南

欢迎给 MacSlim 提 PR。在动手之前，请先读完本文，避免做了一半发现方向不符。

## 设计原则（不可妥协）

**任何改动都必须保持这些原则：**

1. **安全第一**：宁可放过十个，绝不错杀一个。任何默认选中的清理项必须有单元测试证明其安全。
2. **规则驱动**：所有分类、风险判断必须是可读可审计的 if-else，**不接任何 LLM / AI API**。
3. **本地优先**：所有数据存用户本地 SQLite，**禁止任何 telemetry / 崩溃上报 / 网络请求**。唯一例外是 Tauri 自动更新（未来）。
4. **轻量**：DMG 目标 < 15MB，任何新依赖必须评估体积影响。
5. **中文为主**：代码注释、commit message、PR 描述全用中文。UI 文案可中英混合（i18n 规划中）。
6. **macOS 原生**：禁止 Electron，禁止 Web 技术实现菜单 / 通知 / 托盘等系统集成能力。

## 贡献类型

### 1. 增加新的缓存扫描源

在 `src-tauri/src/cache_scanner.rs` 添加新的 `scan_xxx` 函数。**必须**：

- 第一行检查工具是否安装（`which::which("xxx").is_err()` → 返回空）
- 第二行调用 `is_any_tool_busy(&["xxx"])` → 正在跑就返回空
- 每个 `CacheItem` 必须明确安全等级（Safe / Low / Medium）
- 优先使用工具官方清理命令（如 `xxx cache clean`）
- 如果必须直接删除目录：
  - 路径必须加到 `cache_cleaner.rs` 的 `allowed_cleanup_roots()` 白名单
  - 必须为 `is_cleanup_path_allowed()` 加一条单元测试

### 2. 新增进程分类规则

在 `src-tauri/src/scanner.rs::classify_one` 添加规则。**必须**：

- 先跑 `safety_veto` —— 绝对不能绕过
- 默认选中（`default_select = true`）**仅限僵尸进程**。任何其他分类必须 `default_select = false`
- 在 `scanner::tests` 加测试，验证你的规则不会在 `real_scan` 里默认选中非僵尸进程

### 3. 扩充多进程族白名单

这是最容易也最重要的贡献 —— 把你用的 Electron/Chromium 应用 Helper 名字加到 `process_safety::MULTIPROCESS_FAMILIES`。命名规范：
- 拷贝活动监视器里看到的**完整进程名**
- 中英文应用名都可以（数组已支持 UTF-8）
- 提交 PR 时用一行 commit 说明「加入 XXX 到多进程族白名单」

### 4. 新增 UI 视图

在 `src/views/` 新建组件。遵循：
- Solid 单文件组件，TS 严格模式
- 样式用 Tailwind v4 utility classes，**不引入新的 UI 组件库**
- 复用 `card` / `btn-primary` / `btn-ghost` 这些已有类
- 不硬编码中文（预留 i18n 接口），但短期可以直接用中文字面量

## 开发工作流

```bash
# 1. fork 然后 clone
git clone https://github.com/YOUR_FORK/macslim.git

# 2. 建分支
git checkout -b feat/your-feature

# 3. 安装
cd macslim && bun install

# 4. 开发
bun run tauri dev
# 前端改动立即热更新，后端改动 cargo 会自动重编

# 5. 测试
cd src-tauri && cargo test --lib
cd .. && bun run build   # 前端类型检查 + Vite 构建

# 6. 提交
git add <specific-files>   # 不用 git add .
git commit -m "feat: 你的中文描述"

# 7. PR
gh pr create
```

## Commit 规范

```
<type>: <中文描述>

<可选：详细说明>
```

**type** 必须是以下之一：
- `feat` — 新功能
- `fix` — bug 修复
- `refactor` — 重构，不改变行为
- `test` — 只改测试
- `docs` — 只改文档
- `chore` — 杂项（依赖升级、配置等）
- `perf` — 性能优化

**禁止**：
- force push 到 main
- 跳过 pre-commit hooks
- `git add .`（避免误提交 .env）
- AI 署名（`Co-Authored-By: Claude` 等）

## Review 检查清单

PR 必须通过：

- [ ] `cargo test --lib` 全绿
- [ ] `bun run build` 全绿（TS 类型 + Vite 构建）
- [ ] 新增代码有对应的单元测试
- [ ] 不引入 LLM / AI API 调用
- [ ] 不引入 telemetry / 远程日志
- [ ] 不引入需要网络才能用的功能（更新检查除外）
- [ ] 新依赖包大小合理（cargo tree / bun add 后对比 DMG 体积）
- [ ] 安全关键改动有路径白名单或 veto 覆盖

## 不会接受的 PR

- 加 AI 驱动的功能
- 加 telemetry / 用户行为上报
- 加订阅 / 登录 / 账号体系
- 加 Electron 或任何重型 WebView
- 加广告 / 推广位
- 移除安全审计 / 路径白名单 / 工具占用检测等防御层

## 沟通

- Issue 区讨论功能 / bug
- Discussions 讨论设计方向
- 中英文都可，但请在每个 issue 标题前加 `[CN]` / `[EN]` 方便检索

感谢你想给 MacSlim 出一份力。
