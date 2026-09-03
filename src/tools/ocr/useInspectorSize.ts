import { ref } from "vue";

/** localStorage：结果面板尺寸（像素）。 */
export const LS_INSPECTOR_WIDTH = "ocr.inspector.size.width";
export const LS_INSPECTOR_HEIGHT = "ocr.inspector.size.height";

export const INSPECTOR_WIDTH_MIN = 240;
export const INSPECTOR_WIDTH_MAX = 560;
export const INSPECTOR_WIDTH_DEFAULT = 320;

export const INSPECTOR_HEIGHT_MIN = 160;
/** 相对工作区高度的上限比例。 */
export const INSPECTOR_HEIGHT_MAX_RATIO = 0.7;
export const INSPECTOR_HEIGHT_DEFAULT_RATIO = 0.4;

function readStoredPx(key: string): number | null {
  try {
    const raw = localStorage.getItem(key);
    if (raw == null || raw === "") {
      return null;
    }
    const n = Number(raw);
    return Number.isFinite(n) ? n : null;
  } catch {
    return null;
  }
}

function writeStoredPx(key: string, px: number) {
  try {
    localStorage.setItem(key, String(Math.round(px)));
  } catch {
    /* ignore quota / private mode */
  }
}

function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

const storedW = readStoredPx(LS_INSPECTOR_WIDTH);
const widthPx = ref(
  clamp(storedW ?? INSPECTOR_WIDTH_DEFAULT, INSPECTOR_WIDTH_MIN, INSPECTOR_WIDTH_MAX),
);

const storedH = readStoredPx(LS_INSPECTOR_HEIGHT);
const heightPx = ref(storedH != null && storedH >= INSPECTOR_HEIGHT_MIN ? storedH : null);

export function useInspectorSize() {
  function setWidth(px: number) {
    const next = clamp(Math.round(px), INSPECTOR_WIDTH_MIN, INSPECTOR_WIDTH_MAX);
    widthPx.value = next;
    writeStoredPx(LS_INSPECTOR_WIDTH, next);
  }

  /** workspaceH：工作区高度，用于钳制最大高度。 */
  function setHeight(px: number, workspaceH?: number) {
    const max =
      workspaceH != null && workspaceH > 0
        ? Math.max(INSPECTOR_HEIGHT_MIN, Math.floor(workspaceH * INSPECTOR_HEIGHT_MAX_RATIO))
        : Number.POSITIVE_INFINITY;
    const next = clamp(Math.round(px), INSPECTOR_HEIGHT_MIN, max);
    heightPx.value = next;
    writeStoredPx(LS_INSPECTOR_HEIGHT, next);
  }

  /** 无存储时按工作区高度给默认高度。 */
  function ensureHeight(workspaceH: number) {
    if (heightPx.value != null) {
      setHeight(heightPx.value, workspaceH);
      return;
    }
    if (workspaceH <= 0) {
      return;
    }
    setHeight(workspaceH * INSPECTOR_HEIGHT_DEFAULT_RATIO, workspaceH);
  }

  return {
    widthPx,
    heightPx,
    setWidth,
    setHeight,
    ensureHeight,
  };
}
