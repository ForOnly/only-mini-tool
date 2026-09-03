/** OCR 工具全局 settings 键。引擎字段键由后端 EngineSpec 拼装。 */
export const OCR_ACTIVE_ENGINE = "ocr.active_engine" as const;
export const OCR_TIMEOUT_MS = "ocr.timeout_ms" as const;
export const OCR_INSPECTOR_PLACEMENT = "ocr.inspector_placement" as const;

export const DEFAULT_OCR_TIMEOUT_MS = "60000";
export const DEFAULT_OCR_ACTIVE_ENGINE = "baidu";
export const DEFAULT_OCR_INSPECTOR_PLACEMENT = "right" as const;

export type OcrInspectorPlacement = "left" | "right" | "bottom";

export function normalizeInspectorPlacement(raw: string | undefined | null): OcrInspectorPlacement {
  const v = (raw ?? "").trim();
  if (v === "left" || v === "right" || v === "bottom") {
    return v;
  }
  return DEFAULT_OCR_INSPECTOR_PLACEMENT;
}
