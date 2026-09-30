import { invoke } from "@tauri-apps/api/core";

import type {
  ScriptCreate,
  ScriptDto,
  ScriptRunRequest,
  ScriptRunResult,
  ScriptSummary,
  ScriptUpdate,
  ScriptsSettingsBundle,
  ScriptsSettingsSave,
  ScriptVenvSummary,
} from "./types";

/** ts-rs 将 i64 标成 bigint，IPC JSON 实际为 number。 */
export type ScriptId = number;

export function listScripts(): Promise<ScriptSummary[]> {
  return invoke<ScriptSummary[]>("list_scripts");
}

export function getScript(id: ScriptId): Promise<ScriptDto> {
  return invoke<ScriptDto>("get_script", { id });
}

export function createScript(payload: ScriptCreate): Promise<ScriptDto> {
  return invoke<ScriptDto>("create_script", { payload });
}

export function updateScript(id: ScriptId, payload: ScriptUpdate): Promise<ScriptDto> {
  return invoke<ScriptDto>("update_script", { id, payload });
}

export function deleteScript(id: ScriptId): Promise<void> {
  return invoke("delete_script", { id });
}

export function renameScript(id: ScriptId, name: string): Promise<ScriptDto> {
  return invoke<ScriptDto>("rename_script", { id, name });
}

export function getScriptsSettings(): Promise<ScriptsSettingsBundle> {
  return invoke<ScriptsSettingsBundle>("get_scripts_settings");
}

export function saveScriptsSettings(payload: ScriptsSettingsSave): Promise<void> {
  return invoke("save_scripts_settings", { payload });
}

export function runScript(payload: ScriptRunRequest): Promise<ScriptRunResult> {
  return invoke<ScriptRunResult>("run_script", { payload });
}

export function cancelScriptRun(): Promise<void> {
  return invoke("cancel_script_run");
}

export function createScriptsVenv(): Promise<void> {
  return invoke("create_scripts_venv");
}

export function createScriptVenv(name: string): Promise<void> {
  return invoke("create_script_venv", { name });
}

export function listScriptVenvs(): Promise<ScriptVenvSummary[]> {
  return invoke<ScriptVenvSummary[]>("list_script_venvs");
}

export function deleteScriptVenv(name: string): Promise<void> {
  return invoke("delete_script_venv", { name });
}

export function formatScriptCode(code: string): Promise<string> {
  return invoke<string>("format_script_code", { code });
}
