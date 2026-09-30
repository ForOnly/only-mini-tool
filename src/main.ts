import { createApp } from "vue";

import App from "./App.vue";
import { i18n } from "./i18n";
import { useAppearance } from "./composables/useAppearance";
import { useDebug } from "./composables/useDebug";
import { installDesktopGuards } from "./utils/desktopGuards";
import ChildWindowRoot from "./windows/ChildWindowRoot.vue";
import { resolveChildWindow } from "./windows";
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
  // 子窗口：同 bundle 查询参数分发裸根组件（终端/编辑器拖出）
  const spec = resolveChildWindow(new URLSearchParams(window.location.search));
  if (spec) {
    createApp(ChildWindowRoot, { spec }).use(i18n).mount("#app");
  } else {
    createApp(App).use(i18n).mount("#app");
  }
}

void bootstrap();
