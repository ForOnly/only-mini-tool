/** 子窗口平台层：同 bundle 查询参数分发（无 router 方案）。
 *  URL = `?win=<kind>&<params>`；label 已存在则聚焦（同脚本/同会话不重复开窗）。 */

import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

export type ChildWindowKind = "terminal" | "editor";

export interface OpenChildWindowOptions {
  kind: ChildWindowKind;
  /** 窗口标签（全局唯一；建议含业务 id 实现同对象聚焦） */
  label: string;
  title: string;
  params: Record<string, string>;
  width?: number;
  height?: number;
}

export async function openChildWindow(opts: OpenChildWindowOptions): Promise<WebviewWindow> {
  const existing = await WebviewWindow.getByLabel(opts.label);
  if (existing) {
    await existing.setFocus();
    return existing;
  }
  const qs = new URLSearchParams({ win: opts.kind, ...opts.params });
  // 相对当前窗口地址解析——dev（devUrl）与 prod（tauri://localhost）两态一致
  const url = new URL(`?${qs.toString()}`, window.location.href).href;
  const win = new WebviewWindow(opts.label, {
    url,
    title: opts.title,
    width: opts.width ?? 760,
    height: opts.height ?? 480,
    minWidth: 480,
    minHeight: 320,
    decorations: false,
    shadow: true,
    center: true,
  });
  win.once("tauri://error", (event) => {
    console.error("[childWindow] create failed:", event.payload);
  });
  return win;
}
