import { Component, For, Show } from "solid-js";
import type { ProcessInfo } from "@/lib/tauri";

type Props = {
  processes: ProcessInfo[];
  selected: Set<number>;
  onToggle: (pid: number) => void;
};

const kindLabel: Record<string, string> = {
  residual: "软件残留",
  duplicate: "重复进程",
  idle: "长期闲置",
  hog: "高占用闲置",
  dev: "开发残留",
  system: "系统进程",
  foreground: "前台活跃",
};

const riskBadge = (risk: ProcessInfo["risk"]) => {
  switch (risk) {
    case "safe":
      return <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-success-500/15 text-success-600">安全</span>;
    case "low":
      return <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">低风险</span>;
    case "dev":
      return <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-brand-500/15 text-brand-600">开发</span>;
    default:
      return null;
  }
};

const ProcessList: Component<Props> = (props) => {
  return (
    <div class="card p-4 animate-fade-in">
      <div class="flex items-center justify-between mb-3 px-1">
        <h3 class="text-sm font-semibold">可优化进程</h3>
        <span class="text-xs text-zinc-500">
          {props.processes.length} 项
        </span>
      </div>

      <Show
        when={props.processes.length > 0}
        fallback={
          <div class="text-center py-12 text-sm text-zinc-500">
            没有发现可优化的进程，系统运行良好
          </div>
        }
      >
        <ul class="divide-y divide-black/5 dark:divide-white/5 max-h-[320px] overflow-y-auto">
          <For each={props.processes}>
            {(p) => (
              <li class="flex items-center gap-3 py-2 px-1 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] rounded-lg transition-colors">
                <input
                  type="checkbox"
                  checked={props.selected.has(p.pid)}
                  onChange={() => props.onToggle(p.pid)}
                  class="w-4 h-4 rounded accent-brand-500"
                />
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <span class="truncate font-medium text-sm">{p.name}</span>
                    {riskBadge(p.risk)}
                    <span class="text-[10px] text-zinc-400">
                      {kindLabel[p.kind] ?? p.kind}
                    </span>
                  </div>
                  <div class="text-[11px] text-zinc-500 font-mono truncate">
                    PID {p.pid} · {p.reason}
                  </div>
                </div>
                <div class="text-right tabular-nums text-xs text-zinc-500 min-w-[80px]">
                  <div>{p.cpu_percent.toFixed(1)}% CPU</div>
                  <div>{Math.round(p.memory_mb)}MB</div>
                </div>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
};

export default ProcessList;
