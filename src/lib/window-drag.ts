import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * 显式调用 Tauri startDragging API 实现窗口拖动。
 *
 * 背景：Tauri v2 在 macOS + transparent: true + titleBarStyle: Overlay 配置下，
 * `data-tauri-drag-region` 自动处理和 `-webkit-app-region: drag` CSS 都会失效，
 * 必须用 JS 主动调用 API 才能可靠拖动。
 *
 * 用法：在容器元素上 `onMouseDown={handleWindowDrag}`。
 * 子元素如果是 button/input/a/textarea/select 或带 [data-no-drag] 属性，会自动跳过。
 */
export const handleWindowDrag = (e: MouseEvent) => {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  // 排除：交互元素 + 卡片本体 + 列表项 + 任何带 [data-no-drag] 标记
  // 这样主内容区的「卡片之间的 padding 间隙」可以拖动，卡片内部正常交互
  if (target.closest("button, input, textarea, select, a, label, .card, li, [data-no-drag]")) return;
  e.preventDefault();
  void getCurrentWindow().startDragging();
};
