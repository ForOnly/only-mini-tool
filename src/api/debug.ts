import { invoke } from "@tauri-apps/api/core";

export function openDevtools(): Promise<void> {
  return invoke("open_devtools");
}
