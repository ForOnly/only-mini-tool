import { ref } from "vue";

import type { ScriptRunResult } from "@/api/types";
import { cancelScriptRun, runScript, type ScriptId } from "@/api/scripts";

const running = ref(false);
const lastResult = ref<ScriptRunResult | null>(null);

export function useScriptRun() {
  async function run(scriptId: ScriptId, params: Record<string, string>) {
    running.value = true;
    lastResult.value = null;
    try {
      lastResult.value = await runScript({
        scriptId: scriptId as unknown as bigint,
        params,
      });
      return lastResult.value;
    } finally {
      running.value = false;
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
    lastResult.value = null;
  }

  return {
    running,
    lastResult,
    run,
    cancel,
    clearLog,
    resetSession,
  };
}
