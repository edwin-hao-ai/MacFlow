import { Component, Show } from "solid-js";
import RingProgress from "./RingProgress";
import type { SystemHealth } from "@/lib/tauri";
import { useI18n } from "@/i18n";

type Props = { health: SystemHealth | null };

const HealthCard: Component<Props> = (props) => {
  const { t } = useI18n();
  return (
    <div class="card p-6 animate-fade-in">
      <div class="flex items-center justify-between mb-5">
        <div>
          <h2 class="text-base font-semibold">{t("health.title")}</h2>
          <p class="text-xs text-zinc-500 mt-0.5">{t("health.subtitle")}</p>
        </div>
        <Show
          when={props.health}
          fallback={
            <span class="text-xs text-zinc-400">{t("health.reading")}</span>
          }
        >
          <span class="inline-flex items-center gap-1.5 text-xs text-zinc-500">
            <span class="w-1.5 h-1.5 rounded-full bg-success-500 animate-pulse" />
            {t("health.normal")}
          </span>
        </Show>
      </div>

      <div class="grid grid-cols-3 gap-4">
        <Metric
          label={t("health.cpu")}
          value={props.health?.cpu_percent ?? 0}
          sub={props.health ? `${props.health.cpu_percent.toFixed(1)}%` : "—"}
        />
        <Metric
          label={t("health.memory")}
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
          label={t("health.disk")}
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
