import { Component, createSignal, Match, onMount, Switch } from "solid-js";
import Sidebar, { type ViewId } from "@/components/Sidebar";
import ScanView from "@/views/ScanView";
import CacheView from "@/views/CacheView";
import HistoryView from "@/views/HistoryView";
import SettingsView from "@/views/SettingsView";
import Placeholder from "@/views/Placeholder";
import { listen } from "@tauri-apps/api/event";

const viewLabels: Record<ViewId, string> = {
  scan: "智能扫描",
  process: "进程管理",
  cache: "缓存清理",
  history: "历史记录",
  settings: "设置",
};

const App: Component = () => {
  const [view, setView] = createSignal<ViewId>("scan");

  onMount(() => {
    // 托盘菜单 -> 立即扫描
    listen<void>("tray:scan", () => {
      setView("scan");
    });
  });

  return (
    <div class="flex h-full bg-[rgb(var(--bg-app))/var(--bg-app-alpha)]">
      <Sidebar current={view()} onChange={setView} />
      <main class="flex-1 flex flex-col min-w-0">
        <div class="drag-region h-12 flex items-center px-6 border-b border-black/5 dark:border-white/5">
          <h1 class="text-sm font-medium text-zinc-500">
            {viewLabels[view()]}
          </h1>
        </div>
        <div class="flex-1 min-h-0 no-drag">
          <Switch>
            <Match when={view() === "scan"}>
              <ScanView />
            </Match>
            <Match when={view() === "process"}>
              <Placeholder
                title="进程管理"
                desc="高级筛选、白名单快速添加、端口占用查看。里程碑 3 上线。"
              />
            </Match>
            <Match when={view() === "cache"}>
              <CacheView />
            </Match>
            <Match when={view() === "history"}>
              <HistoryView />
            </Match>
            <Match when={view() === "settings"}>
              <SettingsView />
            </Match>
          </Switch>
        </div>
      </main>
    </div>
  );
};

export default App;
