import { invoke } from "@tauri-apps/api/core";

export type SystemHealth = {
  cpu_percent: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_percent: number;
  disk_used_gb: number;
  disk_total_gb: number;
  disk_percent: number;
};

export type ProcessKind = "zombie" | "idle" | "hog" | "dev" | "system" | "foreground";

export type ProcessInfo = {
  pid: number;
  name: string;
  exe: string;
  cpu_percent: number;
  memory_mb: number;
  kind: ProcessKind;
  risk: "safe" | "low" | "dev" | "hidden";
  default_select: boolean;
  reason: string;
  ports: number[];
};

export type ScanResult = {
  health: SystemHealth;
  processes: ProcessInfo[];
  scanned_at_ms: number;
};

export async function getSystemHealth(): Promise<SystemHealth> {
  return invoke("get_system_health");
}

export async function scanAll(): Promise<ScanResult> {
  return invoke("scan_all");
}

export async function killProcesses(
  pids: number[],
  names: string[],
): Promise<{ killed: number[]; failed: number[] }> {
  return invoke("kill_processes", { pids, names });
}

// ========== Cache ==========

export type CacheCategory =
  | "npm"
  | "pnpm"
  | "yarn"
  | "docker"
  | "homebrew"
  | "xcode"
  | "cocoapods"
  | "cargo"
  | "pip"
  | "go"
  | "system";

export type Safety = "safe" | "low" | "medium";

export type CacheItem = {
  id: string;
  category: CacheCategory;
  label: string;
  description: string;
  path: string | null;
  size_bytes: number;
  safety: Safety;
  default_select: boolean;
  command: string | null;
  recover_hint: string;
};

export type CacheScanResult = {
  items: CacheItem[];
  total_bytes: number;
  scanned_at_ms: number;
};

export type CleanReport = {
  id: string;
  label: string;
  success: boolean;
  freed_bytes: number;
  duration_ms: number;
  command: string | null;
  error: string | null;
};

export type CleanSummary = {
  reports: CleanReport[];
  total_freed_bytes: number;
  success_count: number;
  fail_count: number;
};

export async function scanCache(): Promise<CacheScanResult> {
  return invoke("scan_cache");
}

export async function cleanCache(items: CacheItem[]): Promise<CleanSummary> {
  return invoke("clean_cache", { items });
}

// ========== History & Whitelist ==========

export type HistoryEntry = {
  id: number;
  timestamp: string;
  operation: string;
  target: string;
  freed_bytes: number;
  success: boolean;
  detail: string;
};

export async function getHistory(limit = 200): Promise<HistoryEntry[]> {
  return invoke("get_history", { limit });
}

export type WhitelistEntry = {
  id: number;
  kind: string;
  value: string;
  added_at: string;
  note: string;
};

export async function getWhitelist(): Promise<WhitelistEntry[]> {
  return invoke("get_whitelist");
}

export async function addWhitelist(kind: string, value: string, note = ""): Promise<void> {
  return invoke("add_whitelist", { kind, value, note });
}

export async function removeWhitelist(id: number): Promise<void> {
  return invoke("remove_whitelist", { id });
}
