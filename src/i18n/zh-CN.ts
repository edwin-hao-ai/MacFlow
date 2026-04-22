// 中文（简体）翻译 —— MacFlow 的默认文案源
export const zhCN = {
  // 通用
  common: {
    appName: "MacFlow",
    tagline: "Mac 一键优化",
    scan: "扫描",
    rescan: "重新扫描",
    cancel: "取消",
    confirm: "确认",
    close: "关闭",
    back: "返回",
    save: "保存",
    remove: "删除",
    add: "添加",
    loading: "加载中...",
    scanning: "正在扫描...",
    optimizing: "正在优化...",
    cleaning: "正在清理...",
    version: "版本",
    notice_irreversible: "此操作不可撤销，请确认后执行",
  },

  // 导航
  nav: {
    scan: "智能扫描",
    process: "进程管理",
    applications: "应用程序",
    cache: "缓存清理",
    docker: "Docker",
    history: "历史记录",
    settings: "设置",
  },

  // 欢迎页
  welcome: {
    title: "欢迎使用 MacFlow",
    subtitle: "Mac 专属的一键式系统运维工具。\n清理冗余进程和开发缓存，让 Mac 保持轻快。",
    featureProcessTitle: "进程优化",
    featureProcessDesc: "识别残留、重复、高占用进程",
    featureCacheTitle: "缓存清理",
    featureCacheDesc: "NPM · Docker · Xcode · Homebrew",
    featureSafetyTitle: "安全可审计",
    featureSafetyDesc: "规则驱动 · 工具原生命令 · 路径白名单",
    cta: "开始扫描",
    footer: "所有操作本地执行 · 不上传数据 · 不接 AI API",
  },

  // 系统健康
  health: {
    title: "系统健康",
    subtitle: "实时监控 CPU / 内存 / 磁盘",
    reading: "读取中...",
    normal: "正常运行",
    cpu: "CPU",
    memory: "内存",
    disk: "磁盘",
  },

  // 扫描视图
  scan: {
    processListTitle: "可优化进程",
    itemsCount: "{count} 项",
    noProcesses: "没有发现可优化的进程，系统运行良好",
    oneClick: "一键优化",
    selectedCount: "已选 {count} 项",
    killSuccess: "已终止 {count} 个进程",
    killPartial: "，{failed} 个失败",
    whitelistAdded: "{name} 已加入白名单，下次扫描不会再显示",
    scanFailed: "扫描失败: {error}",
    optimizeFailed: "优化失败: {error}",
    whitelistTooltip: "加入白名单（永不再扫描此进程）",
  },

  // 进程分类标签
  kind: {
    zombie: "僵尸进程",
    idle: "长期闲置",
    hog: "资源大户",
    dev: "开发工具",
    system: "系统进程",
    foreground: "前台活跃",
  },

  risk: {
    safe: "安全",
    low: "低风险",
    dev: "开发",
    notice: "注意",
  },

  // 缓存清理
  cache: {
    title: "开发者缓存",
    subtitle: "NPM / Docker / Xcode / Homebrew / Cargo 等",
    scanning: "正在扫描缓存...",
    freeable: "可释放空间",
    cleanCta: "清理 {size} ({count})",
    clean: "清理",
    cleanSuccess: "清理完成，释放 {size}",
    successItems: "成功 {count} 项",
    failItems: "失败 {count} 项",
    groupCount: "{count} 项 · {size}",
    noItems: "没有发现可清理的缓存。你的 Mac 很干净！",
    partialFail: "部分项目清理失败",
    notifyTitle: "MacFlow 清理完成",
    notifyBody: "已释放 {size}，共清理 {count} 项",
  },

  // 历史
  history: {
    title: "操作历史",
    subtitle: "所有进程和缓存清理操作的本地日志。只记录元数据，不上传任何内容。",
    empty: "还没有任何操作记录",
    opProcessKill: "进程清理",
    opCacheClean: "缓存清理",
  },

  // 设置
  settings: {
    general: "通用设置",
    generalDesc: "基础行为开关",
    autostart: "开机自动启动",
    autostartDesc: "macOS 登录时自动启动 MacFlow 并最小化到菜单栏",
    notifyClean: "清理完成通知",
    notifyCleanDesc: "在 macOS 通知中心提示已释放空间",
    notifyRequestPrompt: "通知权限未授权，点此请求",
    language: "界面语言",
    languageDesc: "切换中英文（需重启应用生效）",
    languageAuto: "跟随系统",
    languageZh: "中文",
    languageEn: "English",
    updates: "检查更新",
    updatesDesc: "手动检查 MacFlow 是否有新版本",
    updatesCheck: "检查",
    updatesChecking: "检查中...",
    updatesLatest: "已是最新版本 ({version})",
    updatesAvailable: "有新版本 {version} 可用",
    updatesDownload: "下载并重启",
    updatesDownloading: "下载中...",
    updatesError: "检查失败: {error}",
    whitelist: "自定义白名单",
    whitelistCount: "已添加 {count} 项 · 白名单项永远不会被扫描或清理",
    whitelistEmpty: "还没有自定义白名单。在扫描列表上点击盾牌图标可以快速添加。",
    whitelistKindProcess: "进程名",
    whitelistKindPath: "缓存路径",
    whitelistKindProcessPH: "例如 Chrome",
    whitelistKindPathPH: "例如 ~/.npm",
    whitelistNotePH: "备注（可选）",
    whitelistBadgeProcess: "进程",
    whitelistBadgePath: "路径",
    about: "关于 MacFlow",
    aboutLine1: "版本 0.1.0 · 规则驱动 · 本地存储 · 开源友好",
    aboutLine2: "不上传任何数据，不接任何 LLM，不做广告推送",
    aboutDb: "数据：~/Library/Application Support/MacFlow/macflow.db",
    aboutCli: "CLI：~/MacFlow/src-tauri/target/debug/macflow-cli",
  },

  // 占位符
  placeholder: {
    processTitle: "进程管理",
    processDesc: "高级筛选、白名单快速添加、端口占用查看。里程碑 3 上线。",
    cacheTitle: "缓存清理",
    cacheDesc: "NPM / Docker / Xcode / Homebrew 深度清理。里程碑 2 上线。",
    settingsTitle: "设置",
    settingsDesc: "开机启动、自动监控、清理阈值等。里程碑 3 上线。",
  },
};

export type Dict = typeof zhCN;
