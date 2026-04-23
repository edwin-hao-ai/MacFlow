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
  AlertTriangle,
  X,
  ArrowDown,
  ArrowUp,
  ChevronRight,
  ChevronDown,
  ListTree,
  List,
} from "lucide-solid";
import { useI18n } from "@/i18n";

type SortKey = "memory" | "cpu" | "name" | "pid" | "uptime";
type ViewMode = "tree" | "flat";

type TreeNode = {
  row: ProcessRow;
  children: TreeNode[];
  depth: number;
};

function fmtUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h`;
  return `${Math.floor(secs / 86400)}d`;
}

/** 把扁平 rows 按 parent_pid 组装成树 */
function buildTree(rows: ProcessRow[]): TreeNode[] {
  const byPid = new Map<number, TreeNode>();
  const all = rows.map((r) => ({ row: r, children: [], depth: 0 }) as TreeNode);
  for (const n of all) byPid.set(n.row.pid, n);

  const roots: TreeNode[] = [];
  for (const n of all) {
    const ppid = n.row.parent_pid;
    if (ppid !== null && byPid.has(ppid)) {
      const parent = byPid.get(ppid)!;
      parent.children.push(n);
      n.depth = 0; // 会在 flatten 时设置
    } else {
      roots.push(n);
    }
  }

  // 按内存降序排子节点（递归）
  const sortChildren = (node: TreeNode) => {
    node.children.sort((a, b) => b.row.memory_mb - a.row.memory_mb);
    node.children.forEach(sortChildren);
  };
  roots.forEach(sortChildren);
  roots.sort((a, b) => b.row.memory_mb - a.row.memory_mb);

  return roots;
}

/** 把树扁平化为按深度缩进的显示顺序，respecting collapsed */
function flattenTree(
  roots: TreeNode[],
  collapsed: Set<number>,
  sortKey: SortKey,
  sortDir: "desc" | "asc",
): TreeNode[] {
  const dir = sortDir === "desc" ? -1 : 1;
  const cmp = (a: TreeNode, b: TreeNode) => {
    switch (sortKey) {
      case "memory":
        return dir * (a.row.memory_mb - b.row.memory_mb);
      case "cpu":
        return dir * (a.row.cpu_percent - b.row.cpu_percent);
      case "name":
        return dir * a.row.name.localeCompare(b.row.name);
      case "pid":
        return dir * (a.row.pid - b.row.pid);
      case "uptime":
        return dir * (a.row.uptime_secs - b.row.uptime_secs);
    }
  };
  const sortedRoots = [...roots].sort(cmp);
  const out: TreeNode[] = [];
  const visit = (node: TreeNode, depth: number) => {
    node.depth = depth;
    out.push(node);
    if (collapsed.has(node.row.pid)) return;
    const kids = [...node.children].sort(cmp);
    for (const c of kids) visit(c, depth + 1);
  };
  for (const r of sortedRoots) visit(r, 0);
  return out;
}

const ProcessView: Component = () => {
  const { t } = useI18n();
  const [rows, setRows] = createSignal<ProcessRow[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const [portsOnly, setPortsOnly] = createSignal(false);
  const [viewMode, setViewMode] = createSignal<ViewMode>("tree");
  const [collapsed, setCollapsed] = createSignal(new Set<number>());
  const [sortKey, setSortKey] = createSignal<SortKey>("memory");
  const [sortDir, setSortDir] = createSignal<"desc" | "asc">("desc");
  const [selected, setSelected] = createSignal(new Set<number>());
  const [busy, setBusy] = createSignal(false);
  const [message, setMessage] = createSignal<string | null>(null);
  const [killDetails, setKillDetails] = createSignal<KillResult[] | null>(null);
  const [confirmProtectedKill, setConfirmProtectedKill] =
    createSignal<null | { pids: number[]; names: string[]; protected: ProcessRow[] }>(null);

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
    timer = window.setInterval(load, 5000);
  });
  onCleanup(() => {
    if (timer) clearInterval(timer);
  });

  const filteredRows = createMemo(() => {
    const q = query().trim().toLowerCase();
    let list = rows();
    if (portsOnly()) list = list.filter((r) => r.ports.length > 0);
    if (q) {
      list = list.filter(
        (r) =>
          r.name.toLowerCase().includes(q) ||
          String(r.pid).includes(q) ||
          r.exe.toLowerCase().includes(q) ||
          r.ports.some((p) => String(p).includes(q)),
      );
    }
    return list;
  });

  /** 可见节点（flat 或 tree 模式） */
  const visibleNodes = createMemo((): TreeNode[] => {
    const list = filteredRows();
    if (viewMode() === "flat") {
      const dir = sortDir() === "desc" ? -1 : 1;
      const sorted = [...list].sort((a, b) => {
        switch (sortKey()) {
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
      return sorted.map((r) => ({ row: r, children: [], depth: 0 }));
    }
    const tree = buildTree(list);
    return flattenTree(tree, collapsed(), sortKey(), sortDir());
  });

  const hasChildren = (pid: number): boolean => {
    // 在原始数据中查：pid 有没有被任何行引用为 parent
    return rows().some((r) => r.parent_pid === pid);
  };

  const toggleCollapse = (pid: number) => {
    const next = new Set(collapsed());
    next.has(pid) ? next.delete(pid) : next.add(pid);
    setCollapsed(next);
  };

  const toggleSelect = (pid: number) => {
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
    const procs = rows();
    const names = pids.map(
      (pid) => procs.find((r) => r.pid === pid)?.name ?? String(pid),
    );
    const protectedRows = procs.filter(
      (r) => pids.includes(r.pid) && r.protected,
    );

    // 如果有受保护项，先弹确认
    if (protectedRows.length > 0) {
      setConfirmProtectedKill({ pids, names, protected: protectedRows });
      return;
    }
    await doKill(pids, names);
  };

  const doKill = async (pids: number[], names: string[]) => {
    setBusy(true);
    setMessage(null);
    setKillDetails(null);
    try {
      const r = await killProcesses(pids, names);
      let msg = t("scan.killSuccess", { count: r.killed.length });
      if (r.failed.length > 0) {
        msg += t("scan.killPartial", { failed: r.failed.length });
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

  const totalMem = createMemo(() => rows().reduce((s, r) => s + r.memory_mb, 0));
  const totalCPU = createMemo(() => rows().reduce((s, r) => s + r.cpu_percent, 0));

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
      {/* 顶栏 */}
      <div class="px-6 py-4 border-b border-black/5 dark:border-white/5 flex items-center gap-3 flex-wrap">
        <div class="flex gap-6">
          <div>
            <div class="text-xs text-zinc-500">共</div>
            <div class="text-lg font-semibold tabular-nums">{rows().length}</div>
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

        <div class="flex-1" />

        {/* 视图模式 */}
        <div class="inline-flex bg-black/5 dark:bg-white/5 rounded-lg p-1 gap-0.5">
          <button
            type="button"
            onClick={() => setViewMode("tree")}
            class={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-colors ${
              viewMode() === "tree"
                ? "bg-white dark:bg-zinc-700 shadow-sm"
                : "text-zinc-500 hover:text-zinc-700"
            }`}
            title="树形视图：按父子关系分组"
          >
            <ListTree size={12} />
            树形
          </button>
          <button
            type="button"
            onClick={() => setViewMode("flat")}
            class={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-colors ${
              viewMode() === "flat"
                ? "bg-white dark:bg-zinc-700 shadow-sm"
                : "text-zinc-500 hover:text-zinc-700"
            }`}
            title="扁平视图：按 CPU / 内存排序"
          >
            <List size={12} />
            扁平
          </button>
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
          <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" />
          <input
            type="text"
            placeholder="搜索..."
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
          when={visibleNodes().length > 0}
          fallback={
            <div class="py-20 text-center text-sm text-zinc-500">
              <Show when={!loading()}>
                {query() ? `没有匹配 "${query()}" 的进程` : "没有可见进程"}
              </Show>
            </div>
          }
        >
          <For each={visibleNodes()}>
            {(node) => {
              const r = node.row;
              const isExpandable =
                viewMode() === "tree" && hasChildren(r.pid);
              const isCollapsed = collapsed().has(r.pid);
              return (
                <div
                  class="px-6 py-1.5 grid grid-cols-[1fr_72px_72px_64px_56px_96px] gap-2 items-center text-sm hover:bg-black/[0.02] dark:hover:bg-white/[0.02] cursor-default group"
                  classList={{ "opacity-60": r.protected }}
                >
                  <div class="min-w-0 flex items-center gap-2">
                    {/* 缩进 + 折叠箭头 */}
                    <div
                      class="flex-shrink-0 flex items-center"
                      style={{
                        "padding-left": `${node.depth * 16}px`,
                      }}
                    >
                      <Show
                        when={isExpandable}
                        fallback={<div class="w-4" />}
                      >
                        <button
                          type="button"
                          onClick={() => toggleCollapse(r.pid)}
                          class="p-0.5 rounded hover:bg-black/10 dark:hover:bg-white/10 text-zinc-500"
                          title={isCollapsed ? "展开" : "折叠"}
                        >
                          <Show when={isCollapsed} fallback={<ChevronDown size={12} />}>
                            <ChevronRight size={12} />
                          </Show>
                        </button>
                      </Show>
                    </div>

                    <input
                      type="checkbox"
                      checked={selected().has(r.pid)}
                      onChange={() => toggleSelect(r.pid)}
                      class="w-4 h-4 rounded accent-brand-500 flex-shrink-0"
                      title={
                        r.protected
                          ? "受保护：勾选后终止前会弹窗确认"
                          : undefined
                      }
                    />
                    <Show when={r.protected}>
                      <span title={r.protected_reason ?? "受保护进程"}>
                        <ShieldAlert
                          size={12}
                          class="text-warning-500 flex-shrink-0"
                        />
                      </span>
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
                        <div class="text-[10px] text-warning-600 dark:text-warning-400 truncate">
                          ⚠ {r.protected_reason}
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
                        "text-warning-600":
                          r.cpu_percent > 20 && r.cpu_percent <= 50,
                        "text-zinc-600 dark:text-zinc-400": r.cpu_percent <= 20,
                      }}
                    >
                      {r.cpu_percent.toFixed(1)}%
                    </span>
                  </div>
                  <div class="text-right tabular-nums text-xs text-zinc-600 dark:text-zinc-400">
                    {r.memory_mb < 1024
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
              );
            }}
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
          <Show when={!busy()} fallback={<Loader2 size={14} class="animate-spin" />}>
            终止已选 ({selected().size})
          </Show>
        </button>
        <span class="text-xs text-zinc-500">
          受保护项仍可终止，但会弹窗确认
        </span>
        <Show when={message() && !killDetails()}>
          <span class="ml-auto text-xs text-zinc-500">{message()}</span>
        </Show>
      </div>

      {/* 受保护 kill 确认弹窗 */}
      <Show when={confirmProtectedKill()}>
        {(d) => (
          <div
            class="fixed inset-0 bg-black/40 backdrop-blur-sm z-50 flex items-center justify-center p-6 animate-fade-in"
            onClick={() => setConfirmProtectedKill(null)}
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
                  <h3 class="font-semibold">确认终止受保护进程？</h3>
                  <p class="text-sm text-zinc-500 mt-1">
                    以下进程被标记为受保护，强制终止可能导致应用崩溃、数据丢失或系统异常：
                  </p>
                  <ul class="mt-3 space-y-1.5 max-h-[200px] overflow-y-auto">
                    <For each={d().protected}>
                      {(p) => (
                        <li class="text-xs flex items-start gap-2">
                          <ShieldAlert
                            size={11}
                            class="text-warning-500 flex-shrink-0 mt-0.5"
                          />
                          <div class="min-w-0">
                            <div class="font-medium truncate">
                              {p.name}
                              <span class="text-zinc-400 ml-1">(PID {p.pid})</span>
                            </div>
                            <div class="text-[10px] text-zinc-500 truncate">
                              {p.protected_reason}
                            </div>
                          </div>
                        </li>
                      )}
                    </For>
                  </ul>
                </div>
              </div>
              <div class="flex justify-end gap-2 mt-5">
                <button
                  type="button"
                  class="btn-ghost"
                  onClick={() => setConfirmProtectedKill(null)}
                >
                  取消
                </button>
                <button
                  type="button"
                  class="inline-flex items-center justify-center rounded-xl px-5 py-2.5 font-medium bg-danger-500 hover:bg-danger-400 text-white shadow-sm transition-all"
                  onClick={async () => {
                    const info = d();
                    setConfirmProtectedKill(null);
                    await doKill(info.pids, info.names);
                  }}
                >
                  仍要强制终止
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>
    </div>
  );
};

export default ProcessView;
