import { Component, createSignal, For, onMount, Show } from "solid-js";
import {
  addWhitelist,
  getWhitelist,
  removeWhitelist,
  type WhitelistEntry,
} from "@/lib/tauri";
import { fmtRelativeTime } from "@/lib/format";
import { Plus, Trash2 } from "lucide-solid";
import {
  disable as autostartDisable,
  enable as autostartEnable,
  isEnabled as autostartIsEnabled,
} from "@tauri-apps/plugin-autostart";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";

const PREFS_KEY = "macflow.prefs.v1";
type Prefs = {
  notifyOnCleanComplete: boolean;
};
const defaultPrefs: Prefs = {
  notifyOnCleanComplete: true,
};

function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (!raw) return defaultPrefs;
    return { ...defaultPrefs, ...JSON.parse(raw) };
  } catch {
    return defaultPrefs;
  }
}

function savePrefs(p: Prefs) {
  localStorage.setItem(PREFS_KEY, JSON.stringify(p));
}

const SettingsView: Component = () => {
  const [items, setItems] = createSignal<WhitelistEntry[]>([]);
  const [adding, setAdding] = createSignal(false);
  const [kind, setKind] = createSignal<"process" | "cache_path">("process");
  const [value, setValue] = createSignal("");
  const [note, setNote] = createSignal("");

  const [autostart, setAutostart] = createSignal(false);
  const [notifyGranted, setNotifyGranted] = createSignal(false);
  const [prefs, setPrefs] = createSignal<Prefs>(defaultPrefs);

  const load = async () => {
    setItems(await getWhitelist());
    try {
      setAutostart(await autostartIsEnabled());
    } catch {
      /* noop */
    }
    try {
      setNotifyGranted(await isPermissionGranted());
    } catch {
      /* noop */
    }
    setPrefs(loadPrefs());
  };

  onMount(load);

  const toggleAutostart = async (next: boolean) => {
    try {
      if (next) await autostartEnable();
      else await autostartDisable();
      setAutostart(next);
    } catch (e) {
      console.error(e);
    }
  };

  const ensureNotifyPermission = async () => {
    if (notifyGranted()) return;
    const result = await requestPermission();
    setNotifyGranted(result === "granted");
  };

  const updatePref = (patch: Partial<Prefs>) => {
    const next = { ...prefs(), ...patch };
    setPrefs(next);
    savePrefs(next);
  };

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
        <h2 class="text-base font-semibold">通用设置</h2>
        <p class="text-xs text-zinc-500 mt-0.5">基础行为开关</p>
      </div>

      <div class="card p-2">
        <ToggleRow
          label="开机自动启动"
          desc="macOS 登录时自动启动 MacFlow 并最小化到菜单栏"
          checked={autostart()}
          onChange={toggleAutostart}
        />
        <ToggleRow
          label="清理完成通知"
          desc="在 macOS 通知中心提示已释放空间"
          checked={prefs().notifyOnCleanComplete && notifyGranted()}
          disabled={!notifyGranted() && prefs().notifyOnCleanComplete}
          onChange={async (v) => {
            if (v) await ensureNotifyPermission();
            updatePref({ notifyOnCleanComplete: v });
          }}
        />
        <Show when={!notifyGranted() && prefs().notifyOnCleanComplete}>
          <div class="px-4 pb-3 -mt-1">
            <button
              type="button"
              class="text-xs text-brand-600 hover:underline"
              onClick={ensureNotifyPermission}
            >
              通知权限未授权，点此请求
            </button>
          </div>
        </Show>
      </div>

      <div class="card p-4">
        <div class="flex items-center justify-between mb-3 px-1">
          <div>
            <div class="text-sm font-medium">自定义白名单</div>
            <div class="text-xs text-zinc-500 mt-0.5">
              已添加 {items().length} 项 · 白名单项永远不会被扫描或清理
            </div>
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
              还没有自定义白名单。在扫描列表上点击盾牌图标可以快速添加。
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
          <div>版本 0.1.0 · 规则驱动 · 本地存储 · 开源友好</div>
          <div>不上传任何数据，不接任何 LLM，不做广告推送</div>
          <div class="font-mono text-[10px] mt-2">
            数据：~/Library/Application Support/MacFlow/macflow.db
          </div>
          <div class="font-mono text-[10px]">
            CLI：~/MacFlow/src-tauri/target/debug/macflow-cli
          </div>
        </div>
      </div>
    </div>
  );
};

const ToggleRow: Component<{
  label: string;
  desc?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (next: boolean) => void;
}> = (props) => (
  <label class="flex items-start justify-between gap-4 px-3 py-3 rounded-lg hover:bg-black/[0.02] dark:hover:bg-white/[0.02] cursor-pointer">
    <div class="flex-1 min-w-0">
      <div class="text-sm font-medium">{props.label}</div>
      <Show when={props.desc}>
        <div class="text-xs text-zinc-500 mt-0.5">{props.desc}</div>
      </Show>
    </div>
    <button
      type="button"
      role="switch"
      aria-checked={props.checked}
      onClick={() => !props.disabled && props.onChange(!props.checked)}
      class={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors ${
        props.checked ? "bg-brand-500" : "bg-zinc-300 dark:bg-zinc-700"
      } ${props.disabled ? "opacity-50 cursor-not-allowed" : ""}`}
    >
      <span
        class={`inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform ${
          props.checked ? "translate-x-5" : "translate-x-0.5"
        }`}
      />
    </button>
  </label>
);

export default SettingsView;
