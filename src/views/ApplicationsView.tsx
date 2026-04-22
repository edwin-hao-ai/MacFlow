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
  type AppInfo,
} from "@/lib/tauri";
import {
  Package,
  RefreshCw,
  Loader2,
  PowerOff,
  Zap,
  Search,
  X,
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
  const [busy, setBusy] = createSignal<string | null>(null);
  const [message, setMessage] = createSignal<string | null>(null);

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

  const forceQuit = async (app: AppInfo) => {
    setBusy(app.bundle_path);
    setMessage(null);
    try {
      const results = await forceQuitApplication(app.all_pids);
      const failed = results.filter(([, m]) => !m.includes("已终止") && !m.includes("已不存在"));
      if (failed.length === 0) {
        setMessage(`${app.name} 已强制退出`);
      } else {
        setMessage(
          `${app.name} 部分进程未能终止: ${failed.map(([p, m]) => `PID ${p} · ${m}`).join("；")}`,
        );
      }
      await load();
    } catch (e) {
      setMessage(`强制退出 ${app.name} 失败: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const totalMem = createMemo(() => filtered().reduce((s, a) => s + a.memory_mb, 0));
  const totalCPU = createMemo(() => filtered().reduce((s, a) => s + a.cpu_percent, 0));

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
          <Search
            size={14}
            class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
          />
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
          <Show
            when={!loading()}
            fallback={<Loader2 size={12} class="animate-spin" />}
          >
            <RefreshCw size={12} />
          </Show>
        </button>
      </div>

      <div class="flex-1 overflow-y-auto p-4">
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          <For each={filtered()}>
            {(app) => (
              <div class="card p-4 group hover:shadow-md transition-shadow">
                <div class="flex items-start gap-3">
                  <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-brand-400 to-brand-600 flex items-center justify-center flex-shrink-0 text-white">
                    <Package size={18} />
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="font-semibold text-sm truncate">{app.name}</div>
                    <div class="text-[10px] text-zinc-400 truncate font-mono">
                      {app.bundle_id || app.bundle_path.split("/").pop()}
                    </div>
                  </div>
                </div>

                <div class="mt-3 grid grid-cols-3 gap-2 text-center">
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

                <div class="mt-2 flex items-center gap-1.5 flex-wrap min-h-[20px]">
                  <Show when={app.ports.length > 0}>
                    <span
                      class="px-1.5 py-0.5 rounded-md text-[10px] font-mono font-semibold bg-brand-500/15 text-brand-700 dark:text-brand-300"
                      title={`监听端口: ${app.ports.join(", ")}`}
                    >
                      :{app.ports.slice(0, 3).join(",")}
                      {app.ports.length > 3 && `+${app.ports.length - 3}`}
                    </span>
                  </Show>
                  <Show when={app.is_system}>
                    <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-zinc-500/15 text-zinc-600 dark:text-zinc-400">
                      系统
                    </span>
                  </Show>
                  <span class="text-[10px] text-zinc-400 ml-auto">
                    运行 {fmtUptime(app.uptime_secs)}
                  </span>
                </div>

                <div class="mt-3 flex gap-2">
                  <button
                    type="button"
                    class="btn-ghost flex-1 !py-1.5 !text-xs gap-1"
                    disabled={busy() === app.bundle_path}
                    onClick={() => quit(app)}
                    title="发送 AppleScript quit，触发标准退出流程（保存未存文件等）"
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
                    class="flex-1 !py-1.5 !text-xs gap-1 inline-flex items-center justify-center rounded-lg font-medium text-danger-600 hover:bg-danger-500/10 transition-colors"
                    disabled={busy() === app.bundle_path}
                    onClick={() => forceQuit(app)}
                    title="SIGKILL 所有进程（可能丢失未保存数据）"
                  >
                    <Zap size={11} />
                    强制退出
                  </button>
                </div>
              </div>
            )}
          </For>
        </div>

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
    </div>
  );
};

export default ApplicationsView;
