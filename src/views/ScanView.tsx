import {
  Component,
  createSignal,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import HealthCard from "@/components/HealthCard";
import ProcessList from "@/components/ProcessList";
import Welcome from "@/components/Welcome";
import {
  scanAll,
  killProcesses,
  addWhitelist,
  type ScanResult,
  type SystemHealth,
} from "@/lib/tauri";
import { Sparkles, RefreshCw, Loader2 } from "lucide-solid";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useI18n } from "@/i18n";

const WELCOME_SEEN_KEY = "macflow.welcome.seen";

const ScanView: Component = () => {
  const { t } = useI18n();
  const [result, setResult] = createSignal<ScanResult | null>(null);
  const [scanning, setScanning] = createSignal(false);
  const [selected, setSelected] = createSignal(new Set<number>());
  const [optimizing, setOptimizing] = createSignal(false);
  const [message, setMessage] = createSignal<string | null>(null);
  const [showWelcome, setShowWelcome] = createSignal(
    localStorage.getItem(WELCOME_SEEN_KEY) !== "true",
  );

  const runScan = async () => {
    setScanning(true);
    setMessage(null);
    try {
      const r = await scanAll();
      setResult(r);
      const defaults = new Set(
        r.processes.filter((p) => p.default_select).map((p) => p.pid),
      );
      setSelected(defaults);
    } catch (e) {
      setMessage(t("scan.scanFailed", { error: String(e) }));
    } finally {
      setScanning(false);
    }
  };

  let unlistenHealth: UnlistenFn | undefined;
  let unlistenOptimize: UnlistenFn | undefined;
  onMount(async () => {
    if (!showWelcome()) {
      await runScan();
    }
    unlistenHealth = await listen<SystemHealth>("health:update", (e) => {
      const r = result();
      if (!r) return;
      setResult({ ...r, health: e.payload });
    });
    // 托盘「一键优化」菜单触发
    unlistenOptimize = await listen<void>("tray:optimize", async () => {
      await runScan();
      await optimize();
    });
  });
  onCleanup(() => {
    unlistenHealth?.();
    unlistenOptimize?.();
  });

  const handleStart = async () => {
    localStorage.setItem(WELCOME_SEEN_KEY, "true");
    setShowWelcome(false);
    await runScan();
  };

  const toggle = (pid: number) => {
    const next = new Set(selected());
    next.has(pid) ? next.delete(pid) : next.add(pid);
    setSelected(next);
  };

  const optimize = async () => {
    const pids = Array.from(selected());
    if (pids.length === 0) return;
    const procs = result()?.processes ?? [];
    const names = pids.map(
      (pid) => procs.find((p) => p.pid === pid)?.name ?? String(pid),
    );
    setOptimizing(true);
    setMessage(null);
    try {
      const r = await killProcesses(pids, names);
      let msg = t("scan.killSuccess", { count: r.killed.length });
      if (r.failed.length > 0) {
        msg += t("scan.killPartial", { failed: r.failed.length });
        // 把失败的原因拼进 message
        const failReasons = r.details
          .filter((d) => !d.success)
          .map((d) => `${d.name}: ${d.message}`)
          .join("；");
        if (failReasons) msg += ` —— ${failReasons}`;
      }
      setMessage(msg);
      await runScan();
    } catch (e) {
      setMessage(t("scan.optimizeFailed", { error: String(e) }));
    } finally {
      setOptimizing(false);
    }
  };

  if (showWelcome()) {
    return <Welcome onStart={handleStart} />;
  }

  return (
    <div class="flex flex-col gap-5 p-6 h-full overflow-y-auto">
      <HealthCard health={result()?.health ?? null} />

      <ProcessList
        processes={result()?.processes ?? []}
        selected={selected()}
        onToggle={toggle}
        onWhitelist={async (name) => {
          await addWhitelist("process", name, "scan list add");
          setMessage(t("scan.whitelistAdded", { name }));
          await runScan();
        }}
      />

      <div class="flex items-center gap-3">
        <button
          type="button"
          class="btn-primary gap-2 min-w-[180px]"
          disabled={optimizing() || scanning() || selected().size === 0}
          onClick={optimize}
        >
          <Show
            when={!optimizing()}
            fallback={<Loader2 size={16} class="animate-spin" />}
          >
            <Sparkles size={16} />
          </Show>
          {t("scan.oneClick")}
          {selected().size > 0 ? ` (${selected().size})` : ""}
        </button>

        <button
          type="button"
          class="btn-ghost gap-2"
          disabled={scanning()}
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

        <Show when={message()}>
          <span class="text-xs text-zinc-500 animate-fade-in">{message()}</span>
        </Show>

        <span class="ml-auto text-[11px] text-zinc-400">
          {t("common.notice_irreversible")}
        </span>
      </div>
    </div>
  );
};

export default ScanView;
