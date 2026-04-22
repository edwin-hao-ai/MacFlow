import { Component, For, Show } from "solid-js";
import type { ProcessInfo } from "@/lib/tauri";
import { ShieldCheck } from "lucide-solid";
import { useI18n } from "@/i18n";

type Props = {
  processes: ProcessInfo[];
  selected: Set<number>;
  onToggle: (pid: number) => void;
  onWhitelist?: (name: string) => void;
};

const ProcessList: Component<Props> = (props) => {
  const { t } = useI18n();

  const riskBadge = (risk: ProcessInfo["risk"]) => {
    switch (risk) {
      case "safe":
        return (
          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-success-500/15 text-success-600">
            {t("risk.safe")}
          </span>
        );
      case "low":
        return (
          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">
            {t("risk.low")}
          </span>
        );
      case "dev":
        return (
          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-brand-500/15 text-brand-600">
            {t("risk.dev")}
          </span>
        );
      default:
        return null;
    }
  };

  return (
    <div class="card p-4 animate-fade-in">
      <div class="flex items-center justify-between mb-3 px-1">
        <h3 class="text-sm font-semibold">{t("scan.processListTitle")}</h3>
        <span class="text-xs text-zinc-500">
          {t("scan.itemsCount", { count: props.processes.length })}
        </span>
      </div>

      <Show
        when={props.processes.length > 0}
        fallback={
          <div class="text-center py-12 text-sm text-zinc-500">
            {t("scan.noProcesses")}
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
                  <div class="flex items-center gap-2 flex-wrap">
                    <span class="truncate font-medium text-sm">{p.name}</span>
                    {riskBadge(p.risk)}
                    <span class="text-[10px] text-zinc-400">
                      {t(`kind.${p.kind}`)}
                    </span>
                    <Show when={p.ports.length > 0}>
                      <span class="px-1.5 py-0.5 rounded-md text-[10px] font-mono font-medium bg-brand-500/10 text-brand-700 dark:text-brand-300">
                        :{p.ports.slice(0, 3).join(",")}
                        {p.ports.length > 3 && `+${p.ports.length - 3}`}
                      </span>
                    </Show>
                  </div>
                  <div class="text-[11px] text-zinc-500 font-mono truncate">
                    PID {p.pid} · {p.reason}
                  </div>
                </div>
                <div class="text-right tabular-nums text-xs text-zinc-500 min-w-[80px]">
                  <div>{p.cpu_percent.toFixed(1)}% CPU</div>
                  <div>{Math.round(p.memory_mb)}MB</div>
                </div>
                <Show when={props.onWhitelist}>
                  <button
                    type="button"
                    title={t("scan.whitelistTooltip")}
                    onClick={() => props.onWhitelist?.(p.name)}
                    class="p-1.5 rounded-lg text-zinc-400 hover:text-brand-600 hover:bg-brand-500/10 transition-colors"
                  >
                    <ShieldCheck size={14} />
                  </button>
                </Show>
              </li>
            )}
          </For>
        </ul>
      </Show>
    </div>
  );
};

export default ProcessList;
