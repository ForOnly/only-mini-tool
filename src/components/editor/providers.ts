/** Monaco provider 幂等注册框架：同 id 只注册一次（monaco 为窗口级单例，
 *  多编辑器实例共享；领域模块把 provider 注册收敛到自己的注册函数里）。 */

import { getMonaco } from "@/components/editor/monacoLoader";
import type { MonacoModule } from "@/components/editor/monacoLoader";

/** provider 运行期错误上报（如格式化失败——注册期拿不到 useMessage 上下文，
 *  由首个调用方注入）。 */
export type ReportError = (err: unknown) => void;

const registered = new Set<string>();

export function registerMonacoProviders(
  id: string,
  register: (m: MonacoModule, reportError: ReportError) => void,
  reportError: ReportError,
): void {
  if (registered.has(id)) return;
  const m = getMonaco();
  if (!m) {
    // 须在 loadMonaco 之后调用——静默跳过会让补全/格式化无声失效，极难排查
    console.warn(`[monaco] providers "${id}" registered before loadMonaco — skipped`);
    return;
  }
  registered.add(id);
  register(m, reportError);
}
