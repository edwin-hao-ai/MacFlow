import {
  Component,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import {
  listApplications,
  quitApplication,
  forceQuitApplication,
  killProcesses,
  type AppInfo,
  type AppChildProcess,
} from "@/lib/tauri";
import {
  Package,
  RefreshCw,
  Loader2,
  PowerOff,
  Zap,
  Search,
  X,
  ChevronRight,
  ChevronDown,
  Trash2,
  ShieldAlert,
  ShieldCheck,
  AlertTriangle,
} from "lucide-solid";
import { fmtBytes } from "@/lib/format";

function fmtUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h`;
  return `${Math.floor(secs / 86400)}d`;
}

const ApplicationsView: Component = () => {
  const [apps, setApps] = createSignal<AppInfo[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const [hideSystem, setHideSystem] = createSignal(true);
  const [expanded, setExpanded] = createSignal(new Set<string>());
  const [busy, setBusy] = createSignal<string | null>(null);
  const [message, setMessage] = createSignal<string | null>(null);
  const [confirmForceQuit, setConfirmForceQuit] = createSignal<AppInfo | null>(null);
  const [confirmChildKill, setConfirmChildKill] =
    createSignal<null | { app: AppInfo; child: AppChildProcess }>(null);

  const load = async () => {
    setLoading(true);
    try {
      setApps(await listApplications());
    } catch (e) {
      setMessage(String(e));
    } finally {
      setLoading(false);
    }
  };

  let timer: number | undefined;
  onMount(() => {
    load();
    timer = window.setInterval(load, 5000);
  });
  onCleanup(() => {
    if (timer) clearInterval(timer);
  });

  const filtered = createMemo(() => {
    const q = query().trim().toLowerCase();
    return apps().filter((a) => {
      if (hideSystem() && a.is_system) return false;
      if (q) {
        return (
          a.name.toLowerCase().includes(q) ||
          a.bundle_id.toLowerCase().includes(q) ||
          a.bundle_path.toLowerCase().includes(q)
        );
      }
      return true;
    });
  });

  const toggleExpand = (key: string) => {
    const next = new Set(expanded());
    next.has(key) ? next.delete(key) : next.add(key);
    setExpanded(next);
  };

  const quit = async (app: AppInfo) => {
    setBusy(app.bundle_path);
    setMessage(null);
    try {
      await quitApplication(app.name);
      setMessage(`已发送退出信号给 ${app.name}`);
      await load();
    } catch (e) {
      setMessage(`退出 ${app.name} 失败: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const doForceQuit = async (app: AppInfo) => {
    setBusy(app.bundle_path);
    setMessage(null);
    try {
      const results = await forceQuitApplication(app.all_pids);
      const failed = results.filter(
        ([, m]) => !m.includes("已终止") && !m.includes("已不存在"),
      );
      if (failed.length === 0) {
        setMessage(`${app.name} 已强制退出`);
      } else {
        setMessage(
          `${app.name} 部分进程未能终止：${failed
            .map(([p, m]) => `PID ${p} · ${m}`)
            .join("；")}`,
        );
      }
      await load();
    } catch (e) {
      setMessage(`强制退出 ${app.name} 失败: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const forceQuit = async (app: AppInfo) => {
    if (app.is_system || app.protected_process_count > 0) {
      setConfirmForceQuit(app);
      return;
    }
    await doForceQuit(app);
  };

  const doKillChild = async (app: AppInfo, child: AppChildProcess) => {
    setBusy(`${app.bundle_path}:${child.pid}`);
    setMessage(null);
    try {
      const r = await killProcesses([child.pid], [child.name]);
      if (r.killed.length > 0) {
        setMessage(
          `已终止 ${child.name} (PID ${child.pid})${
            child.is_main ? "（这是主进程，整个应用将退出）" : ""
          }`,
        );
      } else if (r.details[0]) {
        setMessage(`${child.name}: ${r.details[0].message}`);
      }
      await load();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(null);
    }
  };

  const killChild = async (app: AppInfo, child: AppChildProcess) => {
    if (child.protected) {
      setConfirmChildKill({ app, child });
      return;
    }
    await doKillChild(app, child);
  };

  const totalMem = createMemo(() =>
    filtered().reduce((s, a) => s + a.memory_mb, 0),
  );
  const totalCPU = createMemo(() =>
    filtered().reduce((s, a) => s + a.cpu_percent, 0),
  );

  return (
    <div class="flex flex-col h-full">
      <div class="px-6 py-4 border-b border-black/5 dark:border-white/5 flex items-center gap-4">
        <div class="flex-1 flex gap-6">
          <div>
            <div class="text-xs text-zinc-500">应用数</div>
            <div class="text-lg font-semibold tabular-nums">
              {filtered().length}
            </div>
          </div>
          <div>
            <div class="text-xs text-zinc-500">总内存</div>
            <div class="text-lg font-semibold tabular-nums">
              {(totalMem() / 1024).toFixed(1)}
              <span class="text-xs text-zinc-500 ml-1">GB</span>
            </div>
          </div>
          <div>
            <div class="text-xs text-zinc-500">总 CPU</div>
            <div class="text-lg font-semibold tabular-nums">
              {totalCPU().toFixed(1)}
              <span class="text-xs text-zinc-500 ml-1">%</span>
            </div>
          </div>
        </div>

        <label class="inline-flex items-center gap-2 text-xs text-zinc-600 dark:text-zinc-300 cursor-pointer">
          <input
            type="checkbox"
            checked={hideSystem()}
            onChange={(e) => setHideSystem(e.currentTarget.checked)}
            class="accent-brand-500"
          />
          隐藏系统应用
        </label>

        <div class="relative">
          <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" />
          <input
            type="text"
            placeholder="搜索应用..."
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
            class="pl-8 pr-8 py-1.5 rounded-lg text-sm bg-black/5 dark:bg-white/5 border border-transparent focus:border-brand-500/50 focus:bg-white dark:focus:bg-zinc-800 outline-none w-[220px]"
          />
          <Show when={query()}>
            <button
              type="button"
              onClick={() => setQuery("")}
              class="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600"
            >
              <X size={12} />
            </button>
          </Show>
        </div>

        <button
          type="button"
          class="btn-ghost gap-1.5"
          disabled={loading()}
          onClick={load}
        >
          <Show when={!loading()} fallback={<Loader2 size={12} class="animate-spin" />}>
            <RefreshCw size={12} />
          </Show>
        </button>
      </div>

      <div class="flex-1 overflow-y-auto p-4 space-y-3">
        <For each={filtered()}>
          {(app) => {
            const isOpen = () => expanded().has(app.bundle_path);
            return (
              <div class="card overflow-hidden">
                {/* 主卡片行（主应用） */}
                <div class="p-4">
                  <div class="flex items-center gap-3">
                    <button
                      type="button"
                      class="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-zinc-700"
                      onClick={() => toggleExpand(app.bundle_path)}
                      title={isOpen() ? "折叠" : "展开子进程"}
                    >
                      <Show
                        when={isOpen()}
                        fallback={<ChevronRight size={14} />}
                      >
                        <ChevronDown size={14} />
                      </Show>
                    </button>

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
                        <div class="font-semibold text-sm truncate">
                          {app.name}
                        </div>
                        <Show when={app.is_system}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-zinc-500/15 text-zinc-600 dark:text-zinc-400">
                            系统
                          </span>
                        </Show>
                        <Show when={app.whitelisted_process_count > 0}>
                          <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-brand-500/15 text-brand-600">
                            <ShieldCheck size={10} />
                            白名单
                          </span>
                        </Show>
                        <Show when={app.protected_process_count > 0}>
                          <span class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">
                            <ShieldAlert size={10} />
                            谨慎
                          </span>
                        </Show>
                        <Show when={app.ports.length > 0}>
                          <span
                            class="px-1.5 py-0.5 rounded-md text-[10px] font-mono font-semibold bg-brand-500/15 text-brand-700 dark:text-brand-300"
                            title={`监听: ${app.ports.join(", ")}`}
                          >
                            :{app.ports.slice(0, 3).join(",")}
                            {app.ports.length > 3 && `+${app.ports.length - 3}`}
                          </span>
                        </Show>
                      </div>
                      <div class="text-[10px] text-zinc-400 truncate font-mono">
                        {app.bundle_id || app.bundle_path.split("/").pop()}
                      </div>
                    </div>

                    {/* 右侧指标 */}
                    <div class="grid grid-cols-3 gap-3 text-center flex-shrink-0 mr-2">
                      <div>
                        <div class="text-[10px] text-zinc-500">内存</div>
                        <div class="text-sm font-semibold tabular-nums">
                          {fmtBytes(app.memory_mb * 1024 * 1024)}
                        </div>
                      </div>
                      <div>
                        <div class="text-[10px] text-zinc-500">CPU</div>
                        <div
                          class="text-sm font-semibold tabular-nums"
                          classList={{
                            "text-danger-600": app.cpu_percent > 50,
                            "text-warning-600":
                              app.cpu_percent > 20 && app.cpu_percent <= 50,
                          }}
                        >
                          {app.cpu_percent.toFixed(1)}%
                        </div>
                      </div>
                      <div>
                        <div class="text-[10px] text-zinc-500">进程</div>
                        <div class="text-sm font-semibold tabular-nums">
                          {app.all_pids.length}
                        </div>
                      </div>
                    </div>

                    <span class="text-[10px] text-zinc-400 flex-shrink-0">
                      {fmtUptime(app.uptime_secs)}
                    </span>

                    {/* 操作按钮 */}
                    <div class="flex gap-1 flex-shrink-0">
                      <button
                        type="button"
                        class="btn-ghost !py-1.5 !px-2 !text-xs gap-1"
                        disabled={busy() === app.bundle_path}
                        onClick={() => quit(app)}
                        title="发送 AppleScript quit（触发保存提示）"
                      >
                        <Show
                          when={busy() !== app.bundle_path}
                          fallback={<Loader2 size={11} class="animate-spin" />}
                        >
                          <PowerOff size={11} />
                        </Show>
                        退出
                      </button>
                      <button
                        type="button"
                        class="!py-1.5 !px-2 !text-xs gap-1 inline-flex items-center justify-center rounded-lg font-medium text-danger-600 hover:bg-danger-500/10 transition-colors"
                        disabled={busy() === app.bundle_path}
                        onClick={() => forceQuit(app)}
                        title="强制终止该应用全部进程（不可撤销，可能丢数据）"
                      >
                        <Zap size={11} />
                        强制退出
                      </button>
                    </div>
                  </div>
                </div>

                {/* 子进程树（展开时显示） */}
                <Show when={isOpen() && app.children.length > 0}>
                  <div class="border-t border-black/5 dark:border-white/5 bg-black/[0.02] dark:bg-white/[0.02]">
                    <div class="px-4 py-2 text-[10px] font-medium text-zinc-500 grid grid-cols-[1fr_72px_72px_56px_56px] gap-2">
                      <div>子进程</div>
                      <div class="text-right">CPU</div>
                      <div class="text-right">内存</div>
                      <div class="text-right">PID</div>
                      <div class="text-right">操作</div>
                    </div>
                    <For each={app.children}>
                      {(child) => (
                        <div class="px-4 py-1.5 grid grid-cols-[1fr_72px_72px_56px_56px] gap-2 items-center text-xs hover:bg-black/[0.03] dark:hover:bg-white/[0.03] group">
                          <div class="min-w-0">
                            <div class="flex items-center gap-2">
                              <div
                                style={{
                                  "padding-left": `${child.depth * 14 + 4}px`,
                                }}
                                class="text-zinc-400 font-mono flex-shrink-0"
                              >
                                <Show when={child.depth > 0}>
                                  <span>└</span>
                                </Show>
                              </div>
                              <span
                                class="truncate"
                                classList={{
                                  "font-semibold": child.is_main,
                                  "text-zinc-700 dark:text-zinc-300":
                                    child.is_main,
                                }}
                              >
                                {child.name}
                              </span>
                              <Show when={child.is_main}>
                                <span class="px-1 py-0 rounded text-[9px] font-medium bg-brand-500/15 text-brand-600">
                                  主
                                </span>
                              </Show>
                              <Show when={child.whitelisted}>
                                <span class="inline-flex items-center gap-1 px-1 py-0 rounded text-[9px] font-medium bg-brand-500/15 text-brand-600">
                                  <ShieldCheck size={9} />
                                  白名单
                                </span>
                              </Show>
                              <Show when={child.protected && !child.whitelisted}>
                                <span class="inline-flex items-center gap-1 px-1 py-0 rounded text-[9px] font-medium bg-warning-500/15 text-warning-600">
                                  <ShieldAlert size={9} />
                                  谨慎
                                </span>
                              </Show>
                              <Show when={child.ports.length > 0}>
                                <span class="px-1 py-0 rounded text-[9px] font-mono font-semibold bg-brand-500/10 text-brand-700 dark:text-brand-300">
                                  :{child.ports.join(",")}
                                </span>
                              </Show>
                            </div>
                            <Show when={child.protected_reason}>
                              <div class="text-[10px] text-warning-600 dark:text-warning-400 truncate mt-0.5">
                                {child.protected_reason}
                              </div>
                            </Show>
                          </div>
                          <div class="text-right tabular-nums text-[11px]">
                            {child.cpu_percent.toFixed(1)}%
                          </div>
                          <div class="text-right tabular-nums text-[11px] text-zinc-600 dark:text-zinc-400">
                            {child.memory_mb < 1024
                              ? `${child.memory_mb.toFixed(0)}M`
                              : `${(child.memory_mb / 1024).toFixed(1)}G`}
                          </div>
                          <div class="text-right tabular-nums text-[10px] text-zinc-400 font-mono">
                            {child.pid}
                          </div>
                          <div class="flex items-center justify-end opacity-0 group-hover:opacity-100 transition-opacity">
                            <button
                              type="button"
                              class="p-1 rounded-md text-zinc-400 hover:text-danger-500 hover:bg-danger-500/10"
                              disabled={
                                busy() === `${app.bundle_path}:${child.pid}`
                              }
                              onClick={() => killChild(app, child)}
                              title={
                                child.is_main
                                  ? "终止主进程（整个应用会退出）"
                                  : "只终止这一个子进程"
                              }
                            >
                              <Show
                                when={
                                  busy() !==
                                  `${app.bundle_path}:${child.pid}`
                                }
                                fallback={<Loader2 size={11} class="animate-spin" />}
                              >
                                <Trash2 size={11} />
                              </Show>
                            </button>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>
              </div>
            );
          }}
        </For>

        <Show when={filtered().length === 0 && !loading()}>
          <div class="text-center py-20 text-sm text-zinc-500">
            {query() || hideSystem()
              ? "没有符合条件的应用"
              : "没有运行中的应用"}
          </div>
        </Show>
      </div>

      <Show when={message()}>
        <div class="px-6 py-2 border-t border-black/5 dark:border-white/5 text-xs text-zinc-600 dark:text-zinc-400">
          {message()}
        </div>
      </Show>

      <Show when={confirmForceQuit()}>
        {(app) => (
          <div
            class="fixed inset-0 bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center p-6 animate-fade-in"
            onClick={() => setConfirmForceQuit(null)}
          >
            <div
              class="card p-6 max-w-md w-full animate-slide-up"
              onClick={(e) => e.stopPropagation()}
            >
              <div class="flex items-start gap-3">
                <div class="w-10 h-10 rounded-xl bg-warning-500/15 flex items-center justify-center flex-shrink-0">
                  <AlertTriangle size={20} class="text-warning-600" />
                </div>
                <div class="flex-1">
                  <h3 class="font-semibold">确认强制退出 {app().name}？</h3>
                  <p class="text-sm text-zinc-500 mt-1">
                    这个应用包含受保护或白名单进程。强制退出会直接终止整个进程树，操作不可撤销。
                  </p>
                  <div class="mt-3 text-xs text-zinc-500">
                    受保护进程 {app().protected_process_count} 个，白名单进程 {app().whitelisted_process_count} 个。
                  </div>
                </div>
              </div>
              <div class="flex justify-end gap-2 mt-5">
                <button
                  type="button"
                  class="btn-ghost"
                  onClick={() => setConfirmForceQuit(null)}
                >
                  取消
                </button>
                <button
                  type="button"
                  class="inline-flex items-center justify-center rounded-xl px-5 py-2.5 font-medium bg-danger-500 hover:bg-danger-400 text-white shadow-sm transition-all"
                  onClick={async () => {
                    const target = app();
                    setConfirmForceQuit(null);
                    await doForceQuit(target);
                  }}
                >
                  仍要强制退出
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>

      <Show when={confirmChildKill()}>
        {(data) => (
          <div
            class="fixed inset-0 bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center p-6 animate-fade-in"
            onClick={() => setConfirmChildKill(null)}
          >
            <div
              class="card p-6 max-w-md w-full animate-slide-up"
              onClick={(e) => e.stopPropagation()}
            >
              <div class="flex items-start gap-3">
                <div class="w-10 h-10 rounded-xl bg-warning-500/15 flex items-center justify-center flex-shrink-0">
                  <AlertTriangle size={20} class="text-warning-600" />
                </div>
                <div class="flex-1">
                  <h3 class="font-semibold">确认终止子进程？</h3>
                  <p class="text-sm text-zinc-500 mt-1">
                    {data().child.name} 已被标记为谨慎项，终止后可能导致 {data().app.name} 异常、崩溃或立即退出。
                  </p>
                  <div class="mt-3 text-xs text-warning-600 dark:text-warning-400">
                    {data().child.protected_reason}
                  </div>
                </div>
              </div>
              <div class="flex justify-end gap-2 mt-5">
                <button
                  type="button"
                  class="btn-ghost"
                  onClick={() => setConfirmChildKill(null)}
                >
                  取消
                </button>
                <button
                  type="button"
                  class="inline-flex items-center justify-center rounded-xl px-5 py-2.5 font-medium bg-danger-500 hover:bg-danger-400 text-white shadow-sm transition-all"
                  onClick={async () => {
                    const current = data();
                    setConfirmChildKill(null);
                    await doKillChild(current.app, current.child);
                  }}
                >
                  仍要终止
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>
    </div>
  );
};

export default ApplicationsView;
