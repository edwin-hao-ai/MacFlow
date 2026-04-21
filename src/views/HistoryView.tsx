import { Component, createSignal, For, onMount, Show } from "solid-js";
import { getHistory, type HistoryEntry } from "@/lib/tauri";
import { fmtBytes, fmtRelativeTime } from "@/lib/format";
import { CheckCircle2, XCircle, Cpu, HardDrive, Loader2 } from "lucide-solid";

const opLabels: Record<string, string> = {
  process_kill: "进程清理",
  cache_clean: "缓存清理",
};

const HistoryView: Component = () => {
  const [entries, setEntries] = createSignal<HistoryEntry[]>([]);
  const [loading, setLoading] = createSignal(false);

  const load = async () => {
    setLoading(true);
    try {
      const h = await getHistory(300);
      setEntries(h);
    } finally {
      setLoading(false);
    }
  };

  onMount(load);

  return (
    <div class="flex flex-col gap-5 p-6 h-full overflow-y-auto">
      <div class="card p-6">
        <h2 class="text-base font-semibold">操作历史</h2>
        <p class="text-xs text-zinc-500 mt-0.5">
          所有进程和缓存清理操作的本地日志。只记录元数据，不上传任何内容。
        </p>
      </div>

      <Show
        when={!loading()}
        fallback={
          <div class="text-center py-12 text-sm text-zinc-500 flex items-center justify-center gap-2">
            <Loader2 size={14} class="animate-spin" />
            加载中...
          </div>
        }
      >
        <Show
          when={entries().length > 0}
          fallback={
            <div class="card p-12 text-center text-sm text-zinc-500">
              还没有任何操作记录
            </div>
          }
        >
          <div class="card p-0 overflow-hidden">
            <ul class="divide-y divide-black/5 dark:divide-white/5">
              <For each={entries()}>
                {(e) => (
                  <li class="flex items-start gap-3 p-4 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] transition-colors">
                    <div class="w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0 mt-0.5">
                      <Show
                        when={e.success}
                        fallback={
                          <div class="w-9 h-9 rounded-lg bg-danger-500/10 flex items-center justify-center">
                            <XCircle size={16} class="text-danger-500" />
                          </div>
                        }
                      >
                        <div
                          class={`w-9 h-9 rounded-lg flex items-center justify-center ${
                            e.operation === "process_kill"
                              ? "bg-brand-500/10"
                              : "bg-success-500/10"
                          }`}
                        >
                          {e.operation === "process_kill" ? (
                            <Cpu size={16} class="text-brand-600" />
                          ) : (
                            <HardDrive size={16} class="text-success-600" />
                          )}
                        </div>
                      </Show>
                    </div>
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-sm truncate">
                          {e.target}
                        </span>
                        <Show when={e.success}>
                          <CheckCircle2
                            size={12}
                            class="text-success-500 flex-shrink-0"
                          />
                        </Show>
                      </div>
                      <div class="text-xs text-zinc-500 mt-0.5">
                        {opLabels[e.operation] ?? e.operation} · {e.detail}
                      </div>
                    </div>
                    <div class="text-right text-xs text-zinc-500 tabular-nums flex-shrink-0">
                      <Show when={e.freed_bytes > 0}>
                        <div class="font-medium text-success-600">
                          +{fmtBytes(e.freed_bytes)}
                        </div>
                      </Show>
                      <div>{fmtRelativeTime(e.timestamp)}</div>
                    </div>
                  </li>
                )}
              </For>
            </ul>
          </div>
        </Show>
      </Show>
    </div>
  );
};

export default HistoryView;
