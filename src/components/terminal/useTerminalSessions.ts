/** 终端会话前端登记表（模块级单例，对齐 useScripts 风格）：
 *  key（业务键，如 `script:3` / `venv:dev`）→ Rust 会话 id。
 *  会话生命周期独立于视图——unmount 只 detach，显式 dispose 才销毁（D3）。 */

import { ref } from "vue";

import { terminalCreate, terminalDispose } from "@/api/terminal";
import type { TerminalCreatePayload } from "@/api/types";

export interface TerminalSessionState {
  key: string;
  id: string;
  exited: boolean;
  exitCode: number | null;
}

const sessions = ref<TerminalSessionState[]>([]);
/** 同 key 创建并发去重（in-flight 复用同一 Promise）。 */
const pending = new Map<string, Promise<string>>();

export function useTerminalSessions() {
  async function ensureSession(
    key: string,
    configFactory: () => TerminalCreatePayload,
  ): Promise<string> {
    const existing = sessions.value.find((s) => s.key === key);
    if (existing) return existing.id;
    const inFlight = pending.get(key);
    if (inFlight) return inFlight;
    const created = (async () => {
      const info = await terminalCreate(configFactory());
      sessions.value.push({ key, id: info.id, exited: info.exited, exitCode: null });
      return info.id;
    })();
    pending.set(key, created);
    try {
      return await created;
    } finally {
      pending.delete(key);
    }
  }

  function getSession(key: string): TerminalSessionState | null {
    return sessions.value.find((s) => s.key === key) ?? null;
  }

  function getBySessionId(id: string): TerminalSessionState | null {
    return sessions.value.find((s) => s.id === id) ?? null;
  }

  /** Exit 事件回写（视图层转发；attachOnly 会话可能不在登记表内，忽略）。 */
  function markExited(id: string, code: number | null) {
    const state = getBySessionId(id);
    if (state) {
      state.exited = true;
      state.exitCode = code;
    }
  }

  /** 销毁并移出登记表（幂等）。 */
  async function disposeSession(key: string): Promise<void> {
    const state = getSession(key);
    if (!state) return;
    sessions.value = sessions.value.filter((s) => s.key !== key);
    try {
      await terminalDispose(state.id);
    } catch {
      // 会话已不存在（幂等路径）——吞掉
    }
  }

  /** 按会话 id 销毁（attachOnly 视图/子窗口用）。 */
  async function disposeById(id: string): Promise<void> {
    const state = getBySessionId(id);
    if (state) sessions.value = sessions.value.filter((s) => s.key !== state.key);
    try {
      await terminalDispose(id);
    } catch {
      // 幂等
    }
  }

  return {
    sessions,
    ensureSession,
    getSession,
    getBySessionId,
    markExited,
    disposeSession,
    disposeById,
  };
}
