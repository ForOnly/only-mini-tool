import { ref } from "vue";

import type { ScriptRunResult } from "@/api/types";
import { cancelScriptRun, runScript, type ScriptId } from "@/api/scripts";

const running = ref(false);
const startedAt = ref<number | null>(null);
const lastResult = ref<ScriptRunResult | null>(null);

export function useScriptRun() {
  async function run(scriptId: ScriptId, params: Record<string, string>) {
    running.value = true;
    startedAt.value = Date.now();
    lastResult.value = null;
    try {
      lastResult.value = await runScript({
        scriptId: scriptId as unknown as bigint,
        params,
      });
      return lastResult.value;
    } finally {
      running.value = false;
      startedAt.value = null;
    }
  }

  async function cancel() {
    await cancelScriptRun();
  }

  function clearLog() {
    lastResult.value = null;
  }

  async function resetSession() {
    if (running.value) {
      try {
        await cancelScriptRun();
      } catch {
        /* ignore */
      }
    }
    running.value = false;
    startedAt.value = null;
    lastResult.value = null;
  }

  return {
    running,
    startedAt,
    lastResult,
    run,
    cancel,
    clearLog,
    resetSession,
  };
}
