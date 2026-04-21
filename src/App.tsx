import { Component, createSignal, Match, Switch } from "solid-js";
import Sidebar, { type ViewId } from "@/components/Sidebar";
import ScanView from "@/views/ScanView";
import Placeholder from "@/views/Placeholder";

const App: Component = () => {
  const [view, setView] = createSignal<ViewId>("scan");

  return (
    <div class="flex h-full bg-[rgb(var(--bg-app))/var(--bg-app-alpha)]">
      <Sidebar current={view()} onChange={setView} />
      <main class="flex-1 flex flex-col min-w-0">
        <div class="drag-region h-12 flex items-center px-6 border-b border-black/5 dark:border-white/5">
          <h1 class="text-sm font-medium text-zinc-500">
            <Switch>
              <Match when={view() === "scan"}>智能扫描</Match>
              <Match when={view() === "process"}>进程管理</Match>
              <Match when={view() === "cache"}>缓存清理</Match>
              <Match when={view() === "settings"}>设置</Match>
            </Switch>
          </h1>
        </div>
        <div class="flex-1 min-h-0 no-drag">
          <Switch>
            <Match when={view() === "scan"}>
              <ScanView />
            </Match>
            <Match when={view() === "process"}>
              <Placeholder title="进程管理" desc="高级进程筛选、白名单管理等。里程碑 2 上线。" />
            </Match>
            <Match when={view() === "cache"}>
              <Placeholder title="缓存清理" desc="NPM / Docker / Xcode / Homebrew 深度清理。里程碑 2 上线。" />
            </Match>
            <Match when={view() === "settings"}>
              <Placeholder title="设置" desc="开机启动、自动监控、清理阈值等。里程碑 3 上线。" />
            </Match>
          </Switch>
        </div>
      </main>
    </div>
  );
};

export default App;
