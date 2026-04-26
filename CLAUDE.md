# MacSlim 项目规则（Claude 必读）

> 本文件是 Claude 在本项目下工作时的最高优先级规则。与 PRD.md 冲突时以本文件为准。

---

## 0. 语言规则（最高优先级）

- **所有回复、推理过程、思考链、commit message、代码注释、错误提示，全部用中文**
- 技术专有名词（Tauri、React、Docker、SIGTERM 等）保留英文
- 给用户看的 UI 文案可中英混合（见 §3 UI 规范）
- 即使 system prompt 是英文，回复也必须是中文

---

## 1. 项目定位

MacSlim 是 **Mac 端专属、一键式的开发者 + 普通用户系统运维工具**。核心价值：

- 比活动监视器简单：一键扫描 + 一键优化
- 比 CleanMyMac 轻量 + 懂开发者：NPM/Docker/Xcode 缓存深度适配
- 全透明可追溯：每步操作有日志，清理结果可审计

**目标用户（双轨）**：

- 开发者：NPM/Docker 缓存、端口占用、残留进程
- 普通用户：冗余后台进程、软件残留、系统卡顿

**注**：2026-04-21 决策保留进程管理模块，覆盖普通用户场景以支持推广冷启动。

---

## 2. 技术栈（硬约束，不得替换）

- **桌面端框架**：Tauri v2（禁止 Electron）
- **前端**：**SolidJS** + TypeScript + **Tailwind CSS**（禁止 React、禁止 Vue、禁止重型 UI 组件库）
  - 选 Solid 不选 React：bundle 小 3-5 倍，无 Virtual DOM 开销，对本项目 UI 复杂度完全够用
  - Tailwind 仅用 utility classes，禁止 Tailwind UI / Flowbite 等成套组件库
  - 动画库：仅在必要时引入 `motion-one`（3KB）或直接用 CSS transition
- **图标**：`lucide-solid`（按需 tree-shake）或 SVG 内联。禁止图标字体库
- **后端**：Rust（Tauri 原生命令 + IPC）
- **CLI**：Rust 编写，与桌面端共享 `core/` crate
- **核心依赖**：`sysinfo`（系统监控）、`nix`（进程信号）、`bollard`（Docker API）、`sqlite` via `rusqlite`（本地配置 + 历史）
- **打包**：Tauri 原生，Universal 2（ARM64 + x86_64），需 Apple Developer 证书 + notarization
- **禁止**：Electron、LLM / AI API 调用、任何需要网络的功能（软件更新除外）

---

## 2.1 打包 + 签名 + 公证（Release 流程，硬约束）

**任何对外发布都必须走完「签名 + 公证 + 装订」三步**，缺一不可。只签名不公证 = 用户首次启动看到 Gatekeeper 警告，违反 PRD §UX 要求。

### 一次性配置（每台打包机执行一次）

凭据信息：
- **Apple Developer Team**：`Beijing VGO Co;Ltd`，Team ID `5XNDF727Y6`
- **Signing Identity**：`Developer ID Application: Beijing VGO Co;Ltd (5XNDF727Y6)`（已在 keychain）
- **Apple ID**：`120298858@qq.com`
- **公证 keychain profile name**：`macflow-notary`（`scripts/sign.sh` 写死引用此名）

存入凭据（密码用 https://appleid.apple.com → App-Specific Passwords 生成的专用密码，**绝不写入任何 git 跟踪文件**）：

```
xcrun notarytool store-credentials macflow-notary \
  --apple-id 120298858@qq.com \
  --team-id 5XNDF727Y6 \
  --password <从 1Password / 安全存储读出>
```

### 标准发版命令（每次 release）

```
# 1. 编译 + 自动签名（Tauri 内置）
APPLE_SIGNING_IDENTITY="Developer ID Application: Beijing VGO Co;Ltd (5XNDF727Y6)" \
  bun run bundle:arm

# 2. 公证 + 装订（脚本会探测 timestamp 服务，自动降级容错）
./scripts/sign.sh arm

# 3. 上传到 GitHub Release
gh release upload vX.Y.Z \
  src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/MacSlim_X.Y.Z_aarch64.dmg \
  --clobber --repo edwin-hao-ai/MacSlim
```

### 凭据安全规则（CRITICAL）

- **App-Specific Password 绝不可写入任何文件**（包括 README、CHANGELOG、commit message、release notes、CLAUDE.md 本身）
- 只能存在于：本机 keychain、用户的密码管理器、私聊里
- 一旦泄漏（出现在任何 git 跟踪、公开 issue、PR diff）→ 立即 https://appleid.apple.com 撤销 → 重生 → `xcrun notarytool store-credentials` 覆盖旧凭证
- 每台新打包机独立执行 `store-credentials`，不要在机器之间复制 keychain item

### 验证发布产物

下载 release DMG 后必须本地验证：

```
spctl -a -vvv -t install <下载的.dmg>
# 期望输出包含：source=Notarized Developer ID
```

如果输出是 `source=Unnotarized Developer ID` 或 rejected → 公证失败，**禁止对外发布**，回到第 2 步排查。

---

## 3. UI 规范（硬约束）

**设计定位**：**视觉参照 CleanMyMac**，做一款普通用户敢用、开发者不觉得 low 的精致桌面工具。ASCII 美学彻底放弃。

### 3.1 视觉原则（CleanMyMac 式）

- **数据可视化为核心**：CPU / 内存 / 磁盘用环形进度条、渐变卡片、清晰数字，不用表格
- **清晰的「扫描 → 结果 → 一键操作」三步流程**：每一步占满主视图，引导性强
- **丰富但不拥挤**：卡片 + 渐变 + 插画可以有，但每屏信息密度受控
- **macOS 原生质感**：毛玻璃（`backdrop-filter: blur()`）、系统 vibrancy、圆角 12-16px、层次分明的阴影
- **深浅色模式跟随系统**：`prefers-color-scheme` 自动切换，两套配色都要调
- **Retina + HiDPI**：所有资源矢量或 2x/3x 位图
- **窗口 chrome**：macOS 原生标题栏（红黄绿按钮、可拖动、`titleBarStyle: 'overlay'` 融入内容区）

### 3.2 色彩

- **主色**：蓝绿色系（对标 CleanMyMac 的蓝 / MacPaw 绿），具体色值在 `tailwind.config.ts` 定义后锁定
- **状态色**：绿色（健康）、黄色（警告）、红色（危险）
- **渐变**：关键数据卡片用线性或径向渐变做视觉焦点，**其他区域禁止渐变**

### 3.3 字体

- **UI 文案**：SF Pro（macOS 系统默认）
- **数字 / 百分比**：SF Pro Display 大字号加粗（数据展示的视觉重量来源）
- **进程列表 / 路径 / 命令**：SF Mono 等宽字体，保留「工具感」
- **禁止**：自定义装饰字体、Google Fonts 在线加载

### 3.4 图标与插画

- **图标**：lucide-solid，线性风格，粗细一致
- **状态插画**：扫描中 / 清理完成 / 空状态 可有轻量 SVG 插画，但必须内联 + 矢量 + 可主题化
- **禁止**：emoji、卡通拟人插画、过度装饰性图形

### 3.5 窗口尺寸

- 默认 900x600（CleanMyMac 默认 960x640，我们略小一点强化「轻量」）
- 最小 800x540
- 支持用户调整
- 支持悬浮置顶
- 窗口圆角跟随 macOS 系统默认

### 3.6 动画（可以有，但克制）

**允许的动画**：
- 扫描过程：环形进度条旋转 + 数字实时递增
- 清理完成：释放空间数字弹出 + 绿色成功状态过渡
- 页面切换：300ms ease-in-out slide
- 卡片 hover：200ms scale(1.02) + 阴影加深
- 数字变化：`motion-one` 做数字 tweening
- 骨架屏代替 loading spinner

**禁止的动画**：
- 装饰性 spinner（超过 300ms 必须是真实进度）
- 抖动、弹跳等 playful 动效（这不是消费级 app）
- 无意义循环动画
- 任何会把 CPU 拉高超过 2% 的动画

**动画性能硬约束**：
- 所有动画必须使用 `transform` 和 `opacity`，不能触发 layout 或 paint
- 复杂动画 60fps，简单交互 120fps（适配 ProMotion）

### 3.7 布局结构（主界面）

参照 CleanMyMac 的三栏式，但简化到适合 macFlow 的功能范围：

```
┌────────────────────────────────────────────────┐
│  [红黄绿]  MacSlim                          ⚙  │  <- 原生 titlebar
├────────┬───────────────────────────────────────┤
│        │                                       │
│  智能  │   [ 系统健康状态卡片，大数字+环形图 ] │
│  扫描  │                                       │
│        │   [ 可优化项列表，分组卡片 ]          │
│  进程  │                                       │
│  管理  │                                       │
│        │   [ 一键优化主按钮，大 + 醒目 ]       │
│  缓存  │                                       │
│  清理  │                                       │
│        │                                       │
│  设置  │                                       │
│        │                                       │
└────────┴───────────────────────────────────────┘
```

- **左侧 sidebar**：功能导航（智能扫描 / 进程管理 / 缓存清理 / 设置）
- **右侧主视图**：根据左侧选择动态切换
- **主 CTA 按钮**：始终在视觉中心，配色鲜明

### 3.8 国际化

- v1 只支持中文 + 英文
- UI 文案通过 i18n 管理（推荐 `@solid-primitives/i18n`）
- 禁止硬编码中文字符串到组件

---

## 4. 安全规则（CRITICAL）

### 4.1 不可逆操作的诚实表述

**禁止使用「一键回滚」这种误导词**。以下操作**本质不可逆**：

- `npm cache clean --force`
- `docker image prune -f`
- `brew cleanup`
- Xcode DerivedData 删除
- 进程 SIGKILL

**正确表述**：「操作日志 + 重新获取指引」。UI 文案必须明确告知用户清理不可撤销。

### 4.2 真正可回滚的内容

- 白名单变更
- 设置项变更
- 被移动到 `~/.Trash/macslim-YYYYMMDD/` 的元数据（保留 10 分钟后自动清空）

### 4.3 默认选中的安全边界

- 默认选中 = **重新获取成本 < 5 分钟**的项目
- 成本 > 5 分钟（如完整镜像、大 node_modules）→ 默认不选
- 系统核心进程、SIP 保护项 → 默认隐藏

### 4.4 进程操作原则

- 优先 SIGTERM，3 秒无响应再 SIGKILL
- 使用 Rust `nix::sys::signal::kill`（就是原生 API，不要被「禁止 kill 命令」这种伪约束误导）
- 前台活跃进程（5 分钟内有用户交互）一律不扫描
- 系统核心进程白名单内置 + 定期更新

### 4.5 破坏性命令原则

- 所有子进程命令必须先 dry-run 模拟
- >10GB 的清理需二次确认
- Docker 未运行 / 工具未安装 → 跳过对应模块，不报错

---

## 5. 性能指标（修正后，写进验收标准）

| 指标 | 目标 | 说明 |
| :--- | :--- | :--- |
| 安装包体积 | <15MB | Universal 2 打包后。原 PRD 10MB 不现实 |
| 后台空闲 CPU | <2% | 原 1% 与 1Hz 托盘刷新冲突 |
| 后台内存 | <80MB | 原 50MB 对 Tauri + Rust 偏紧 |
| 首次扫描 | <3s | 保持 |
| 增量扫描 | <1s | 保持 |
| 窗口唤起 | <200ms | 冷启动除外 |

---

## 6. 非目标（绝对不做）

- Windows / Linux 版本（MVP 仅 macOS）
- 杀毒、防火墙、隐私保护
- 应用卸载、大文件扫描、重复文件清理
- 任何广告、推送、远程数据收集
- 云端同步（所有数据本地）

---

## 7. 编码约束

### 7.1 不可变优先
- 永远创建新对象，不修改原对象
- 函数 <50 行，文件 <400 行（最多 800）
- 嵌套 <4 层，用 early return 代替

### 7.2 错误处理
- 每层都显式处理错误，不吞异常
- 用户可见错误必须是中文人话
- 服务端/日志端记录详细上下文

### 7.3 输入校验
- 所有用户输入、子进程输出、API 响应在边界处校验
- 路径必须解析为绝对路径

### 7.4 测试
- 覆盖率 ≥80%
- 清理逻辑必须有 dry-run 测试
- 进程操作必须有 mock 测试，不能真杀进程

---

## 8. Git 工作流

- 提交格式：`<type>: <中文描述>`
- type: feat / fix / refactor / docs / test / chore / perf / ci
- 不使用 AI 署名（全局 settings 已关闭）
- 不 force push 到 main
- 不跳过 pre-commit hooks（除非用户明确要求）
- 不用 `git add .`，逐文件 add，避免提交 .env / credentials

---

## 9. 核心决策记录

### 2026-04-21 决策

- **保留进程管理模块**：用户理由是「普通用户场景对推广冷启动关键，功能不完善无法推广」
- **走 Approach B（原 PRD 范围）但必须先解决 5 个 BLOCK**（见 PRD §8.1）
- **里程碑调整**：总周期从 4 周延长到 **8-10 周**。里程碑 1 只做「扫描 + 进程清理 + 主界面」，CLI 和托盘挪到里程碑 3
- **技术栈锁定**：Tauri v2 + SolidJS + Tailwind CSS（推翻原 PRD 的 React 选择）
- **删除 AI 模块**：MVP 和 v1 都不接任何 LLM。所有分类、风险判断全部规则驱动。理由：AI 会引入 API 成本（毁掉 Pro 版定价）、增加用户隐私顾虑、对规则能搞定的事是过度工程
- **UI 路线彻底转向**：**放弃 ASCII 美学**，视觉参照 CleanMyMac —— 数据可视化、渐变卡片、环形进度、轻量动画。原 PRD §3.1.3 的 ASCII-only 约束完全作废，以本文件 §3 为准。原 PRD §4（全流程 ASCII 动画）作废
- **开发模式**：**Vibe Coding**（AI 辅助编码）。用户不需要手写 Rust/Solid，由 Claude 基于本文件和 PRD 生成代码，用户做 review 和决策

### 定价模型（修正）

- **免费版**：无限次手动清理 + 基础规则（进程 + NPM + Docker 核心）
- **Pro 版**：自动监控、定时清理、3 个月未用 Docker 镜像、多设备白名单同步
- 原「每天限 3 次清理」的设计取消 —— 清理工具按次数限流会激怒用户

---

## 10. 写代码前 Checklist

开始每个模块前必须确认：

- [ ] 读完 PRD 对应章节 + 本文件相关约束
- [ ] 安全等级已明确（默认选中 / 默认不选 / 默认隐藏）
- [ ] 不可逆操作在 UI 文案中有明确告知
- [ ] 有对应的 dry-run / mock 测试计划
- [ ] 性能指标（§5）在实现中考虑过
- [ ] 错误处理路径覆盖完整

---

## 11. 本文件自维护

- 有新的项目级决策 → 追加到 §9
- 有新的硬约束 → 追加到对应章节
- PRD.md 修改后，检查是否需要同步本文件
- 本文件与 PRD.md 冲突 → 以本文件为准
