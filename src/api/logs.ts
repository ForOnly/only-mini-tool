import { invoke } from "@tauri-apps/api/core";

export function readAppLog(maxBytes?: number): Promise<string> {
  return invoke<string>("read_app_log", { maxBytes: maxBytes ?? null });
}

export function clearAppLog(): Promise<void> {
  return invoke("clear_app_log");
}

export function getLogsDir(): Promise<string> {
  return invoke<string>("get_logs_dir");
}

export function openLogsDir(): Promise<void> {
  return invoke("open_logs_dir");
}
