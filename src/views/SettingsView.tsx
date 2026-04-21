import { Component, createSignal, For, onMount, Show } from "solid-js";
import {
  addWhitelist,
  getWhitelist,
  removeWhitelist,
  type WhitelistEntry,
} from "@/lib/tauri";
import { fmtRelativeTime } from "@/lib/format";
import { Plus, Trash2 } from "lucide-solid";

const SettingsView: Component = () => {
  const [items, setItems] = createSignal<WhitelistEntry[]>([]);
  const [adding, setAdding] = createSignal(false);
  const [kind, setKind] = createSignal<"process" | "cache_path">("process");
  const [value, setValue] = createSignal("");
  const [note, setNote] = createSignal("");

  const load = async () => {
    setItems(await getWhitelist());
  };

  onMount(load);

  const submit = async () => {
    if (!value().trim()) return;
    await addWhitelist(kind(), value().trim(), note().trim());
    setValue("");
    setNote("");
    setAdding(false);
    await load();
  };

  const remove = async (id: number) => {
    await removeWhitelist(id);
    await load();
  };

  return (
    <div class="flex flex-col gap-5 p-6 h-full overflow-y-auto">
      <div class="card p-6">
        <h2 class="text-base font-semibold">白名单管理</h2>
        <p class="text-xs text-zinc-500 mt-0.5">
          加入白名单的项目永远不会被扫描或清理
        </p>
      </div>

      <div class="card p-4">
        <div class="flex items-center justify-between mb-3 px-1">
          <div class="text-sm font-medium">
            自定义白名单 ({items().length})
          </div>
          <button
            type="button"
            class="btn-ghost gap-1.5"
            onClick={() => setAdding(!adding())}
          >
            <Plus size={14} />
            添加
          </button>
        </div>

        <Show when={adding()}>
          <div class="p-3 mb-3 rounded-xl bg-black/5 dark:bg-white/5 space-y-2 animate-slide-up">
            <div class="flex gap-2">
              <select
                value={kind()}
                onChange={(e) =>
                  setKind(e.currentTarget.value as "process" | "cache_path")
                }
                class="rounded-lg px-2 py-1.5 text-sm bg-white dark:bg-zinc-800 border border-black/10 dark:border-white/10"
              >
                <option value="process">进程名</option>
                <option value="cache_path">缓存路径</option>
              </select>
              <input
                type="text"
                placeholder={
                  kind() === "process" ? "例如 Chrome" : "例如 ~/.npm"
                }
                value={value()}
                onInput={(e) => setValue(e.currentTarget.value)}
                class="flex-1 rounded-lg px-3 py-1.5 text-sm bg-white dark:bg-zinc-800 border border-black/10 dark:border-white/10"
              />
            </div>
            <input
              type="text"
              placeholder="备注（可选）"
              value={note()}
              onInput={(e) => setNote(e.currentTarget.value)}
              class="w-full rounded-lg px-3 py-1.5 text-sm bg-white dark:bg-zinc-800 border border-black/10 dark:border-white/10"
            />
            <div class="flex justify-end gap-2">
              <button
                type="button"
                class="btn-ghost"
                onClick={() => setAdding(false)}
              >
                取消
              </button>
              <button type="button" class="btn-primary" onClick={submit}>
                添加
              </button>
            </div>
          </div>
        </Show>

        <Show
          when={items().length > 0}
          fallback={
            <div class="text-center py-8 text-sm text-zinc-500">
              还没有自定义白名单
            </div>
          }
        >
          <ul class="divide-y divide-black/5 dark:divide-white/5">
            <For each={items()}>
              {(w) => (
                <li class="flex items-center gap-3 py-2 px-1">
                  <span
                    class={`px-2 py-0.5 rounded-md text-[10px] font-semibold ${
                      w.kind === "process"
                        ? "bg-brand-500/15 text-brand-600"
                        : "bg-warning-500/15 text-warning-600"
                    }`}
                  >
                    {w.kind === "process" ? "进程" : "路径"}
                  </span>
                  <div class="flex-1 min-w-0">
                    <div class="text-sm font-medium truncate">{w.value}</div>
                    <Show when={w.note}>
                      <div class="text-xs text-zinc-500 truncate">{w.note}</div>
                    </Show>
                  </div>
                  <div class="text-[10px] text-zinc-400">
                    {fmtRelativeTime(w.added_at)}
                  </div>
                  <button
                    type="button"
                    class="p-1.5 rounded-lg hover:bg-danger-500/10 text-zinc-400 hover:text-danger-500 transition-colors"
                    onClick={() => remove(w.id)}
                  >
                    <Trash2 size={14} />
                  </button>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </div>

      <div class="card p-6">
        <div class="text-sm font-medium mb-1">关于 MacFlow</div>
        <div class="text-xs text-zinc-500 space-y-1">
          <div>版本 0.1.0 · 规则驱动 · 本地存储</div>
          <div>不上传任何数据，不接任何 LLM，不做广告推送</div>
          <div class="font-mono text-[10px] mt-2">
            配置文件：~/Library/Application Support/MacFlow/macflow.db
          </div>
        </div>
      </div>
    </div>
  );
};

export default SettingsView;
