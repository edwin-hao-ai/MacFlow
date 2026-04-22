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
import { useI18n, getT } from "@/i18n";

function isNotifyEnabled(): boolean {
  try {
    const raw = localStorage.getItem("macflow.prefs.v1");
    if (!raw) return true;
    const p = JSON.parse(raw);
    return p?.notifyOnCleanComplete !== false;
  } catch {
    return true;
  }
}

async function notifyCleanComplete(bytes: number, count: number) {
  if (!isNotifyEnabled()) return;
  const t = getT();
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      granted = (await requestPermission()) === "granted";
    }
    if (!granted) return;
    sendNotification({
      title: t("cache.notifyTitle"),
      body: t("cache.notifyBody", { size: fmtBytes(bytes), count }),
    });
  } catch {
    /* noop */
  }
}

const CacheView: Component = () => {
  const { t } = useI18n();
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
      <div class="card p-6 animate-fade-in">
        <div class="flex items-center justify-between">
          <div>
            <h2 class="text-base font-semibold">{t("cache.title")}</h2>
            <p class="text-xs text-zinc-500 mt-0.5">{t("cache.subtitle")}</p>
          </div>
          <Show
            when={!scanning()}
            fallback={
              <div class="flex items-center gap-2 text-xs text-zinc-500">
                <Loader2 size={14} class="animate-spin" />
                {t("cache.scanning")}
              </div>
            }
          >
            <div class="text-right">
              <div class="text-3xl font-bold text-brand-600 tabular-nums">
                {fmtBytes(result()?.total_bytes ?? 0)}
              </div>
              <div class="text-xs text-zinc-500">{t("cache.freeable")}</div>
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
                  {t("cache.cleanSuccess", {
                    size: fmtBytes(s().total_freed_bytes),
                  })}
                </div>
                <div class="text-xs text-zinc-500">
                  {t("cache.successItems", { count: s().success_count })}
                  {s().fail_count > 0 &&
                    ` · ${t("cache.failItems", { count: s().fail_count })}`}
                </div>
              </div>
            </div>
          </div>
        )}
      </Show>

      <Show
        when={(result()?.items.length ?? 0) > 0}
        fallback={
          <Show when={!scanning()}>
            <div class="card p-12 text-center text-sm text-zinc-500">
              {t("cache.noItems")}
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
                  {t("cache.groupCount", {
                    count: items.length,
                    size: fmtBytes(items.reduce((s, i) => s + i.size_bytes, 0)),
                  })}
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
                              {t("risk.safe")}
                            </span>
                          )}
                          {item.safety === "low" && (
                            <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-warning-500/15 text-warning-600">
                              {t("risk.low")}
                            </span>
                          )}
                          {item.safety === "medium" && (
                            <span class="px-1.5 py-0.5 rounded-md text-[10px] font-medium bg-danger-500/15 text-danger-600">
                              {t("risk.notice")}
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
          {t("cache.cleanCta", {
            size: fmtBytes(selectedBytes()),
            count: selected().size,
          })}
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
          {t("common.rescan")}
        </button>

        <span class="ml-auto text-[11px] text-zinc-400">
          {t("common.notice_irreversible")}
        </span>
      </div>

      <Show when={summary() && summary()!.fail_count > 0}>
        <div class="card p-4 border-danger-500/20">
          <div class="flex items-center gap-2 mb-2">
            <XCircle size={16} class="text-danger-500" />
            <span class="font-medium text-sm">{t("cache.partialFail")}</span>
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
