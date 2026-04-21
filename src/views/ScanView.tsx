import { Component, createSignal, onMount, Show } from "solid-js";
import HealthCard from "@/components/HealthCard";
import ProcessList from "@/components/ProcessList";
import { scanAll, killProcesses, type ScanResult } from "@/lib/tauri";
import { Sparkles, RefreshCw, Loader2 } from "lucide-solid";

const ScanView: Component = () => {
  const [result, setResult] = createSignal<ScanResult | null>(null);
  const [scanning, setScanning] = createSignal(false);
  const [selected, setSelected] = createSignal(new Set<number>());
  const [optimizing, setOptimizing] = createSignal(false);
  const [message, setMessage] = createSignal<string | null>(null);

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
      setMessage(`扫描失败: ${String(e)}`);
    } finally {
      setScanning(false);
    }
  };

  onMount(runScan);

  const toggle = (pid: number) => {
    const next = new Set(selected());
    next.has(pid) ? next.delete(pid) : next.add(pid);
    setSelected(next);
  };

  const optimize = async () => {
    const pids = Array.from(selected());
    if (pids.length === 0) return;
    setOptimizing(true);
    setMessage(null);
    try {
      const r = await killProcesses(pids);
      setMessage(
        `已终止 ${r.killed.length} 个进程` +
          (r.failed.length > 0 ? `，${r.failed.length} 个失败` : ""),
      );
      await runScan();
    } catch (e) {
      setMessage(`优化失败: ${String(e)}`);
    } finally {
      setOptimizing(false);
    }
  };

  return (
    <div class="flex flex-col gap-5 p-6 h-full overflow-y-auto">
      <HealthCard health={result()?.health ?? null} />

      <ProcessList
        processes={result()?.processes ?? []}
        selected={selected()}
        onToggle={toggle}
      />

      <div class="flex items-center gap-3">
        <button
          type="button"
          class="btn-primary gap-2 min-w-[180px]"
          disabled={optimizing() || scanning() || selected().size === 0}
          onClick={optimize}
        >
          <Show when={!optimizing()} fallback={<Loader2 size={16} class="animate-spin" />}>
            <Sparkles size={16} />
          </Show>
          一键优化 ({selected().size})
        </button>

        <button
          type="button"
          class="btn-ghost gap-2"
          disabled={scanning()}
          onClick={runScan}
        >
          <Show when={!scanning()} fallback={<Loader2 size={16} class="animate-spin" />}>
            <RefreshCw size={16} />
          </Show>
          重新扫描
        </button>

        <Show when={message()}>
          <span class="text-xs text-zinc-500 animate-fade-in">{message()}</span>
        </Show>

        <span class="ml-auto text-[11px] text-zinc-400">
          此操作不可撤销，请确认后执行
        </span>
      </div>
    </div>
  );
};

export default ScanView;
