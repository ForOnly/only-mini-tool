import { createApp } from "vue";

import App from "./App.vue";
import { i18n } from "./i18n";
import { useAppearance } from "./composables/useAppearance";
import { useDebug } from "./composables/useDebug";
import { installDesktopGuards } from "./utils/desktopGuards";
import "./styles/tokens.css";
import "./styles/base.css";

async function bootstrap() {
  const { refreshDebug } = useDebug();
  const { refresh: refreshAppearance } = useAppearance();
  try {
    await refreshAppearance();
  } catch {
    /* 后端未就绪时保留 CSS media 回退 */
  }
  try {
    await refreshDebug();
  } catch {
    /* 后端未就绪时保持 debug=false */
  }
  installDesktopGuards();
  createApp(App).use(i18n).mount("#app");
}

void bootstrap();
