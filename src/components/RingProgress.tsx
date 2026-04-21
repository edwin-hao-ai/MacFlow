import { Component, createMemo } from "solid-js";

type Props = {
  value: number; // 0-100
  size?: number;
  stroke?: number;
  label?: string;
  sublabel?: string;
};

const RingProgress: Component<Props> = (props) => {
  const size = () => props.size ?? 96;
  const stroke = () => props.stroke ?? 8;
  const r = createMemo(() => (size() - stroke()) / 2);
  const circ = createMemo(() => 2 * Math.PI * r());
  const clamped = createMemo(() => Math.max(0, Math.min(100, props.value)));
  const offset = createMemo(() => circ() * (1 - clamped() / 100));

  const color = createMemo(() => {
    const v = clamped();
    if (v < 60) return "text-success-500";
    if (v < 85) return "text-warning-500";
    return "text-danger-500";
  });

  return (
    <div
      class="relative inline-flex items-center justify-center"
      style={{ width: `${size()}px`, height: `${size()}px` }}
    >
      <svg width={size()} height={size()} class="-rotate-90">
        <circle
          cx={size() / 2}
          cy={size() / 2}
          r={r()}
          stroke="currentColor"
          stroke-width={stroke()}
          fill="none"
          class="text-black/10 dark:text-white/10"
        />
        <circle
          cx={size() / 2}
          cy={size() / 2}
          r={r()}
          stroke="currentColor"
          stroke-width={stroke()}
          stroke-linecap="round"
          fill="none"
          stroke-dasharray={String(circ())}
          stroke-dashoffset={String(offset())}
          class={`${color()} transition-[stroke-dashoffset] duration-500 ease-out`}
        />
      </svg>
      <div class="absolute inset-0 flex flex-col items-center justify-center">
        <div class="text-xl font-semibold tabular-nums">
          {Math.round(clamped())}
          <span class="text-sm font-medium text-zinc-500">%</span>
        </div>
        {props.sublabel && (
          <div class="text-[10px] text-zinc-500 mt-0.5">{props.sublabel}</div>
        )}
      </div>
    </div>
  );
};

export default RingProgress;
