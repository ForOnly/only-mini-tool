/** 终端会话前端登记表（模块级单例，对齐 useScripts 风格）：
 *  key（业务键，如 `script:3:dev:D:\\ws` / `venv:dev`）→ Rust 会话 id。
 *  会话生命周期独立于视图——unmount 只 detach，显式 dispose 才销毁；
 *  Exit 事件后 10s 自动回收（退出的 shell 不再常驻 conhost）。 */

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
/** Exit 后自动回收定时器（会话 id → timer）。 */
const exitTimers = new Map<string, number>();
const EXIT_CLEANUP_DELAY = 10_000;

function clearExitTimer(id: string) {
  const timer = exitTimers.get(id);
  if (timer != undefined) {
    window.clearTimeout(timer);
    exitTimers.delete(id);
  }
}

/** Exit 后自动回收：终态覆盖层的文本活在本地 xterm 缓冲，会话死后无损失；
 *  期间用户点「新建会话」会先 dispose（清本定时器），无误杀。 */
function scheduleExitCleanup(id: string, delay = EXIT_CLEANUP_DELAY) {
  if (exitTimers.has(id)) return;
  const timer = window.setTimeout(() => {
    exitTimers.delete(id);
    sessions.value = sessions.value.filter((s) => s.id !== id);
    void terminalDispose(id).catch(() => {
      // 幂等
    });
  }, delay);
  exitTimers.set(id, timer);
}

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

  /** Exit 事件回写并调度自动回收（attachOnly 会话可能不在登记表内，忽略 id 侧）。 */
  function markExited(id: string, code: number | null) {
    const state = getBySessionId(id);
    if (state) {
      state.exited = true;
      state.exitCode = code;
    }
    scheduleExitCleanup(id);
  }

  /** attach 报 not_found 时清登记表死键（跨窗 dispose 后的收敛）。 */
  function reapSession(id: string) {
    sessions.value = sessions.value.filter((s) => s.id !== id);
    clearExitTimer(id);
  }

  /** 销毁并移出登记表（幂等）。 */
  async function disposeSession(key: string): Promise<void> {
    const state = getSession(key);
    if (!state) return;
    sessions.value = sessions.value.filter((s) => s.key !== key);
    clearExitTimer(state.id);
    try {
      await terminalDispose(state.id);
    } catch {
      // 会话已不存在（幂等路径）——吞掉
    }
  }

  /** 按会话 id 销毁（attachOnly 视图/子窗口/自动回收用）。 */
  async function disposeById(id: string): Promise<void> {
    const state = getBySessionId(id);
    if (state) sessions.value = sessions.value.filter((s) => s.key !== state.key);
    clearExitTimer(id);
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
    reapSession,
    scheduleExitCleanup,
    disposeSession,
    disposeById,
  };
}
