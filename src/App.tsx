import { Component, createSignal, Match, onMount, Switch } from "solid-js";
import Sidebar, { type ViewId } from "@/components/Sidebar";
import ScanView from "@/views/ScanView";
import CacheView from "@/views/CacheView";
import HistoryView from "@/views/HistoryView";
import SettingsView from "@/views/SettingsView";
import Placeholder from "@/views/Placeholder";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "@/i18n";

const App: Component = () => {
  const { t } = useI18n();
  const [view, setView] = createSignal<ViewId>("scan");

  onMount(() => {
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
            {t(`nav.${view()}`)}
          </h1>
        </div>
        <div class="flex-1 min-h-0 no-drag">
          <Switch>
            <Match when={view() === "scan"}>
              <ScanView />
            </Match>
            <Match when={view() === "process"}>
              <Placeholder
                title={t("placeholder.processTitle")}
                desc={t("placeholder.processDesc")}
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
