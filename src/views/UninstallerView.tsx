import {
  Component,
  createMemo,
  createSignal,
  For,
  onMount,
  Show,
} from "solid-js";
import {
  scanInstalledApps,
  scanAppResidues,
  uninstallApps,
  checkAppRunning,
  quitAndUninstall,
  type InstalledApp,
  type AppResidue,
  type ResidueItem,
  type UninstallTarget,
  type UninstallReport,
} from "@/lib/tauri";
import { fmtBytes } from "@/lib/format";
import { useI18n } from "@/i18n";
import {
  Search,
  X,
  Loader2,
  Package,
  Trash2,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  ChevronDown,
  ChevronRight,
} from "lucide-solid";

// ========== 阶段枚举 ==========
type Phase = "list" | "residue" | "confirm" | "running" | "large-confirm" | "uninstalling" | "done";

const TEN_GB = 10 * 1024 * 1024 * 1024;

const UninstallerView: Component = () => {
  const { t } = useI18n();

  // 应用列表阶段
  const [apps, setApps] = createSignal<InstalledApp[]>([]);
  const [scanning, setScanning] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const [hideSystem, setHideSystem] = createSignal(true);
  const [selectedApps, setSelectedApps] = createSignal(new Set<string>());

  // 残留阶段
  const [phase, setPhase] = createSignal<Phase>("list");
  const [residues, setResidues] = createSignal<AppResidue[]>([]);
  const [residueLoading, setResidueLoading] = createSignal(false);
  const [residueSelection, setResidueSelection] = createSignal(new Map<string, boolean>());
  const [expandedCategories, setExpandedCategories] = createSignal(new Set<string>());

  // 卸载阶段
  const [reports, setReports] = createSignal<UninstallReport[]>([]);
  const [runningApp, setRunningApp] = createSignal<InstalledApp | null>(null);
  const [forceQuitTimer, setForceQuitTimer] = createSignal(0);

  // 扫描已安装应用
  const loadApps = async () => {
    setScanning(true);
    try {
      setApps(await scanInstalledApps());
    } catch (e) {
      console.error("扫描应用失败:", e);
    } finally {
      setScanning(false);
    }
  };

  onMount(loadApps);

  // 过滤后的应用列表
  const filtered = createMemo(() => {
    const q = query().trim().toLowerCase();
    return apps().filter((a) => {
      if (hideSystem() && a.is_system) return false;
      if (q) {
        return a.name.toLowerCase().includes(q) || a.bundle_id.toLowerCase().includes(q);
      }
      return true;
    });
  });

  // 切换应用选中
  const toggleApp = (bundlePath: string) => {
    const next = new Set(selectedApps());
    next.has(bundlePath) ? next.delete(bundlePath) : next.add(bundlePath);
    setSelectedApps(next);
  };

  // 选中的应用对象列表
  const selectedAppList = createMemo(() =>
    apps().filter((a) => selectedApps().has(a.bundle_path)),
  );

  // 预估释放空间
  const estimatedFreeBytes = createMemo(() =>
    selectedAppList().reduce((s, a) => s + a.bundle_size_bytes + a.estimated_residue_bytes, 0),
  );

  // 进入残留扫描阶段
  const enterResiduePhase = async () => {
    const list = selectedAppList();
    if (list.length === 0) return;
    setPhase("residue");
    setResidueLoading(true);
    setResidues([]);
    try {
      const results = await Promise.all(
        list.map((a) => scanAppResidues(a.bundle_id, a.name)),
      );
      setResidues(results);
      // 默认全选所有残留
      const sel = new Map<string, boolean>();
      for (const r of results) {
        for (const item of r.items) {
          sel.set(item.path, item.selected);
        }
      }
      setResidueSelection(sel);
    } catch (e) {
      console.error("残留扫描失败:", e);
    } finally {
      setResidueLoading(false);
    }
  };

  // 残留总大小（选中的）
  const selectedResidueBytes = createMemo(() => {
    const sel = residueSelection();
    let total = 0;
    for (const r of residues()) {
      for (const item of r.items) {
        if (sel.get(item.path)) total += item.size_bytes;
      }
    }
    return total;
  });

  // 总卸载大小
  const totalUninstallBytes = createMemo(() =>
    selectedAppList().reduce((s, a) => s + a.bundle_size_bytes, 0) + selectedResidueBytes(),
  );

  // 切换残留项选中
  const toggleResidue = (path: string) => {
    const next = new Map(residueSelection());
    next.set(path, !next.get(path));
    setResidueSelection(next);
  };

  // 全选/取消全选残留
  const toggleAllResidues = (selectAll: boolean) => {
    const next = new Map<string, boolean>();
    for (const r of residues()) {
      for (const item of r.items) {
        next.set(item.path, selectAll);
      }
    }
    setResidueSelection(next);
  };

  // 切换分类展开
  const toggleCategory = (key: string) => {
    const next = new Set(expandedCategories());
    next.has(key) ? next.delete(key) : next.add(key);
    setExpandedCategories(next);
  };

  // 确认卸载流程
  const startUninstall = async () => {
    // 检查是否有运行中的应用
    for (const app of selectedAppList()) {
      if (app.is_running) {
        try {
          const running = await checkAppRunning(app.bundle_path);
          if (running) {
            setRunningApp(app);
            setPhase("running");
            setForceQuitTimer(5);
            // 5 秒倒计时
            const interval = window.setInterval(() => {
              setForceQuitTimer((v) => {
                if (v <= 1) { clearInterval(interval); return 0; }
                return v - 1;
              });
            }, 1000);
            return;
          }
        } catch { /* 检查失败则继续 */ }
      }
    }
    // 检查 >10GB 二次确认
    if (totalUninstallBytes() > TEN_GB) {
      setPhase("large-confirm");
      return;
    }
    setPhase("confirm");
  };

  // 构建卸载目标
  const buildTargets = (): UninstallTarget[] => {
    const sel = residueSelection();
    return selectedAppList().map((app) => {
      const appResidue = residues().find((r) => r.app_name === app.name);
      const paths = appResidue
        ? appResidue.items.filter((i) => sel.get(i.path)).map((i) => i.path)
        : [];
      return {
        bundle_path: app.bundle_path,
        app_name: app.name,
        bundle_id: app.bundle_id,
        residue_paths: paths,
      };
    });
  };

  // 执行卸载
  const doUninstall = async () => {
    setPhase("uninstalling");
    try {
      const results = await uninstallApps(buildTargets());
      setReports(results);
      setPhase("done");
    } catch (e) {
      console.error("卸载失败:", e);
      setPhase("done");
    }
  };

  // 退出并卸载
  const doQuitAndUninstall = async () => {
    const app = runningApp();
    if (!app) return;
    setPhase("uninstalling");
    try {
      const targets = buildTargets();
      const target = targets.find((t) => t.bundle_path === app.bundle_path);
      if (target) {
        const report = await quitAndUninstall(app.name, target);
        // 处理剩余目标
        const otherTargets = targets.filter((t) => t.bundle_path !== app.bundle_path);
        const otherReports = otherTargets.length > 0 ? await uninstallApps(otherTargets) : [];
        setReports([report, ...otherReports]);
      } else {
        const results = await uninstallApps(targets);
        setReports(results);
      }
      setPhase("done");
    } catch (e) {
      console.error("退出并卸载失败:", e);
      setPhase("done");
    }
  };

  // 完成摘要数据
  const totalFreed = createMemo(() => reports().reduce((s, r) => s + r.total_freed_bytes, 0));
  const totalMoved = createMemo(() => reports().reduce((s, r) => s + r.moved_count, 0));
  const totalFailed = createMemo(() => reports().reduce((s, r) => s + r.failed_count, 0));

  // 重置回列表
  const resetToList = () => {
    setPhase("list");
    setSelectedApps(new Set<string>());
    setResidues([]);
    setResidueSelection(new Map<string, boolean>());
    setReports([]);
    setRunningApp(null);
    loadApps();
  };

  // ========== 渲染：应用列表阶段 ==========
  const AppListView = () => (
    <div class="flex flex-col h-full">
      {/* 顶部工具栏 */}
      <div class="px-6 py-4 border-b border-black/5 dark:border-white/5 flex items-center gap-4">
        <label class="inline-flex items-center gap-2 text-xs text-zinc-600 dark:text-zinc-300 cursor-pointer">
          <input
            type="checkbox"
            checked={hideSystem()}
            onChange={(e) => setHideSystem(e.currentTarget.checked)}
            class="accent-brand-500"
          />
          {t("uninstaller.hideSystem")}
        </label>
        <div class="relative flex-1 max-w-[320px]">
          <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" />
          <input
            type="text"
            placeholder={t("uninstaller.search")}
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
            class="w-full pl-8 pr-8 py-1.5 rounded-lg text-sm bg-black/5 dark:bg-white/5 border border-transparent focus:border-brand-500/50 focus:bg-white dark:focus:bg-zinc-800 outline-none"
          />
          <Show when={query()}>
            <button type="button" onClick={() => setQuery("")} class="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600">
              <X size={12} />
            </button>
          </Show>
        </div>
        <Show when={scanning()}>
          <div class="flex items-center gap-2 text-xs text-zinc-500">
            <Loader2 size={14} class="animate-spin" />
            {t("uninstaller.scanning")}
          </div>
        </Show>
      </div>

      {/* 应用列表 */}
      <div class="flex-1 overflow-y-auto p-4 space-y-2">
        <Show when={!scanning() && filtered().length === 0}>
          <div class="text-center py-20 text-sm text-zinc-500">{t("uninstaller.noApps")}</div>
        </Show>
        <Show when={scanning() && apps().length === 0}>
          <div class="space-y-3">
            <For each={[1, 2, 3, 4, 5]}>
              {() => (
                <div class="card p-4 animate-pulse">
                  <div class="flex items-center gap-3">
                    <div class="w-10 h-10 rounded-xl bg-zinc-200 dark:bg-zinc-700" />
                    <div class="flex-1 space-y-2">
                      <div class="h-4 w-32 bg-zinc-200 dark:bg-zinc-700 rounded" />
                      <div class="h-3 w-48 bg-zinc-100 dark:bg-zinc-800 rounded" />
                    </div>
                    <div class="h-4 w-16 bg-zinc-200 dark:bg-zinc-700 rounded" />
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
        <For each={filtered()}>
          {(app) => {
            const isSelected = () => selectedApps().has(app.bundle_path);
            return (
              <div
                class="card p-4 transition-all duration-200 hover:scale-[1.01] hover:shadow-md cursor-pointer"
                classList={{ "ring-2 ring-brand-500/50": isSelected() }}
                onClick={() => !app.is_system && toggleApp(app.bundle_path)}
              >
                <div class="flex items-center gap-3">
                  <Show when={!app.is_system}>
                    <input
                      type="checkbox"
                      checked={isSelected()}
                      onChange={() => toggleApp(app.bundle_path)}
                      onClick={(e) => e.stopPropagation()}
                      class="w-4 h-4 accent-brand-500 flex-shrink-0"
                    />
                  </Show>
                  <Show when={app.icon_base64} fallback={
                    <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-brand-400 to-brand-600 flex items-center justify-center flex-shrink-0 text-white">
                      <Package size={18} />
                    </div>
                  }>
                    <img
                      src={`data:image/png;base64,${app.icon_base64}`}
                      alt={app.name}
                      class="w-10 h-10 rounded-xl flex-shrink-0"
                    />
                  </Show>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <span class="font-semibold text-sm truncate">{app.name}</span>
                      <Show when={app.is_system}>
                        <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-zinc-500/15 text-zinc-600 dark:text-zinc-400">
                          {t("uninstaller.systemApp")}
                        </span>
                      </Show>
                      <Show when={app.is_running}>
                        <span class="w-2 h-2 rounded-full bg-success-500 flex-shrink-0" title={t("uninstaller.appRunning")} />
                      </Show>
                    </div>
                    <div class="text-[10px] text-zinc-400 truncate font-mono">{app.bundle_id || app.bundle_path}</div>
                  </div>
                  <div class="text-right flex-shrink-0">
                    <div class="text-sm font-semibold tabular-nums">{fmtBytes(app.bundle_size_bytes)}</div>
                    <div class="text-[10px] text-zinc-500">{t("uninstaller.appSize")}</div>
                  </div>
                </div>
              </div>
            );
          }}
        </For>
      </div>

      {/* 底部操作栏 */}
      <Show when={selectedApps().size > 0}>
        <div class="px-6 py-3 border-t border-black/5 dark:border-white/5 flex items-center gap-4 bg-white/50 dark:bg-zinc-900/50 backdrop-blur-sm">
          <span class="text-sm text-zinc-600 dark:text-zinc-300">
            {t("uninstaller.selectedCount", { count: selectedApps().size })}
          </span>
          <span class="text-sm text-zinc-500">
            {t("uninstaller.estimatedFree", { size: fmtBytes(estimatedFreeBytes()) })}
          </span>
          <button
            type="button"
            class="btn-primary gap-2 ml-auto"
            onClick={enterResiduePhase}
          >
            <Trash2 size={16} />
            {t("uninstaller.uninstallSelected")}
          </button>
        </div>
      </Show>
    </div>
  );

  // ========== 渲染：残留详情阶段 ==========
  const ResidueView = () => {
    // 按应用分组，每个应用内按 category 分组
    const groupedResidues = createMemo(() => {
      return residues().map((r) => {
        const groups = new Map<string, ResidueItem[]>();
        for (const item of r.items) {
          const cat = item.is_dev_tool ? t("uninstaller.devToolData") : item.category;
          if (!groups.has(cat)) groups.set(cat, []);
          groups.get(cat)!.push(item);
        }
        return { ...r, groups: Array.from(groups.entries()) };
      });
    });

    const allSelected = createMemo(() => {
      const sel = residueSelection();
      for (const r of residues()) {
        for (const item of r.items) {
          if (!sel.get(item.path)) return false;
        }
      }
      return residues().some((r) => r.items.length > 0);
    });

    return (
      <div class="flex flex-col h-full">
        <div class="px-6 py-4 border-b border-black/5 dark:border-white/5 flex items-center gap-4">
          <button type="button" class="btn-ghost text-xs" onClick={() => setPhase("list")}>
            {t("common.back")}
          </button>
          <span class="text-sm font-medium">
            {t("uninstaller.selectedCount", { count: selectedApps().size })}
          </span>
          <button
            type="button"
            class="ml-auto text-xs text-brand-600 hover:underline"
            onClick={() => toggleAllResidues(!allSelected())}
          >
            {allSelected() ? t("uninstaller.deselectAll") : t("uninstaller.selectAll")}
          </button>
        </div>

        <div class="flex-1 overflow-y-auto p-4 space-y-4">
          <Show when={residueLoading()}>
            <div class="flex items-center gap-2 text-sm text-zinc-500 py-8 justify-center">
              <Loader2 size={16} class="animate-spin" />
              {t("uninstaller.scanning")}
            </div>
          </Show>
          <For each={groupedResidues()}>
            {(appRes) => (
              <div class="card p-4 space-y-3">
                <div class="flex items-center gap-2">
                  <span class="font-semibold text-sm">{appRes.app_name}</span>
                  <span class="text-xs text-zinc-500">{fmtBytes(appRes.total_bytes)}</span>
                  <Show when={!appRes.scan_complete}>
                    <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">
                      {t("uninstaller.residueIncomplete")}
                    </span>
                  </Show>
                </div>
                <For each={appRes.groups}>
                  {([category, items]) => {
                    const catKey = () => `${appRes.bundle_id}:${category}`;
                    const isOpen = () => expandedCategories().has(catKey()) || appRes.groups.length <= 3;
                    return (
                      <div>
                        <button
                          type="button"
                          class="flex items-center gap-2 text-xs text-zinc-600 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200 w-full"
                          onClick={() => toggleCategory(catKey())}
                        >
                          <Show when={isOpen()} fallback={<ChevronRight size={12} />}>
                            <ChevronDown size={12} />
                          </Show>
                          <span class="font-medium">{category}</span>
                          <span class="text-zinc-400">
                            {items.length} · {fmtBytes(items.reduce((s, i) => s + i.size_bytes, 0))}
                          </span>
                        </button>
                        <Show when={isOpen()}>
                          <ul class="mt-1 space-y-0.5 ml-5">
                            <For each={items}>
                              {(item) => (
                                <li class="flex items-center gap-2 py-1 text-xs hover:bg-black/[0.02] dark:hover:bg-white/[0.02] rounded px-1">
                                  <input
                                    type="checkbox"
                                    checked={residueSelection().get(item.path) ?? false}
                                    onChange={() => toggleResidue(item.path)}
                                    class="w-3.5 h-3.5 accent-brand-500"
                                  />
                                  <span class="truncate flex-1 font-mono text-zinc-500">{item.path}</span>
                                  <span class="tabular-nums text-zinc-600 dark:text-zinc-400 flex-shrink-0">
                                    {fmtBytes(item.size_bytes)}
                                  </span>
                                </li>
                              )}
                            </For>
                          </ul>
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </div>
            )}
          </For>
        </div>

        {/* 底部操作栏 */}
        <div class="px-6 py-3 border-t border-black/5 dark:border-white/5 flex items-center gap-4 bg-white/50 dark:bg-zinc-900/50 backdrop-blur-sm">
          <span class="text-sm text-zinc-600 dark:text-zinc-300">
            {t("uninstaller.totalSize")}: {fmtBytes(totalUninstallBytes())}
          </span>
          <button type="button" class="btn-primary gap-2 ml-auto" onClick={startUninstall}>
            <Trash2 size={16} />
            {t("uninstaller.uninstallSelected")}
          </button>
        </div>
      </div>
    );
  };

  // ========== 渲染：确认对话框 ==========
  const ConfirmDialog = (props: { large?: boolean }) => (
    <div class="fixed inset-0 bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center p-6 animate-fade-in" onClick={() => setPhase("residue")}>
      <div class="card p-6 max-w-md w-full animate-slide-up" onClick={(e) => e.stopPropagation()}>
        <div class="flex items-start gap-3">
          <div class="w-10 h-10 rounded-xl bg-warning-500/15 flex items-center justify-center flex-shrink-0">
            <AlertTriangle size={20} class="text-warning-600" />
          </div>
          <div class="flex-1">
            <h3 class="font-semibold">
              {props.large ? t("uninstaller.confirmLargeTitle") : t("uninstaller.confirmTitle")}
            </h3>
            <p class="text-sm text-zinc-500 mt-1">
              {props.large ? t("uninstaller.confirmLargeMessage") : t("uninstaller.confirmMessage")}
            </p>
            <div class="mt-3 text-xs text-zinc-500">
              {t("uninstaller.totalSize")}: {fmtBytes(totalUninstallBytes())}
              {" · "}
              {t("uninstaller.selectedCount", { count: selectedApps().size })}
            </div>
          </div>
        </div>
        <div class="flex justify-end gap-2 mt-5">
          <button type="button" class="btn-ghost" onClick={() => setPhase("residue")}>
            {t("uninstaller.cancel")}
          </button>
          <button
            type="button"
            class="inline-flex items-center justify-center rounded-xl px-5 py-2.5 font-medium bg-danger-500 hover:bg-danger-400 text-white shadow-sm transition-all"
            onClick={doUninstall}
          >
            {t("uninstaller.confirm")}
          </button>
        </div>
      </div>
    </div>
  );

  // ========== 渲染：运行中应用提示 ==========
  const RunningDialog = () => (
    <div class="fixed inset-0 bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center p-6 animate-fade-in" onClick={() => setPhase("residue")}>
      <div class="card p-6 max-w-md w-full animate-slide-up" onClick={(e) => e.stopPropagation()}>
        <div class="flex items-start gap-3">
          <div class="w-10 h-10 rounded-xl bg-warning-500/15 flex items-center justify-center flex-shrink-0">
            <AlertTriangle size={20} class="text-warning-600" />
          </div>
          <div class="flex-1">
            <h3 class="font-semibold">{t("uninstaller.appRunning")}</h3>
            <p class="text-sm text-zinc-500 mt-1">
              {runningApp()?.name} {t("uninstaller.appRunning").toLowerCase()}
            </p>
          </div>
        </div>
        <div class="flex justify-end gap-2 mt-5">
          <button type="button" class="btn-ghost" onClick={() => setPhase("residue")}>
            {t("uninstaller.cancel")}
          </button>
          <Show when={forceQuitTimer() <= 0} fallback={
            <button
              type="button"
              class="inline-flex items-center justify-center rounded-xl px-5 py-2.5 font-medium bg-brand-500 hover:bg-brand-400 text-white shadow-sm transition-all"
              onClick={doQuitAndUninstall}
            >
              {t("uninstaller.quitAndUninstall")}
            </button>
          }>
            <button
              type="button"
              class="inline-flex items-center justify-center rounded-xl px-5 py-2.5 font-medium bg-danger-500 hover:bg-danger-400 text-white shadow-sm transition-all"
              onClick={doQuitAndUninstall}
            >
              {t("uninstaller.forceQuitAndUninstall")}
            </button>
          </Show>
        </div>
      </div>
    </div>
  );

  // ========== 渲染：卸载中 ==========
  const UninstallingView = () => (
    <div class="flex flex-col items-center justify-center h-full gap-4">
      <Loader2 size={32} class="animate-spin text-brand-500" />
      <span class="text-sm text-zinc-600 dark:text-zinc-300">{t("uninstaller.uninstalling")}</span>
    </div>
  );

  // ========== 渲染：完成摘要 ==========
  const DoneView = () => (
    <div class="flex flex-col items-center justify-center h-full gap-6 p-6">
      <div class="w-16 h-16 rounded-full bg-success-500/15 flex items-center justify-center">
        <CheckCircle2 size={32} class="text-success-600" />
      </div>
      <h2 class="text-lg font-semibold">{t("uninstaller.complete")}</h2>
      <div class="text-3xl font-bold tabular-nums text-success-600">
        {fmtBytes(totalFreed())}
      </div>
      <div class="text-sm text-zinc-500 space-y-1 text-center">
        <div>{t("uninstaller.freedSpace", { size: fmtBytes(totalFreed()) })}</div>
        <div>{t("uninstaller.cleanedFiles", { count: totalMoved() })}</div>
        <Show when={totalFailed() > 0}>
          <div class="text-warning-600">{t("uninstaller.failedFiles", { count: totalFailed() })}</div>
        </Show>
      </div>
      {/* 失败项列表 */}
      <Show when={totalFailed() > 0}>
        <div class="card p-4 w-full max-w-lg border-danger-500/20">
          <div class="flex items-center gap-2 mb-2">
            <XCircle size={16} class="text-danger-500" />
            <span class="font-medium text-sm">{t("uninstaller.failedFiles", { count: totalFailed() })}</span>
          </div>
          <ul class="text-xs space-y-1 max-h-40 overflow-y-auto">
            <For each={reports().flatMap((r) => r.details.filter((d) => !d.success))}>
              {(detail) => (
                <li class="flex gap-2">
                  <span class="truncate font-mono text-zinc-500 flex-1">{detail.path}</span>
                  <span class="text-danger-500 flex-shrink-0">{detail.error}</span>
                </li>
              )}
            </For>
          </ul>
        </div>
      </Show>
      <button type="button" class="btn-primary mt-4" onClick={resetToList}>
        {t("common.back")}
      </button>
    </div>
  );

  // ========== 主渲染 ==========
  return (
    <div class="h-full relative">
      <Show when={phase() === "list"}>
        <AppListView />
      </Show>
      <Show when={phase() === "residue"}>
        <ResidueView />
      </Show>
      <Show when={phase() === "confirm"}>
        <ResidueView />
        <ConfirmDialog />
      </Show>
      <Show when={phase() === "large-confirm"}>
        <ResidueView />
        <ConfirmDialog large />
      </Show>
      <Show when={phase() === "running"}>
        <ResidueView />
        <RunningDialog />
      </Show>
      <Show when={phase() === "uninstalling"}>
        <UninstallingView />
      </Show>
      <Show when={phase() === "done"}>
        <DoneView />
      </Show>
    </div>
  );
};

export default UninstallerView;
