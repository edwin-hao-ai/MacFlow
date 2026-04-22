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
  addWhitelist,
  killProcesses,
  listAllProcesses,
  type KillResult,
  type ProcessRow,
} from "@/lib/tauri";
import {
  Loader2,
  Network,
  RefreshCw,
  Search,
  ShieldCheck,
  ShieldAlert,
  X,
  ArrowDown,
  ArrowUp,
} from "lucide-solid";
import { useI18n } from "@/i18n";

type SortKey = "memory" | "cpu" | "name" | "pid" | "uptime";

function fmtUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h`;
  return `${Math.floor(secs / 86400)}d`;
}

const ProcessView: Component = () => {
  const { t } = useI18n();
  const [rows, setRows] = createSignal<ProcessRow[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const [portsOnly, setPortsOnly] = createSignal(false);
  const [sortKey, setSortKey] = createSignal<SortKey>("memory");
  const [sortDir, setSortDir] = createSignal<"desc" | "asc">("desc");
  const [selected, setSelected] = createSignal(new Set<number>());
  const [busy, setBusy] = createSignal(false);
  const [message, setMessage] = createSignal<string | null>(null);
  const [killDetails, setKillDetails] = createSignal<KillResult[] | null>(null);

  const load = async () => {
    setLoading(true);
    try {
      const r = await listAllProcesses();
      setRows(r);
    } catch (e) {
      setMessage(String(e));
    } finally {
      setLoading(false);
    }
  };

  let timer: number | undefined;
  onMount(() => {
    load();
    // 每 5 秒自动刷新
    timer = window.setInterval(load, 5000);
  });
  onCleanup(() => {
    if (timer) clearInterval(timer);
  });

  const filtered = createMemo(() => {
    const q = query().trim().toLowerCase();
    let list = rows();

    // 仅端口占用筛选：方便开发者快速找到占用 3000 / 8080 等端口的进程
    if (portsOnly()) {
      list = list.filter((r) => r.ports.length > 0);
    }

    if (q) {
      list = list.filter(
        (r) =>
          r.name.toLowerCase().includes(q) ||
          String(r.pid).includes(q) ||
          r.exe.toLowerCase().includes(q) ||
          r.ports.some((p) => String(p).includes(q)),
      );
    }

    const key = sortKey();
    const dir = sortDir() === "desc" ? -1 : 1;
    list = [...list].sort((a, b) => {
      // 端口模式下按最小端口号排序
      if (portsOnly() && key === "memory") {
        const aPort = a.ports[0] ?? Number.MAX_SAFE_INTEGER;
        const bPort = b.ports[0] ?? Number.MAX_SAFE_INTEGER;
        return aPort - bPort;
      }
      switch (key) {
        case "memory":
          return dir * (a.memory_mb - b.memory_mb);
        case "cpu":
          return dir * (a.cpu_percent - b.cpu_percent);
        case "name":
          return dir * a.name.localeCompare(b.name);
        case "pid":
          return dir * (a.pid - b.pid);
        case "uptime":
          return dir * (a.uptime_secs - b.uptime_secs);
      }
    });
    return list;
  });

  const toggle = (pid: number, allowed: boolean) => {
    if (!allowed) return;
    const next = new Set(selected());
    next.has(pid) ? next.delete(pid) : next.add(pid);
    setSelected(next);
  };

  const onHeaderClick = (key: SortKey) => {
    if (sortKey() === key) {
      setSortDir(sortDir() === "desc" ? "asc" : "desc");
    } else {
      setSortKey(key);
      setSortDir(key === "name" || key === "pid" ? "asc" : "desc");
    }
  };

  const killSelected = async () => {
    const pids = Array.from(selected());
    if (pids.length === 0) return;
    const names = pids.map(
      (pid) => rows().find((r) => r.pid === pid)?.name ?? String(pid),
    );
    setBusy(true);
    setMessage(null);
    setKillDetails(null);
    try {
      const r = await killProcesses(pids, names);
      let msg = t("scan.killSuccess", { count: r.killed.length });
      if (r.failed.length > 0) {
        msg += t("scan.killPartial", { failed: r.failed.length });
        // 只展示失败的细节
        setKillDetails(r.details.filter((d) => !d.success));
      }
      setMessage(msg);
      setSelected(new Set<number>());
      await load();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  };

  const whitelist = async (name: string) => {
    await addWhitelist("process", name, "process view add");
    setMessage(t("scan.whitelistAdded", { name }));
    await load();
  };

  const totalMem = createMemo(() =>
    rows().reduce((s, r) => s + r.memory_mb, 0),
  );
  const totalCPU = createMemo(() =>
    rows().reduce((s, r) => s + r.cpu_percent, 0),
  );

  const selectableCount = createMemo(
    () => filtered().filter((r) => !r.protected).length,
  );

  const SortHeader: Component<{ k: SortKey; label: string; class?: string }> = (
    p,
  ) => (
    <button
      type="button"
      onClick={() => onHeaderClick(p.k)}
      class={`flex items-center gap-1 hover:text-zinc-900 dark:hover:text-zinc-100 transition-colors ${p.class ?? ""}`}
    >
      <span>{p.label}</span>
      <Show when={sortKey() === p.k}>
        {sortDir() === "desc" ? <ArrowDown size={10} /> : <ArrowUp size={10} />}
      </Show>
    </button>
  );

  return (
    <div class="flex flex-col h-full">
      {/* 顶部统计栏 */}
      <div class="px-6 py-4 border-b border-black/5 dark:border-white/5 flex items-center gap-4">
        <div class="flex-1 flex gap-6">
          <div>
            <div class="text-xs text-zinc-500">共</div>
            <div class="text-lg font-semibold tabular-nums">
              {rows().length}
              <span class="text-xs text-zinc-500 ml-1">项</span>
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
          <div>
            <div class="text-xs text-zinc-500">已选</div>
            <div class="text-lg font-semibold tabular-nums text-brand-600">
              {selected().size}
            </div>
          </div>
        </div>

        <button
          type="button"
          onClick={() => setPortsOnly(!portsOnly())}
          title={portsOnly() ? "关闭端口筛选" : "只看占用端口的进程"}
          class={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            portsOnly()
              ? "bg-brand-500 text-white"
              : "bg-black/5 dark:bg-white/5 text-zinc-600 dark:text-zinc-300 hover:bg-black/10 dark:hover:bg-white/10"
          }`}
        >
          <Network size={12} />
          仅端口占用
        </button>

        <div class="relative">
          <Search
            size={14}
            class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
          />
          <input
            type="text"
            placeholder="搜索 进程名 / PID / 端口..."
            value={query()}
            onInput={(e) => setQuery(e.currentTarget.value)}
            class="pl-8 pr-8 py-1.5 rounded-lg text-sm bg-black/5 dark:bg-white/5 border border-transparent focus:border-brand-500/50 focus:bg-white dark:focus:bg-zinc-800 outline-none w-[260px]"
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

      {/* 表头 */}
      <div class="px-6 py-2 grid grid-cols-[1fr_72px_72px_64px_56px_96px] gap-2 text-[11px] font-medium text-zinc-500 border-b border-black/5 dark:border-white/5">
        <SortHeader k="name" label="进程" />
        <SortHeader k="cpu" label="CPU" class="justify-end" />
        <SortHeader k="memory" label="内存" class="justify-end" />
        <SortHeader k="uptime" label="运行" class="justify-end" />
        <SortHeader k="pid" label="PID" class="justify-end" />
        <div class="text-right">操作</div>
      </div>

      {/* 表格主体 */}
      <div class="flex-1 overflow-y-auto">
        <Show
          when={filtered().length > 0}
          fallback={
            <div class="py-20 text-center text-sm text-zinc-500">
              <Show
                when={!loading() && rows().length === 0}
                fallback={
                  <Show when={!loading()}>
                    没有匹配 "{query()}" 的进程
                  </Show>
                }
              >
                没有可见进程
              </Show>
            </div>
          }
        >
          <For each={filtered()}>
            {(r) => (
              <div
                class="px-6 py-1.5 grid grid-cols-[1fr_72px_72px_64px_56px_96px] gap-2 items-center text-sm hover:bg-black/[0.02] dark:hover:bg-white/[0.02] cursor-default group"
                classList={{ "opacity-50": r.protected }}
              >
                <div class="min-w-0 flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={selected().has(r.pid)}
                    disabled={r.protected}
                    onChange={() => toggle(r.pid, !r.protected)}
                    class="w-4 h-4 rounded accent-brand-500 flex-shrink-0"
                    title={
                      r.protected
                        ? r.protected_reason ?? "受保护进程"
                        : undefined
                    }
                  />
                  <Show when={r.protected}>
                    <ShieldAlert
                      size={12}
                      class="text-warning-500 flex-shrink-0"
                    />
                  </Show>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 min-w-0">
                      <span class="truncate font-medium">{r.name}</span>
                      <Show when={r.ports.length > 0}>
                        <span
                          class="px-1.5 py-0.5 rounded-md text-[10px] font-mono font-semibold bg-brand-500/15 text-brand-700 dark:text-brand-300"
                          title={`监听端口: ${r.ports.join(", ")}`}
                        >
                          :
                          {portsOnly()
                            ? r.ports.join(", ")
                            : r.ports.length <= 2
                              ? r.ports.join(",")
                              : `${r.ports.slice(0, 2).join(",")}+${r.ports.length - 2}`}
                        </span>
                      </Show>
                      <Show when={r.status === "僵尸" || r.status === "已死"}>
                        <span class="px-1.5 py-0.5 rounded-md text-[9px] font-medium bg-danger-500/15 text-danger-600">
                          {r.status}
                        </span>
                      </Show>
                    </div>
                    <Show when={r.protected && r.protected_reason}>
                      <div class="text-[10px] text-zinc-500 truncate">
                        {r.protected_reason}
                      </div>
                    </Show>
                    <Show when={!r.protected && r.exe}>
                      <div class="text-[10px] text-zinc-400 font-mono truncate">
                        {r.exe}
                      </div>
                    </Show>
                  </div>
                </div>
                <div class="text-right tabular-nums text-xs">
                  <span
                    classList={{
                      "text-danger-600 font-semibold": r.cpu_percent > 50,
                      "text-warning-600": r.cpu_percent > 20 && r.cpu_percent <= 50,
                      "text-zinc-600 dark:text-zinc-400": r.cpu_percent <= 20,
                    }}
                  >
                    {r.cpu_percent.toFixed(1)}%
                  </span>
                </div>
                <div class="text-right tabular-nums text-xs text-zinc-600 dark:text-zinc-400">
                  {r.memory_mb < 100
                    ? `${r.memory_mb.toFixed(0)}M`
                    : r.memory_mb < 1024
                      ? `${r.memory_mb.toFixed(0)}M`
                      : `${(r.memory_mb / 1024).toFixed(1)}G`}
                </div>
                <div class="text-right tabular-nums text-[11px] text-zinc-500">
                  {fmtUptime(r.uptime_secs)}
                </div>
                <div class="text-right tabular-nums text-[11px] text-zinc-400 font-mono">
                  {r.pid}
                </div>
                <div class="flex items-center justify-end gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button
                    type="button"
                    title={t("scan.whitelistTooltip")}
                    onClick={() => whitelist(r.name)}
                    class="p-1 rounded-md text-zinc-400 hover:text-brand-600 hover:bg-brand-500/10"
                  >
                    <ShieldCheck size={13} />
                  </button>
                </div>
              </div>
            )}
          </For>
        </Show>
      </div>

      {/* 失败详情展开 */}
      <Show when={killDetails() && killDetails()!.length > 0}>
        <div class="mx-6 mb-3 p-3 rounded-lg bg-warning-500/10 border border-warning-500/20 animate-slide-up">
          <div class="text-xs font-semibold text-warning-700 dark:text-warning-400 mb-1.5">
            以下进程未能终止：
          </div>
          <ul class="text-[11px] space-y-1 text-zinc-600 dark:text-zinc-300">
            <For each={killDetails()}>
              {(d) => (
                <li class="flex gap-2">
                  <span class="font-medium min-w-[120px]">
                    {d.name} <span class="text-zinc-400">(PID {d.pid})</span>
                  </span>
                  <span class="text-zinc-500">{d.message}</span>
                </li>
              )}
            </For>
          </ul>
        </div>
      </Show>

      {/* 底部操作条 */}
      <div class="px-6 py-3 border-t border-black/5 dark:border-white/5 flex items-center gap-3">
        <button
          type="button"
          class="btn-primary"
          disabled={selected().size === 0 || busy()}
          onClick={killSelected}
        >
          <Show
            when={!busy()}
            fallback={<Loader2 size={14} class="animate-spin" />}
          >
            终止已选 ({selected().size})
          </Show>
        </button>
        <span class="text-xs text-zinc-500">
          共 {selectableCount()} 项可终止 · 受保护进程已禁用选择
        </span>
        <Show when={message() && !killDetails()}>
          <span class="ml-auto text-xs text-zinc-500">{message()}</span>
        </Show>
      </div>
    </div>
  );
};

export default ProcessView;
