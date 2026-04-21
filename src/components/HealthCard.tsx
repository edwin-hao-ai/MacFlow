import { Component, Show } from "solid-js";
import RingProgress from "./RingProgress";
import type { SystemHealth } from "@/lib/tauri";

type Props = { health: SystemHealth | null };

const HealthCard: Component<Props> = (props) => {
  return (
    <div class="card p-6 animate-fade-in">
      <div class="flex items-center justify-between mb-5">
        <div>
          <h2 class="text-base font-semibold">系统健康</h2>
          <p class="text-xs text-zinc-500 mt-0.5">实时监控 CPU / 内存 / 磁盘</p>
        </div>
        <Show
          when={props.health}
          fallback={
            <span class="text-xs text-zinc-400">读取中...</span>
          }
        >
          <span class="inline-flex items-center gap-1.5 text-xs text-zinc-500">
            <span class="w-1.5 h-1.5 rounded-full bg-success-500 animate-pulse" />
            正常运行
          </span>
        </Show>
      </div>

      <div class="grid grid-cols-3 gap-4">
        <Metric
          label="CPU"
          value={props.health?.cpu_percent ?? 0}
          sub={props.health ? `${props.health.cpu_percent.toFixed(1)}%` : "—"}
        />
        <Metric
          label="内存"
          value={props.health?.memory_percent ?? 0}
          sub={
            props.health
              ? `${fmtMb(props.health.memory_used_mb)} / ${fmtMb(
                  props.health.memory_total_mb,
                )}`
              : "—"
          }
        />
        <Metric
          label="磁盘"
          value={props.health?.disk_percent ?? 0}
          sub={
            props.health
              ? `${props.health.disk_used_gb.toFixed(0)}GB / ${props.health.disk_total_gb.toFixed(0)}GB`
              : "—"
          }
        />
      </div>
    </div>
  );
};

const Metric: Component<{ label: string; value: number; sub: string }> = (
  props,
) => (
  <div class="flex flex-col items-center gap-2">
    <RingProgress value={props.value} />
    <div class="text-center">
      <div class="text-sm font-medium">{props.label}</div>
      <div class="text-xs text-zinc-500 tabular-nums">{props.sub}</div>
    </div>
  </div>
);

function fmtMb(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`;
  return `${Math.round(mb)}MB`;
}

export default HealthCard;
