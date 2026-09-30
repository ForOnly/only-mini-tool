/** 子窗口分发：?win=<kind>&<params> → 裸根组件（不经 AppShell）。
 *  editor 分支在 P6 接入。 */

import { markRaw } from "vue";
import type { Component } from "vue";

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
  console.warn("[childWindow] unknown window kind:", kind);
  return null;
}
