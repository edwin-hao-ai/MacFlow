/**
 * 清理完成时的全页光晕 + 冲击波覆盖层。
 * 纯 CSS transform + opacity 动画，不触发 layout/paint。
 * 动画总时长 ~800ms，一次性播放后自动卸载。
 */
import { Component, createSignal, onCleanup, onMount, Show } from "solid-js";

export type CleanupFlashProps = {
  /** 控制是否显示 */
  visible: boolean;
  /** 动画结束后回调（父组件用来重置 visible） */
  onDone: () => void;
};

const CleanupFlash: Component<CleanupFlashProps> = (props) => {
  const [alive, setAlive] = createSignal(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const dismiss = () => {
    if (!alive()) return;
    setAlive(false);
    props.onDone();
  };

  onMount(() => {
    // 下一帧触发动画
    requestAnimationFrame(() => {
      setAlive(true);
      // 超时兜底：800ms 后无论如何都消失
      timer = setTimeout(dismiss, 800);
    });
  });

  onCleanup(() => {
    if (timer) clearTimeout(timer);
  });

  return (
    <Show when={props.visible}>
      <div
        class="cleanup-flash-overlay"
        classList={{ "cleanup-flash-overlay--active": alive() }}
      >
        <div
          class="cleanup-flash-glow"
          classList={{ "cleanup-flash-glow--active": alive() }}
        />
        <div
          class="cleanup-flash-ring"
          classList={{ "cleanup-flash-ring--active": alive() }}
        />
      </div>
    </Show>
  );
};

export default CleanupFlash;
