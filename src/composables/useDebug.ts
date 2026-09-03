import { ref } from "vue";

import { getSetting, setSetting } from "@/api/settings";
import { DEBUG_ENABLED } from "@/settings/system/keys";

const debugEnabled = ref(false);

export function useDebug() {
  async function refreshDebug(): Promise<void> {
    debugEnabled.value = (await getSetting(DEBUG_ENABLED)) === "true";
  }

  async function setDebugEnabled(enabled: boolean): Promise<void> {
    await setSetting(DEBUG_ENABLED, enabled ? "true" : "false");
    debugEnabled.value = enabled;
  }

  return {
    debugEnabled,
    refreshDebug,
    setDebugEnabled,
  };
}
