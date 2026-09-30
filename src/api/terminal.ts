import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  ScriptTerminalConfig,
  TerminalCreatePayload,
  TerminalEvent,
  TerminalInfo,
} from "./types";

export function terminalCreate(payload: TerminalCreatePayload): Promise<TerminalInfo> {
  return invoke<TerminalInfo>("terminal_create", { payload });
}

/** 附着会话：Channel 首条为 Replay（scrollback 快照），返回 attachId 供卸载时 detach。 */
export function terminalAttach(id: string, onEvent: Channel<TerminalEvent>): Promise<string> {
  return invoke<string>("terminal_attach", { id, onEvent });
}

export function terminalDetach(id: string, attachId: string): Promise<void> {
  return invoke("terminal_detach", { id, attachId });
}

export function terminalWrite(id: string, data: string): Promise<void> {
  return invoke("terminal_write", { id, data });
}

export function terminalResize(id: string, cols: number, rows: number): Promise<void> {
  return invoke("terminal_resize", { id, cols, rows });
}

/** 幂等销毁。 */
export function terminalDispose(id: string): Promise<void> {
  return invoke("terminal_dispose", { id });
}

export function terminalList(): Promise<TerminalInfo[]> {
  return invoke<TerminalInfo[]>("terminal_list");
}

/** 脚本编辑页终端预设（复用 run 的 cwd/venv 解析链）。 */
export function resolveScriptTerminal(scriptId: number): Promise<ScriptTerminalConfig> {
  return invoke<ScriptTerminalConfig>("resolve_script_terminal", { scriptId });
}
