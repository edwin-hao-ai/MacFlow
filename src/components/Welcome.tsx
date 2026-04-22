import { Component } from "solid-js";
import { Sparkles, ShieldCheck, Cpu, HardDrive } from "lucide-solid";
import { useI18n } from "@/i18n";

type Props = {
  onStart: () => void;
};

const Welcome: Component<Props> = (props) => {
  const { t } = useI18n();
  return (
    <div class="flex flex-col items-center justify-center h-full text-center p-10 animate-fade-in">
      <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-brand-400 to-brand-600 flex items-center justify-center mb-5 shadow-lg shadow-brand-500/30">
        <Sparkles size={28} class="text-white" />
      </div>
      <h2 class="text-2xl font-semibold tracking-tight">
        {t("welcome.title")}
      </h2>
      <p class="text-sm text-zinc-500 mt-2 max-w-md whitespace-pre-line">
        {t("welcome.subtitle")}
      </p>

      <div class="grid grid-cols-3 gap-3 mt-8 max-w-xl">
        <Feature
          icon={Cpu}
          title={t("welcome.featureProcessTitle")}
          desc={t("welcome.featureProcessDesc")}
        />
        <Feature
          icon={HardDrive}
          title={t("welcome.featureCacheTitle")}
          desc={t("welcome.featureCacheDesc")}
        />
        <Feature
          icon={ShieldCheck}
          title={t("welcome.featureSafetyTitle")}
          desc={t("welcome.featureSafetyDesc")}
        />
      </div>

      <button
        type="button"
        class="btn-primary gap-2 mt-8 min-w-[220px]"
        onClick={props.onStart}
      >
        <Sparkles size={16} />
        {t("welcome.cta")}
      </button>

      <div class="mt-6 text-[11px] text-zinc-400">
        {t("welcome.footer")}
      </div>
    </div>
  );
};

const Feature: Component<{
  icon: Component<{ size?: number; class?: string }>;
  title: string;
  desc: string;
}> = (p) => (
  <div class="p-4 rounded-xl bg-black/[0.03] dark:bg-white/[0.03] text-left">
    <p.icon size={18} class="text-brand-600" />
    <div class="text-sm font-semibold mt-2">{p.title}</div>
    <div class="text-[11px] text-zinc-500 mt-0.5 leading-relaxed">{p.desc}</div>
  </div>
);

export default Welcome;
