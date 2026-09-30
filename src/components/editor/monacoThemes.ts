/** Monaco 主题（从 ScriptEditor.vue 迁出）：only-light / only-dark + 外观跟随。 */

import { watch } from "vue";

import type { ColorScheme } from "@/api/types";
import { useAppearance } from "@/composables/useAppearance";
import { getMonaco } from "@/components/editor/monacoLoader";
import type { MonacoModule } from "@/components/editor/monacoLoader";

let themesDefined = false;

/** 定义主题（幂等；须在 loadMonaco 之后调用）。 */
export function defineOnlyThemes(m: MonacoModule) {
  if (themesDefined) return;
  themesDefined = true;
  m.editor.defineTheme("only-light", {
    base: "vs",
    inherit: true,
    rules: [],
    colors: { "editor.background": "#ffffff" },
  });
  m.editor.defineTheme("only-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: { "editor.background": "#1e293b" }, // 对齐 --surface
  });
}

export function themeNameFor(scheme: ColorScheme): string {
  return scheme === "dark" ? "only-dark" : "only-light";
}

/** 应用全局主题（monaco 未就绪时静默跳过——加载完成后由 watch immediate 补）。 */
export function applyMonacoTheme(scheme: ColorScheme) {
  getMonaco()?.editor.setTheme(themeNameFor(scheme));
}

/** 外观跟随（组件 setup 作用域调用；返回取消函数）。
 *  monaco 主题为全局态，多编辑器实例各自订阅同一结果，幂等无害。 */
export function watchMonacoTheme(): () => void {
  const { appearance } = useAppearance();
  const stop = watch(
    () => appearance.value?.resolved,
    (scheme) => {
      if (scheme) applyMonacoTheme(scheme);
    },
    { immediate: true },
  );
  return stop;
}
