/** 子窗口分发：?win=<kind>&<params> → 裸根组件（不经 AppShell）。
 *  editor 分支在 P6 接入。 */

import { markRaw } from "vue";
import type { Component } from "vue";

import EditorWindowApp from "./EditorWindowApp.vue";
import TerminalWindowApp from "./TerminalWindowApp.vue";

export interface ChildWindowSpec {
  title: string;
  component: Component;
  props: Record<string, unknown>;
}

export function resolveChildWindow(query: URLSearchParams): ChildWindowSpec | null {
  const kind = query.get("win");
  if (!kind) return null;
  if (kind === "terminal") {
    const sessionIds = (query.get("sessionIds") ?? "")
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    const titles = (query.get("titles") ?? "")
      .split(",")
      .filter(Boolean);
    return {
      title: "Terminal",
      component: markRaw(TerminalWindowApp),
      props: { sessionIds, titles },
    };
  }
  if (kind === "editor") {
    const scriptId = Number(query.get("scriptId") ?? "");
    if (!Number.isFinite(scriptId) || scriptId <= 0) {
      console.warn("[childWindow] editor window missing scriptId");
      return null;
    }
    return {
      title: "Editor",
      component: markRaw(EditorWindowApp),
      props: { scriptId },
    };
  }
  console.warn("[childWindow] unknown window kind:", kind);
  return null;
}
