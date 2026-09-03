import { ref } from "vue";

import { getAppearance, getSetting, setSetting } from "@/api/settings";
import type { AppearanceDto, ColorScheme, UiTheme } from "@/api/types";
import { UI_THEME } from "@/settings/system/keys";
import { formatAppError } from "@/utils/error";

const appearance = ref<AppearanceDto | null>(null);
const errorMessage = ref<string | null>(null);

function applyDocumentScheme(scheme: ColorScheme) {
  document.documentElement.dataset.scheme = scheme;
}

export function useAppearance() {
  async function refresh() {
    try {
      appearance.value = await getAppearance();
      if (appearance.value) {
        applyDocumentScheme(appearance.value.resolved);
      }
      errorMessage.value = null;
    } catch (error) {
      errorMessage.value = formatAppError(error);
    }
  }

  async function setTheme(theme: UiTheme) {
    try {
      await setSetting(UI_THEME, theme);
      await refresh();
      errorMessage.value = null;
    } catch (error) {
      errorMessage.value = formatAppError(error);
    }
  }

  async function loadStoredTheme(): Promise<UiTheme> {
    const value = await getSetting(UI_THEME);
    if (value === "light" || value === "dark" || value === "system") {
      return value;
    }
    return "system";
  }

  return {
    appearance,
    errorMessage,
    refresh,
    setTheme,
    loadStoredTheme,
  };
}
