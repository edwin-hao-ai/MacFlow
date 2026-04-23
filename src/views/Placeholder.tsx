import { Component } from "solid-js";
import { Construction } from "lucide-solid";
import { useI18n } from "@/i18n";

const Placeholder: Component<{ title: string; desc?: string }> = (props) => {
  const { t } = useI18n();
  return (
    <div class="flex flex-col items-center justify-center h-full text-center p-10">
      <div class="w-14 h-14 rounded-2xl bg-black/5 dark:bg-white/5 flex items-center justify-center mb-4">
        <Construction size={24} class="text-zinc-400" />
      </div>
      <h3 class="text-base font-semibold">{props.title}</h3>
      <p class="text-sm text-zinc-500 mt-1 max-w-xs">
        {props.desc ?? t("placeholder.buildingDesc")}
      </p>
    </div>
  );
};

export default Placeholder;
