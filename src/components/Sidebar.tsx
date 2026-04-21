import { Component, For } from "solid-js";
import {
  Activity,
  Cpu,
  HardDrive,
  History as HistoryIcon,
  Settings as SettingsIcon,
} from "lucide-solid";

export type ViewId = "scan" | "process" | "cache" | "history" | "settings";

type Props = {
  current: ViewId;
  onChange: (id: ViewId) => void;
};

const items: { id: ViewId; label: string; icon: Component<{ size?: number }> }[] = [
  { id: "scan", label: "智能扫描", icon: Activity },
  { id: "process", label: "进程管理", icon: Cpu },
  { id: "cache", label: "缓存清理", icon: HardDrive },
  { id: "history", label: "历史记录", icon: HistoryIcon },
  { id: "settings", label: "设置", icon: SettingsIcon },
];

const Sidebar: Component<Props> = (props) => {
  return (
    <aside class="w-[200px] flex flex-col border-r border-black/5 dark:border-white/5 bg-[rgb(var(--bg-sidebar))/var(--bg-sidebar-alpha)]">
      <div class="drag-region h-12 flex items-center px-5">
        <span class="font-semibold tracking-tight text-[15px]">MacFlow</span>
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
              <span>{item.label}</span>
            </button>
          )}
        </For>
      </nav>
      <div class="mt-auto p-3 text-[10px] text-zinc-400">v0.1.0</div>
    </aside>
  );
};

export default Sidebar;
