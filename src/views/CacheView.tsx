import {
  Component,
  createMemo,
  createSignal,
  For,
  onMount,
  Show,
} from "solid-js";
import {
  cleanCache,
  scanCache,
  type CacheItem,
  type CacheScanResult,
  type CleanSummary,
} from "@/lib/tauri";
import {
  CATEGORY_COLORS,
  CATEGORY_LABELS,
  fmtBytes,
  fmtDuration,
} from "@/lib/format";
import { CheckCircle2, Loader2, RefreshCw, Sparkles, XCircle } from "lucide-solid";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

function isNotifyEnabled(): boolean {
  try {
    const raw = localStorage.getItem("macflow.prefs.v1");
    if (!raw) return true; // 默认开
    const p = JSON.parse(raw);
    return p?.notifyOnCleanComplete !== false;
  } catch {
    return true;
  }
}

async function notifyCleanComplete(bytes: number, count: number) {
  if (!isNotifyEnabled()) return;
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      granted = (await requestPermission()) === "granted";
    }
    if (!granted) return;
    sendNotification({
      title: "MacFlow 清理完成",
      body: `已释放 ${fmtBytes(bytes)}，共清理 ${count} 项`,
    });
  } catch {
    // 通知失败不影响清理本身
  }
}

const CacheView: Component = () => {
  const [result, setResult] = createSignal<CacheScanResult | null>(null);
  const [scanning, setScanning] = createSignal(false);
  const [selected, setSelected] = createSignal(new Set<string>());
  const [cleaning, setCleaning] = createSignal(false);
  const [summary, setSummary] = createSignal<CleanSummary | null>(null);

  const runScan = async () => {
    setScanning(true);
    setSummary(null);
    try {
      const r = await scanCache();
      setResult(r);
      const defaults = new Set(
        r.items.filter((i) => i.default_select).map((i) => i.id),
      );
      setSelected(defaults);
    } catch (e) {
      console.error(e);
    } finally {
      setScanning(false);
    }
  };

  onMount(runScan);

  const toggle = (id: string) => {
    const next = new Set(selected());
    next.has(id) ? next.delete(id) : next.add(id);
    setSelected(next);
  };

  const selectedBytes = createMemo(() => {
    const items = result()?.items ?? [];
    return items
      .filter((i) => selected().has(i.id))
      .reduce((s, i) => s + i.size_bytes, 0);
  });

  const runClean = async () => {
    const items = (result()?.items ?? []).filter((i) => selected().has(i.id));
    if (items.length === 0) return;
    setCleaning(true);
    setSummary(null);
    try {
      const s = await cleanCache(items);
      setSummary(s);
      await runScan();
      await notifyCleanComplete(s.total_freed_bytes, s.success_count);
    } catch (e) {
      console.error(e);
    } finally {
      setCleaning(false);
    }
  };

  // 按 category 分组
  const grouped = createMemo(() => {
    const items = result()?.items ?? [];
    const map = new Map<string, CacheItem[]>();
    for (const i of items) {
      if (!map.has(i.category)) map.set(i.category, []);
      map.get(i.category)!.push(i);
    }
    return Array.from(map.entries());
  });

  return (
    <div class="flex flex-col gap-5 p-6 h-full overflow-y-auto">
      {/* 顶部统计卡 */}
      <div class="card p-6 animate-fade-in">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-base font-semibold">开发者缓存</h2>
            <p class="text-xs text-zinc-500 mt-0.5">
              NPM / Docker / Xcode / Homebrew / Cargo 等
            </p>
          </div>
          <Show
            when={!scanning()}
            fallback={
              <div class="flex items-center gap-2 text-xs text-zinc-500">
                <Loader2 size={14} class="animate-spin" />
                正在扫描缓存...
              </div>
            }
          >
            <div class="text-right">
              <div class="text-3xl font-bold text-brand-600 tabular-nums">
                {fmtBytes(result()?.total_bytes ?? 0)}
              </div>
              <div class="text-xs text-zinc-500">可释放空间</div>
            </div>
          </Show>
        </div>
      </div>

      <Show when={summary()}>
        {(s) => (
          <div class="card p-5 animate-slide-up bg-success-500/5 border-success-500/20">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 rounded-full bg-success-500/15 flex items-center justify-center">
                <CheckCircle2 size={20} class="text-success-600" />
              </div>
              <div>
                <div class="font-semibold">
                  清理完成，释放 {fmtBytes(s().total_freed_bytes)}
                </div>
                <div class="text-xs text-zinc-500">
                  成功 {s().success_count} 项
                  {s().fail_count > 0 && ` · 失败 ${s().fail_count} 项`}
                </div>
              </div>
            </div>
          </div>
        )}
      </Show>

      {/* 分组列表 */}
      <Show
        when={(result()?.items.length ?? 0) > 0}
        fallback={
          <Show when={!scanning()}>
            <div class="card p-12 text-center text-sm text-zinc-500">
              没有发现可清理的缓存。你的 Mac 很干净！
            </div>
          </Show>
        }
      >
        <For each={grouped()}>
          {([category, items]) => (
            <div class="card p-4 animate-fade-in">
              <div class="flex items-center gap-2 mb-3 px-1">
                <span
                  class={`px-2 py-0.5 rounded-md text-[11px] font-semibold ${CATEGORY_COLORS[category] ?? ""}`}
                >
                  {CATEGORY_LABELS[category] ?? category}
                </span>
                <span class="text-xs text-zinc-500">
                  {items.length} 项 ·{" "}
                  {fmtBytes(items.reduce((s, i) => s + i.size_bytes, 0))}
                </span>
              </div>
              <ul class="space-y-1">
                <For each={items}>
                  {(item) => (
                    <li class="flex items-start gap-3 p-2 rounded-lg hover:bg-black/[0.02] dark:hover:bg-white/[0.02]">
                      <input
                        type="checkbox"
                        checked={selected().has(item.id)}
                        onChange={() => toggle(item.id)}
                        class="mt-1 w-4 h-4 rounded accent-brand-500"
                      />
                      <div class="flex-1 min-w-0">
                        <div class="flex items-center gap-2">
                          <span class="font-medium text-sm">{item.label}</span>
                          {item.safety === "safe" && (
                            <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-success-500/15 text-success-600">
                              安全
                            </span>
                          )}
                          {item.safety === "low" && (
                            <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">
                              低风险
                            </span>
                          )}
                          {item.safety === "medium" && (
                            <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-danger-500/15 text-danger-600">
                              注意
                            </span>
                          )}
                        </div>
                        <div class="text-xs text-zinc-500 mt-0.5">
                          {item.description}
                        </div>
                        <Show when={item.command}>
                          <div class="text-[10px] font-mono text-zinc-400 mt-1 truncate">
                            $ {item.command}
                          </div>
                        </Show>
                        <Show when={item.path}>
                          <div class="text-[10px] font-mono text-zinc-400 mt-0.5 truncate">
                            {item.path}
                          </div>
                        </Show>
                      </div>
                      <div class="text-right tabular-nums text-sm font-semibold min-w-[80px]">
                        {fmtBytes(item.size_bytes)}
                      </div>
                    </li>
                  )}
                </For>
              </ul>
            </div>
          )}
        </For>
      </Show>

      {/* 底部操作条 */}
      <div class="flex items-center gap-3 pb-4">
        <button
          type="button"
          class="btn-primary gap-2 min-w-[200px]"
          disabled={
            cleaning() ||
            scanning() ||
            selected().size === 0 ||
            selectedBytes() === 0
          }
          onClick={runClean}
        >
          <Show
            when={!cleaning()}
            fallback={<Loader2 size={16} class="animate-spin" />}
          >
            <Sparkles size={16} />
          </Show>
          清理 {fmtBytes(selectedBytes())} ({selected().size})
        </button>

        <button
          type="button"
          class="btn-ghost gap-2"
          disabled={scanning() || cleaning()}
          onClick={runScan}
        >
          <Show
            when={!scanning()}
            fallback={<Loader2 size={16} class="animate-spin" />}
          >
            <RefreshCw size={16} />
          </Show>
          重新扫描
        </button>

        <span class="ml-auto text-[11px] text-zinc-400">
          清理操作不可撤销，请确认后执行
        </span>
      </div>

      {/* 清理详情 */}
      <Show when={summary() && summary()!.fail_count > 0}>
        <div class="card p-4 border-danger-500/20">
          <div class="flex items-center gap-2 mb-2">
            <XCircle size={16} class="text-danger-500" />
            <span class="font-medium text-sm">部分项目清理失败</span>
          </div>
          <ul class="text-xs space-y-1">
            <For each={summary()!.reports.filter((r) => !r.success)}>
              {(r) => (
                <li class="flex gap-2">
                  <span class="font-medium min-w-[140px]">{r.label}</span>
                  <span class="text-zinc-500">{r.error}</span>
                  <span class="ml-auto text-zinc-400">
                    {fmtDuration(r.duration_ms)}
                  </span>
                </li>
              )}
            </For>
          </ul>
        </div>
      </Show>
    </div>
  );
};

export default CacheView;
