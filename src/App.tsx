import { Component, createSignal, onMount } from "solid-js";
import Sidebar, { type ViewId } from "@/components/Sidebar";
import ScanView from "@/views/ScanView";
import CacheView from "@/views/CacheView";
import HistoryView from "@/views/HistoryView";
import SettingsView from "@/views/SettingsView";
import ProcessView from "@/views/ProcessView";
import ApplicationsView from "@/views/ApplicationsView";
import UninstallerView from "@/views/UninstallerView";
import { listen } from "@tauri-apps/api/event";
import { handleWindowDrag } from "@/lib/window-drag";
import { useI18n } from "@/i18n";

/** 用 CSS display 切换的 tab 面板，组件始终挂载不丢状态 */
const TabPanel: Component<{ id: ViewId; active: ViewId; children: any }> = (props) => (
  <div
    class="h-full"
    style={{ display: props.active === props.id ? "block" : "none" }}
  >
    {props.children}
  </div>
);

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
        <div
          class="drag-region h-12 flex items-center px-6 border-b border-black/5 dark:border-white/5"
          data-tauri-drag-region
          onMouseDown={handleWindowDrag}
        >
          <h1 class="text-sm font-medium text-zinc-500 pointer-events-none">
            {t(`nav.${view()}`)}
          </h1>
        </div>
        <div class="flex-1 min-h-0 overflow-hidden" onMouseDown={handleWindowDrag}>
          <TabPanel id="scan" active={view()}><ScanView /></TabPanel>
          <TabPanel id="process" active={view()}><ProcessView /></TabPanel>
          <TabPanel id="applications" active={view()}><ApplicationsView /></TabPanel>
          <TabPanel id="cache" active={view()}><CacheView /></TabPanel>
          <TabPanel id="uninstaller" active={view()}><UninstallerView /></TabPanel>
          <TabPanel id="history" active={view()}><HistoryView /></TabPanel>
          <TabPanel id="settings" active={view()}><SettingsView /></TabPanel>
        </div>
      </main>
    </div>
  );
};

export default App;
