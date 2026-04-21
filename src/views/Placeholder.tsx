import { Component } from "solid-js";
import { Construction } from "lucide-solid";

const Placeholder: Component<{ title: string; desc?: string }> = (props) => (
  <div class="flex flex-col items-center justify-center h-full text-center p-10">
    <div class="w-14 h-14 rounded-2xl bg-black/5 dark:bg-white/5 flex items-center justify-center mb-4">
      <Construction size={24} class="text-zinc-400" />
    </div>
    <h3 class="text-base font-semibold">{props.title}</h3>
    <p class="text-sm text-zinc-500 mt-1 max-w-xs">
      {props.desc ?? "该模块正在建设中，里程碑 2 及之后版本发布。"}
    </p>
  </div>
);

export default Placeholder;
