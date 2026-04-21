import { Component, createSignal, onCleanup, onMount, Show } from "solid-js";
import HealthCard from "@/components/HealthCard";
import ProcessList from "@/components/ProcessList";
import {
  scanAll,
  killProcesses,
  addWhitelist,
  type ScanResult,
  type SystemHealth,
} from "@/lib/tauri";
import { Sparkles, RefreshCw, Loader2 } from "lucide-solid";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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

  // 订阅后台监控的健康更新：不需要完整重扫就能看到 CPU/内存/磁盘变化
  let unlisten: UnlistenFn | undefined;
  onMount(async () => {
    await runScan();
    unlisten = await listen<SystemHealth>("health:update", (e) => {
      const r = result();
      if (!r) return;
      setResult({ ...r, health: e.payload });
    });
  });
  onCleanup(() => unlisten?.());

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
        onWhitelist={async (name) => {
          await addWhitelist("process", name, "从扫描列表添加");
          setMessage(`${name} 已加入白名单，下次扫描不会再显示`);
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
