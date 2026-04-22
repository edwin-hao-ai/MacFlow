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
  type DockerImage,
  type DockerContainer,
  type DockerVolume,
} from "@/lib/tauri";
import { fmtBytes } from "@/lib/format";
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

const DockerView: Component = () => {
  const [inv, { refetch }] = createResource(dockerInventory);
  const [tab, setTab] = createSignal<Tab>("images");
  const [busy, setBusy] = createSignal(false);
  const [message, setMessage] = createSignal<string | null>(null);

  const dangling = createMemo(
    () => inv()?.images.filter((i) => i.dangling) ?? [],
  );
  const unusedVols = createMemo(
    () => inv()?.volumes.filter((v) => !v.in_use) ?? [],
  );
  const stoppedContainers = createMemo(
    () => inv()?.containers.filter((c) => !c.running) ?? [],
  );

  const removeImg = async (img: DockerImage) => {
    setBusy(true);
    setMessage(null);
    try {
      await dockerRemoveImage(img.id);
      setMessage(`已删除镜像 ${img.repository}:${img.tag}`);
      await refetch();
    } catch (e) {
      setMessage(`删除失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const removeContainer = async (c: DockerContainer) => {
    setBusy(true);
    setMessage(null);
    try {
      await dockerRemoveContainer(c.id);
      setMessage(`已删除容器 ${c.name}`);
      await refetch();
    } catch (e) {
      setMessage(`删除失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const removeVol = async (v: DockerVolume) => {
    setBusy(true);
    setMessage(null);
    try {
      await dockerRemoveVolume(v.name);
      setMessage(`已删除卷 ${v.name}`);
      await refetch();
    } catch (e) {
      setMessage(`删除失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const pruneAll = async () => {
    if (
      !confirm(
        "将执行 `docker system prune -f --volumes`：\n清理悬空镜像 + 已停止容器 + 构建缓存 + 未引用卷。\n正在运行的不动。继续？",
      )
    )
      return;
    setBusy(true);
    setMessage(null);
    try {
      const out = await dockerPruneAll();
      setMessage(out.split("\n").slice(-3).join("\n"));
      await refetch();
    } catch (e) {
      setMessage(`prune 失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="flex flex-col h-full">
      {/* Top summary */}
      <div class="px-6 py-4 border-b border-black/5 dark:border-white/5">
        <Show
          when={inv.loading}
          fallback={
            <Show
              when={inv()?.daemon_running}
              fallback={
                <div class="text-sm text-zinc-500">
                  Docker 未运行或未安装。启动 Docker Desktop 后刷新。
                </div>
              }
            >
              <div class="flex items-center gap-6">
                <div>
                  <div class="text-xs text-zinc-500">可回收</div>
                  <div class="text-2xl font-bold text-brand-600 tabular-nums">
                    {fmtBytes(inv()?.reclaimable_bytes ?? 0)}
                  </div>
                </div>
                <div>
                  <div class="text-xs text-zinc-500">镜像</div>
                  <div class="text-lg font-semibold tabular-nums">
                    {inv()?.images.length}
                    <span class="text-xs text-zinc-400 ml-1">
                      （{dangling().length} 悬空）
                    </span>
                  </div>
                </div>
                <div>
                  <div class="text-xs text-zinc-500">容器</div>
                  <div class="text-lg font-semibold tabular-nums">
                    {inv()?.containers.length}
                    <span class="text-xs text-zinc-400 ml-1">
                      （{stoppedContainers().length} 停止）
                    </span>
                  </div>
                </div>
                <div>
                  <div class="text-xs text-zinc-500">卷</div>
                  <div class="text-lg font-semibold tabular-nums">
                    {inv()?.volumes.length}
                    <span class="text-xs text-zinc-400 ml-1">
                      （{unusedVols().length} 未引用）
                    </span>
                  </div>
                </div>
                <div>
                  <div class="text-xs text-zinc-500">构建缓存</div>
                  <div class="text-lg font-semibold tabular-nums">
                    {fmtBytes(inv()?.builder.total_bytes ?? 0)}
                  </div>
                </div>

                <button
                  type="button"
                  class="ml-auto btn-primary gap-1.5"
                  disabled={busy() || (inv()?.reclaimable_bytes ?? 0) === 0}
                  onClick={pruneAll}
                >
                  <Show
                    when={!busy()}
                    fallback={<Loader2 size={14} class="animate-spin" />}
                  >
                    <Sparkles size={14} />
                  </Show>
                  一键回收所有
                </button>
                <button
                  type="button"
                  class="btn-ghost gap-1.5"
                  onClick={() => refetch()}
                  disabled={inv.loading}
                >
                  <RefreshCw size={12} />
                </button>
              </div>
            </Show>
          }
        >
          <div class="flex items-center gap-2 text-sm text-zinc-500">
            <Loader2 size={14} class="animate-spin" />
            读取 Docker 状态...
          </div>
        </Show>
      </div>

      <Show when={inv()?.daemon_running}>
        {/* Tabs */}
        <div class="px-6 pt-3 border-b border-black/5 dark:border-white/5">
          <div class="flex gap-1">
            <TabBtn
              active={tab() === "images"}
              onClick={() => setTab("images")}
              icon={Layers}
              label="镜像"
              count={inv()?.images.length ?? 0}
            />
            <TabBtn
              active={tab() === "containers"}
              onClick={() => setTab("containers")}
              icon={Container}
              label="容器"
              count={inv()?.containers.length ?? 0}
            />
            <TabBtn
              active={tab() === "volumes"}
              onClick={() => setTab("volumes")}
              icon={Database}
              label="卷"
              count={inv()?.volumes.length ?? 0}
            />
          </div>
        </div>

        {/* Body */}
        <div class="flex-1 overflow-y-auto">
          <Show when={tab() === "images"}>
            <ul class="divide-y divide-black/5 dark:divide-white/5">
              <For each={inv()?.images ?? []}>
                {(img) => (
                  <li class="flex items-center gap-3 px-6 py-3 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] group">
                    <HardDrive
                      size={14}
                      class={img.dangling ? "text-warning-500" : "text-zinc-400"}
                    />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-sm truncate">
                          {img.dangling
                            ? "<悬空镜像>"
                            : `${img.repository}:${img.tag}`}
                        </span>
                        <Show when={img.dangling}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">
                            悬空
                          </span>
                        </Show>
                        <Show when={img.in_use}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-brand-500/15 text-brand-600">
                            使用中
                          </span>
                        </Show>
                      </div>
                      <div class="text-[10px] text-zinc-500 font-mono truncate">
                        {img.id} · {img.created}
                      </div>
                    </div>
                    <div class="text-sm tabular-nums text-zinc-600 dark:text-zinc-400">
                      {fmtBytes(img.size_bytes)}
                    </div>
                    <button
                      type="button"
                      class="p-1.5 rounded-lg text-zinc-400 hover:text-danger-500 hover:bg-danger-500/10 opacity-0 group-hover:opacity-100 transition"
                      disabled={busy()}
                      onClick={() => removeImg(img)}
                      title="删除镜像"
                    >
                      <Trash2 size={13} />
                    </button>
                  </li>
                )}
              </For>
              <Show when={(inv()?.images.length ?? 0) === 0}>
                <li class="py-10 text-center text-sm text-zinc-500">
                  没有镜像
                </li>
              </Show>
            </ul>
          </Show>

          <Show when={tab() === "containers"}>
            <ul class="divide-y divide-black/5 dark:divide-white/5">
              <For each={inv()?.containers ?? []}>
                {(c) => (
                  <li class="flex items-center gap-3 px-6 py-3 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] group">
                    <Container
                      size={14}
                      class={c.running ? "text-success-500" : "text-zinc-400"}
                    />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-sm truncate">
                          {c.name}
                        </span>
                        <Show when={c.running}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-success-500/15 text-success-600">
                            运行中
                          </span>
                        </Show>
                        <Show when={!c.running}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-zinc-500/15 text-zinc-500">
                            已停止
                          </span>
                        </Show>
                      </div>
                      <div class="text-[10px] text-zinc-500 truncate">
                        {c.image} · {c.status}
                      </div>
                    </div>
                    <div class="text-sm tabular-nums text-zinc-600 dark:text-zinc-400">
                      {c.size_bytes > 0 ? fmtBytes(c.size_bytes) : "—"}
                    </div>
                    <button
                      type="button"
                      class="p-1.5 rounded-lg text-zinc-400 hover:text-danger-500 hover:bg-danger-500/10 opacity-0 group-hover:opacity-100 transition"
                      disabled={busy()}
                      onClick={() => removeContainer(c)}
                      title={c.running ? "强制删除容器" : "删除容器"}
                    >
                      <Trash2 size={13} />
                    </button>
                  </li>
                )}
              </For>
              <Show when={(inv()?.containers.length ?? 0) === 0}>
                <li class="py-10 text-center text-sm text-zinc-500">
                  没有容器
                </li>
              </Show>
            </ul>
          </Show>

          <Show when={tab() === "volumes"}>
            <ul class="divide-y divide-black/5 dark:divide-white/5">
              <For each={inv()?.volumes ?? []}>
                {(v) => (
                  <li class="flex items-center gap-3 px-6 py-3 hover:bg-black/[0.02] dark:hover:bg-white/[0.02] group">
                    <Database
                      size={14}
                      class={v.in_use ? "text-brand-500" : "text-zinc-400"}
                    />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-medium text-sm font-mono truncate">
                          {v.name}
                        </span>
                        <Show when={v.in_use}>
                          <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-brand-500/15 text-brand-600">
                            使用中
                          </span>
                        </Show>
                      </div>
                      <div class="text-[10px] text-zinc-500">
                        driver: {v.driver}
                      </div>
                    </div>
                    <button
                      type="button"
                      class="p-1.5 rounded-lg text-zinc-400 hover:text-danger-500 hover:bg-danger-500/10 opacity-0 group-hover:opacity-100 transition"
                      disabled={busy() || v.in_use}
                      onClick={() => removeVol(v)}
                      title={v.in_use ? "使用中，无法删除" : "删除卷"}
                    >
                      <Trash2 size={13} />
                    </button>
                  </li>
                )}
              </For>
              <Show when={(inv()?.volumes.length ?? 0) === 0}>
                <li class="py-10 text-center text-sm text-zinc-500">
                  没有卷
                </li>
              </Show>
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

export default DockerView;
