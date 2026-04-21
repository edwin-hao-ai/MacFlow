export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function fmtDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

export function fmtRelativeTime(iso: string): string {
  const d = new Date(iso);
  const diff = Date.now() - d.getTime();
  const min = Math.floor(diff / 60000);
  if (min < 1) return "刚刚";
  if (min < 60) return `${min} 分钟前`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h} 小时前`;
  const day = Math.floor(h / 24);
  if (day < 7) return `${day} 天前`;
  return d.toLocaleDateString("zh-CN");
}

export const CATEGORY_LABELS: Record<string, string> = {
  npm: "NPM",
  pnpm: "PNPM",
  yarn: "Yarn",
  docker: "Docker",
  homebrew: "Homebrew",
  xcode: "Xcode",
  cocoapods: "CocoaPods",
  cargo: "Cargo",
  pip: "Pip",
  go: "Go",
  system: "系统",
};

export const CATEGORY_COLORS: Record<string, string> = {
  npm: "bg-red-500/15 text-red-600",
  pnpm: "bg-yellow-500/15 text-yellow-600",
  yarn: "bg-blue-500/15 text-blue-600",
  docker: "bg-sky-500/15 text-sky-600",
  homebrew: "bg-amber-500/15 text-amber-600",
  xcode: "bg-indigo-500/15 text-indigo-600",
  cocoapods: "bg-pink-500/15 text-pink-600",
  cargo: "bg-orange-500/15 text-orange-600",
  pip: "bg-green-500/15 text-green-600",
  go: "bg-cyan-500/15 text-cyan-600",
  system: "bg-zinc-500/15 text-zinc-600",
};
