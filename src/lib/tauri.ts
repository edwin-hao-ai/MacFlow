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

export type ProcessKind = "residual" | "duplicate" | "idle" | "hog" | "dev" | "system" | "foreground";

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

export async function killProcesses(pids: number[]): Promise<{ killed: number[]; failed: number[] }> {
  return invoke("kill_processes", { pids });
}
