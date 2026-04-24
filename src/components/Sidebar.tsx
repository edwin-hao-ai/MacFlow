import { Component, For } from "solid-js";
import {
  Activity,
  Cpu,
  HardDrive,
  History as HistoryIcon,
  Package,
  Settings as SettingsIcon,
  Trash2,
} from "lucide-solid";
import { useI18n } from "@/i18n";

export type ViewId =
  | "scan"
  | "process"
  | "applications"
  | "cache"
  | "uninstaller"
  | "history"
  | "settings";

type Props = {
  current: ViewId;
  onChange: (id: ViewId) => void;
};

const items: { id: ViewId; icon: Component<{ size?: number }> }[] = [
  { id: "scan", icon: Activity },
  { id: "process", icon: Cpu },
  { id: "applications", icon: Package },
  { id: "cache", icon: HardDrive },
  { id: "uninstaller", icon: Trash2 },
  { id: "history", icon: HistoryIcon },
  { id: "settings", icon: SettingsIcon },
];

const Sidebar: Component<Props> = (props) => {
  const { t } = useI18n();
  return (
    <aside class="w-[200px] flex flex-col border-r border-black/5 dark:border-white/5 bg-[rgb(var(--bg-sidebar))/var(--bg-sidebar-alpha)]">
      <div class="drag-region h-13 flex items-end pb-2 px-5">
        <div class="flex items-center gap-2">
          <div class="w-6 h-6 rounded-lg bg-gradient-to-br from-brand-400 to-brand-600 flex items-center justify-center shadow-sm">
            <Activity size={13} class="text-white" />
          </div>
          <span class="font-semibold tracking-tight text-[15px]">
            {t("common.appName")}
          </span>
        </div>
      </div>
      <nav class="px-2 py-2 flex flex-col gap-0.5 no-drag">
        <For each={items}>
          {(item) => (
            <button
              type="button"
              class="sidebar-item"
              data-active={props.current === item.id}
              onClick={() => props.onChange(item.id)}
            >
              <item.icon size={16} />
              <span>{t(`nav.${item.id}`)}</span>
            </button>
          )}
        </For>
      </nav>
      <div class="mt-auto p-3 text-[10px] text-zinc-400">v0.1.0</div>
    </aside>
  );
};

export default Sidebar;
