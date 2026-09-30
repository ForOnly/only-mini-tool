/** Monaco 懒加载单例（从 ScriptEditor.vue 迁出泛化）：
 *  裁剪入口 editor.api + editor.all 贡献集（suggest/format/右键/find 的全功能
 *  依赖 editor.all，见 commit 315594f）；语言贡献经 ensureLanguage 按需注册，
 *  新语言只需在 languageContributions 加一行。 */

import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";

export type MonacoModule = typeof import("monaco-editor/esm/vs/editor/editor.api");

// worker 与环境必须先于 monaco.create 就绪（静态赋值，不依赖 monaco 命名空间）
self.MonacoEnvironment = {
  getWorker() {
    return new editorWorker();
  },
};

let monaco: MonacoModule | null = null;
let loading: Promise<MonacoModule> | null = null;

/** 已加载的 monaco（未就绪为 null——主题/provider 等全局操作需判空）。 */
export function getMonaco(): MonacoModule | null {
  return monaco;
}

export async function loadMonaco(): Promise<MonacoModule> {
  if (monaco) return monaco;
  if (loading) return loading;
  loading = (async () => {
    const [m] = await Promise.all([
      import("monaco-editor/esm/vs/editor/editor.api"),
      // 纯副作用：注册 editor 全功能贡献集
      import("monaco-editor/esm/vs/editor/editor.all.js"),
    ]);
    monaco = m as MonacoModule;
    return monaco;
  })();
  try {
    return await loading;
  } catch (err) {
    loading = null; // 失败允许重试（不留死缓存）
    throw err;
  }
}

/** 语言贡献按需注册（幂等）。basic-languages 之外的内建语言交由 monaco 自身。 */
const languageContributions: Record<string, () => Promise<unknown>> = {
  python: () => import("monaco-editor/esm/vs/basic-languages/python/python.contribution"),
};
const loadedLanguages = new Set<string>();

export async function ensureLanguage(language: string): Promise<void> {
  if (loadedLanguages.has(language)) return;
  const loader = languageContributions[language];
  if (!loader) {
    // 未登记语言会退化为 plaintext 无高亮——显式告警而非静默
    if (language && language !== "plaintext") {
      console.warn(`[monaco] no contribution registered for language "${language}"`);
    }
    return;
  }
  await loader();
  loadedLanguages.add(language);
}
