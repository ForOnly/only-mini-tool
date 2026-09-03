import { computed, ref } from "vue";

import { getOcrSettings, saveOcrSettings } from "@/api/ocr";
import type { OcrEngineInfo, OcrSettingsBundle } from "@/api/types";
import {
  DEFAULT_OCR_ACTIVE_ENGINE,
  DEFAULT_OCR_INSPECTOR_PLACEMENT,
  DEFAULT_OCR_TIMEOUT_MS,
  normalizeInspectorPlacement,
  type OcrInspectorPlacement,
} from "@/tools/ocr/settings";
import { formatAppError } from "@/utils/error";

const engines = ref<OcrEngineInfo[]>([]);
const values = ref<Record<string, string>>({});
const activeEngine = ref(DEFAULT_OCR_ACTIVE_ENGINE);
const timeoutMs = ref(DEFAULT_OCR_TIMEOUT_MS);
const inspectorPlacement = ref<OcrInspectorPlacement>(DEFAULT_OCR_INSPECTOR_PLACEMENT);
const busy = ref(false);
const errorMessage = ref<string | null>(null);

const activeEngineInfo = computed(
  () => engines.value.find((e) => e.id === activeEngine.value) ?? engines.value[0] ?? null,
);

export function useOcrSettings() {
  async function load(): Promise<void> {
    const bundle: OcrSettingsBundle = await getOcrSettings();
    engines.value = bundle.engines;
    const next: Record<string, string> = {};
    for (const [k, v] of Object.entries(bundle.values ?? {})) {
      next[k] = v ?? "";
    }
    values.value = next;
    activeEngine.value = bundle.activeEngine || DEFAULT_OCR_ACTIVE_ENGINE;
    timeoutMs.value = bundle.timeoutMs || DEFAULT_OCR_TIMEOUT_MS;
    inspectorPlacement.value = normalizeInspectorPlacement(bundle.inspectorPlacement);
    errorMessage.value = null;
  }

  async function save(): Promise<void> {
    busy.value = true;
    try {
      const info = activeEngineInfo.value;
      const fieldValues: Record<string, string> = {};
      if (info) {
        for (const field of info.fields) {
          fieldValues[field.settingKey] = (values.value[field.settingKey] ?? "").trim();
        }
      }
      const placement = normalizeInspectorPlacement(inspectorPlacement.value);
      await saveOcrSettings({
        activeEngine: activeEngine.value || DEFAULT_OCR_ACTIVE_ENGINE,
        timeoutMs: timeoutMs.value.trim() || DEFAULT_OCR_TIMEOUT_MS,
        inspectorPlacement: placement,
        values: fieldValues,
      });
      inspectorPlacement.value = placement;
      // 合并回本地 values，便于切引擎后仍看到已保存值
      values.value = { ...values.value, ...fieldValues };
      errorMessage.value = null;
    } catch (error) {
      errorMessage.value = formatAppError(error);
      throw error;
    } finally {
      busy.value = false;
    }
  }

  function fieldLabelKey(name: string): string {
    return `settings.ocrField_${name}`;
  }

  function engineLabelKey(id: string): string {
    return `settings.ocrEngine_${id}`;
  }

  return {
    engines,
    values,
    activeEngine,
    activeEngineInfo,
    timeoutMs,
    inspectorPlacement,
    busy,
    errorMessage,
    load,
    save,
    fieldLabelKey,
    engineLabelKey,
  };
}
