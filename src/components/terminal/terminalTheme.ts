/** xterm 主题：背景/前景/光标取自 design tokens（随 data-scheme 切换），
 *  ANSI 16 色用与应用配色同族的固定色板（亮暗各一套）。 */

import type { Terminal } from "@xterm/xterm";

import type { ColorScheme } from "@/api/types";

type ITheme = NonNullable<ConstructorParameters<typeof Terminal>[0]>["theme"];

/** 读取根元素上的 CSS token（运行时取值，随主题切换刷新）。 */
function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function palette(scheme: ColorScheme): ITheme {
  if (scheme === "dark") {
    return {
      background: token("--terminal-bg"),
      foreground: token("--text"),
      cursor: token("--accent"),
      cursorAccent: token("--on-accent"),
      selectionBackground: `${token("--accent")}55`,
      black: "#1e293b",
      red: "#f87171",
      green: "#4ade80",
      yellow: "#fbbf24",
      blue: "#60a5fa",
      magenta: "#f472b6",
      cyan: "#22d3ee",
      white: "#f1f5f9",
      brightBlack: "#94a3b8",
      brightRed: "#ef4444",
      brightGreen: "#22c55e",
      brightYellow: "#f59e0b",
      brightBlue: "#3b82f6",
      brightMagenta: "#ec4899",
      brightCyan: "#06b6d4",
      brightWhite: "#ffffff",
    };
  }
  return {
    background: token("--terminal-bg"),
    foreground: token("--text"),
    cursor: token("--accent"),
    cursorAccent: token("--on-accent"),
    selectionBackground: `${token("--accent")}44`,
    black: "#0f172a",
    red: "#dc2626",
    green: "#16a34a",
    yellow: "#d97706",
    blue: "#2563eb",
    magenta: "#db2777",
    cyan: "#0e7490",
    // 浅色背景：white 槽位给暗灰（CLI 常以 white 作前景色，纯白会隐形）
    white: "#475569",
    brightBlack: "#64748b",
    brightRed: "#ef4444",
    brightGreen: "#15803d",
    brightYellow: "#b45309",
    brightBlue: "#1d4ed8",
    brightMagenta: "#be185d",
    brightCyan: "#0e7490",
    brightWhite: "#0f172a",
  };
}

export function buildXtermTheme(scheme: ColorScheme): ITheme {
  return palette(scheme);
}

/** 应用主题（不重建 Terminal）。 */
export function applyTheme(term: Terminal, scheme: ColorScheme): void {
  term.options.theme = buildXtermTheme(scheme);
}
