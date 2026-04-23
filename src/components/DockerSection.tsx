import {
  Component,
  createMemo,
  createResource,
  createSignal,
  For,
  Show,
} from "solid-js";
import {
  dockerInventory,
  dockerRemoveContainer,
  dockerRemoveImage,
  dockerRemoveVolume,
  dockerPruneAll,
  type DockerContainer,
  type DockerImage,
  type DockerVolume,
} from "@/lib/tauri";
import { fmtBytes } from "@/lib/format";
import { useI18n } from "@/i18n";
import {
  Container,
  Database,
  HardDrive,
  Layers,
  Loader2,
  RefreshCw,
  Sparkles,
  Trash2,
} from "lucide-solid";

type Tab = "images" | "containers" | "volumes";

const DockerSection: Component = () => {
  const { t } = useI18n();
  const [inv, { refetch }] = createResource(dockerInventory);
  const [tab, setTab] = createSignal<Tab>("images");
  const [busy, setBusy] = createSignal(false);
  const [message, setMessage] = createSignal<string | null>(null);

  const dangling = createMemo(() => inv()?.images.filter((i) => i.dangling) ?? []);
  const unusedVols = createMemo(() => inv()?.volumes.filter((v) => !v.in_use) ?? []);
  const stoppedContainers = createMemo(() => inv()?.containers.filter((c) => !c.running) ?? []);

  const removeImg = async (img: DockerImage) => {
    setBusy(true);
    setMessage(null);
    try {
      await dockerRemoveImage(img.id);
      setMessage(t("docker.deletedImage", { name: `${img.repository}:${img.tag}` }));
      await refetch();
    } catch (e) {
      setMessage(t("docker.deleteFailed", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const removeContainer = async (c: DockerContainer) => {
    setBusy(true);
    setMessage(null);
    try {
      await dockerRemoveContainer(c.id);
      setMessage(t("docker.deletedContainer", { name: c.name }));
      await refetch();
    } catch (e) {
      setMessage(t("docker.deleteFailed", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const removeVol = async (v: DockerVolume) => {
    setBusy(true);
    setMessage(null);
    try {
      await dockerRemoveVolume(v.name);
      setMessage(t("docker.deletedVolume", { name: v.name }));
      await refetch();
    } catch (e) {
      setMessage(t("docker.deleteFailed", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const pruneAll = async () => {
    if (!confirm(t("docker.pruneConfirm"))) return;
    setBusy(true);
    setMessage(null);
    try {
      const out = await dockerPruneAll();
      setMessage(out.split("\n").slice(-3).join("\n"));
      await refetch();
    } catch (e) {
      setMessage(t("docker.pruneFailed", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="card p-0 overflow-hidden animate-fade-in">
      <div class="px-6 py-5 border-b border-black/5 dark:border-white/5">
        <Show
          when={!inv.loading}
          fallback={
            <div class="flex items-center gap-2 text-sm text-zinc-500">
              <Loader2 size={14} class="animate-spin" />
              {t("docker.loading")}
            </div>
          }
        >
          <Show
            when={inv()?.daemon_running}
            fallback={
              <div class="space-y-2">
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <h3 class="text-base font-semibold">{t("docker.title")}</h3>
                    <p class="text-xs text-zinc-500 mt-1">{t("docker.subtitleOff")}</p>
                  </div>
                  <button type="button" class="btn-ghost gap-1.5" onClick={() => refetch()} disabled={inv.loading}>
                    <RefreshCw size={12} />
                    {t("docker.refresh")}
                  </button>
                </div>
                <div class="text-sm text-zinc-500">{t("docker.notRunning")}</div>
              </div>
            }
          >
            <div class="space-y-4">
              <div class="flex items-start justify-between gap-4">
                <div>
                  <h3 class="text-base font-semibold">{t("docker.title")}</h3>
                  <p class="text-xs text-zinc-500 mt-1">{t("docker.subtitleOn")}</p>
                </div>
                <div class="flex items-center gap-2">
                  <button
                    type="button"
                    class="btn-primary gap-1.5"
                    disabled={busy() || (inv()?.reclaimable_bytes ?? 0) === 0}
                    onClick={pruneAll}
                  >
                    <Show when={!busy()} fallback={<Loader2 size={14} class="animate-spin" />}>
                      <Sparkles size={14} />
                    </Show>
                    {t("docker.pruneAll")}
                  </button>
                  <button type="button" class="btn-ghost gap-1.5" onClick={() => refetch()} disabled={inv.loading}>
                    <RefreshCw size={12} />
                  </button>
                </div>
              </div>
              <div class="grid grid-cols-2 lg:grid-cols-5 gap-3">
                <StatCard label={t("docker.reclaimable")} value={fmtBytes(inv()?.reclaimable_bytes ?? 0)} />
                <StatCard label={t("docker.images")} value={`${inv()?.images.length ?? 0}`} note={t("docker.dangling", { count: dangling().length })} />
                <StatCard label={t("docker.containers")} value={`${inv()?.containers.length ?? 0}`} note={t("docker.stopped", { count: stoppedContainers().length })} />
                <StatCard label={t("docker.volumes")} value={`${inv()?.volumes.length ?? 0}`} note={t("docker.unused", { count: unusedVols().length })} />
                <StatCard label={t("docker.buildCache")} value={fmtBytes(inv()?.builder.total_bytes ?? 0)} />
              </div>
            </div>
          </Show>
        </Show>
      </div>

      <Show when={inv()?.daemon_running}>
        <div class="px-6 pt-3 border-b border-black/5 dark:border-white/5">
          <div class="flex gap-1">
            <TabBtn active={tab() === "images"} onClick={() => setTab("images")} icon={Layers} label={t("docker.images")} count={inv()?.images.length ?? 0} />
            <TabBtn active={tab() === "containers"} onClick={() => setTab("containers")} icon={Container} label={t("docker.containers")} count={inv()?.containers.length ?? 0} />
            <TabBtn active={tab() === "volumes"} onClick={() => setTab("volumes")} icon={Database} label={t("docker.volumes")} count={inv()?.volumes.length ?? 0} />
          </div>
        </div>

        <div class="max-h-[440px] overflow-y-auto">
          <Show when={tab() === "images"}>
            <ul class="divide-y divide-black/5 dark:divide-white/5">
              <For each={inv()?.images ?? []}>
                {(img) => (
                  <li class="flex items-center gap-3 px-6 py-3 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] group">
                    <HardDrive size={14} class={img.dangling ? "text-warning-500" : "text-zinc-400"} />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-sm truncate">
                          {img.dangling ? `<${t("docker.danglingBadge")}>` : `${img.repository}:${img.tag}`}
                        </span>
                        <Show when={img.dangling}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">{t("docker.danglingBadge")}</span>
                        </Show>
                        <Show when={img.in_use}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-brand-500/15 text-brand-600">{t("docker.inUseBadge")}</span>
                        </Show>
                      </div>
                      <div class="text-[10px] text-zinc-500 font-mono truncate">{img.id} · {img.created}</div>
                    </div>
                    <div class="text-sm tabular-nums text-zinc-600 dark:text-zinc-400">{fmtBytes(img.size_bytes)}</div>
                    <button type="button" class="p-1.5 rounded-lg text-zinc-400 hover:text-danger-500 hover:bg-danger-500/10 opacity-0 group-hover:opacity-100 transition" disabled={busy()} onClick={() => removeImg(img)} title={t("docker.deleteImage")}>
                      <Trash2 size={13} />
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </Show>

          <Show when={tab() === "containers"}>
            <ul class="divide-y divide-black/5 dark:divide-white/5">
              <For each={inv()?.containers ?? []}>
                {(c) => (
                  <li class="flex items-center gap-3 px-6 py-3 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] group">
                    <Container size={14} class={c.running ? "text-success-500" : "text-zinc-400"} />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-sm truncate">{c.name}</span>
                        <Show when={c.running}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-success-500/15 text-success-600">{t("docker.runningBadge")}</span>
                        </Show>
                        <Show when={!c.running}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-zinc-500/15 text-zinc-500">{t("docker.stoppedBadge")}</span>
                        </Show>
                      </div>
                      <div class="text-[10px] text-zinc-500 truncate">{c.image} · {c.status}</div>
                    </div>
                    <div class="text-sm tabular-nums text-zinc-600 dark:text-zinc-400">{c.size_bytes > 0 ? fmtBytes(c.size_bytes) : "—"}</div>
                    <button type="button" class="p-1.5 rounded-lg text-zinc-400 hover:text-danger-500 hover:bg-danger-500/10 opacity-0 group-hover:opacity-100 transition" disabled={busy()} onClick={() => removeContainer(c)} title={c.running ? t("docker.deleteContainerForce") : t("docker.deleteContainer")}>
                      <Trash2 size={13} />
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </Show>

          <Show when={tab() === "volumes"}>
            <ul class="divide-y divide-black/5 dark:divide-white/5">
              <For each={inv()?.volumes ?? []}>
                {(v) => (
                  <li class="flex items-center gap-3 px-6 py-3 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] group">
                    <Database size={14} class={v.in_use ? "text-brand-500" : "text-zinc-400"} />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-sm font-mono truncate">{v.name}</span>
                        <Show when={v.in_use}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-brand-500/15 text-brand-600">{t("docker.inUseBadge")}</span>
                        </Show>
                      </div>
                      <div class="text-[10px] text-zinc-500">driver: {v.driver}</div>
                    </div>
                    <button type="button" class="p-1.5 rounded-lg text-zinc-400 hover:text-danger-500 hover:bg-danger-500/10 opacity-0 group-hover:opacity-100 transition" disabled={busy() || v.in_use} onClick={() => removeVol(v)} title={v.in_use ? t("docker.deleteVolumeDisabled") : t("docker.deleteVolume")}>
                      <Trash2 size={13} />
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </div>
      </Show>

      <Show when={message()}>
        <div class="px-6 py-2 border-t border-black/5 dark:border-white/5 text-xs text-zinc-600 dark:text-zinc-400 whitespace-pre-wrap">
          {message()}
        </div>
      </Show>
    </div>
  );
};

const TabBtn: Component<{
  active: boolean;
  onClick: () => void;
  icon: Component<{ size?: number; class?: string }>;
  label: string;
  count: number;
}> = (p) => (
  <button
    type="button"
    onClick={p.onClick}
    class={`inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-t-lg border-b-2 -mb-[2px] transition-colors ${
      p.active
        ? "border-brand-500 text-zinc-900 dark:text-zinc-100"
        : "border-transparent text-zinc-500 hover:text-zinc-700"
    }`}
  >
    <p.icon size={14} />
    {p.label}
    <span class="text-xs text-zinc-400">({p.count})</span>
  </button>
);

const StatCard: Component<{ label: string; value: string; note?: string }> = (props) => (
  <div class="rounded-2xl bg-black/[0.03] dark:bg-white/[0.03] px-4 py-3">
    <div class="text-[11px] text-zinc-500">{props.label}</div>
    <div class="mt-1 text-lg font-semibold tabular-nums">{props.value}</div>
    <Show when={props.note}>
      <div class="mt-1 text-[10px] text-zinc-400">{props.note}</div>
    </Show>
  </div>
);

export default DockerSection;
