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

export type KillResult = {
  pid: number;
  name: string;
  success: boolean;
  message: string;
};

export type KillReport = {
  killed: number[];
  failed: number[];
  details: KillResult[];
};

export async function killProcesses(
  pids: number[],
  names: string[],
): Promise<KillReport> {
  return invoke("kill_processes", { pids, names });
}

// ========== 进程管理视图 ==========

export type ProcessRow = {
  pid: number;
  parent_pid: number | null;
  name: string;
  exe: string;
  cpu_percent: number;
  memory_mb: number;
  uptime_secs: number;
  status: string;
  ports: number[];
  protected: boolean;
  protected_reason: string | null;
  whitelisted: boolean;
};

export async function listAllProcesses(): Promise<ProcessRow[]> {
  return invoke("list_all_processes");
}

// ========== 应用程序管理 ==========

export type AppChildProcess = {
  pid: number;
  parent_pid: number | null;
  name: string;
  memory_mb: number;
  cpu_percent: number;
  ports: number[];
  is_main: boolean;
  depth: number;
  protected: boolean;
  protected_reason: string | null;
  whitelisted: boolean;
};

export type AppInfo = {
  bundle_path: string;
  name: string;
  bundle_id: string;
  main_pid: number;
  all_pids: number[];
  children: AppChildProcess[];
  memory_mb: number;
  cpu_percent: number;
  uptime_secs: number;
  ports: number[];
  is_system: boolean;
  protected_process_count: number;
  whitelisted_process_count: number;
};

export async function listApplications(): Promise<AppInfo[]> {
  return invoke("list_applications");
}

export async function quitApplication(name: string): Promise<void> {
  return invoke("quit_application", { name });
}

export async function forceQuitApplication(
  pids: number[],
): Promise<[number, string][]> {
  return invoke("force_quit_application", { pids });
}

// ========== Docker 深度视图 ==========

export type DockerImage = {
  id: string;
  repository: string;
  tag: string;
  size_bytes: number;
  created: string;
  dangling: boolean;
  in_use: boolean;
};

export type DockerContainer = {
  id: string;
  name: string;
  image: string;
  status: string;
  running: boolean;
  size_bytes: number;
  created: string;
};

export type DockerVolume = {
  name: string;
  driver: string;
  size_bytes: number;
  in_use: boolean;
};

export type DockerBuilderCache = {
  total_bytes: number;
  reclaimable_bytes: number;
};

export type DockerInventory = {
  daemon_running: boolean;
  images: DockerImage[];
  containers: DockerContainer[];
  volumes: DockerVolume[];
  builder: DockerBuilderCache;
  reclaimable_bytes: number;
};

export async function dockerAvailable(): Promise<boolean> {
  return invoke("docker_available");
}
export async function dockerInventory(): Promise<DockerInventory> {
  return invoke("docker_inventory");
}
export async function dockerRemoveImage(id: string): Promise<void> {
  return invoke("docker_remove_image", { id });
}
export async function dockerRemoveContainer(id: string): Promise<void> {
  return invoke("docker_remove_container", { id });
}
export async function dockerRemoveVolume(name: string): Promise<void> {
  return invoke("docker_remove_volume", { name });
}
export async function dockerPruneAll(): Promise<string> {
  return invoke("docker_prune_all");
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
