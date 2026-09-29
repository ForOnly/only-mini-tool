import type { ToolDefinition } from "@/platform/toolTypes";
import OcrSettingsSection from "@/tools/ocr/OcrSettingsSection.vue";
import OcrStage from "@/tools/ocr/OcrStage.vue";
import { ocrSession } from "@/tools/ocr/session";
import ScriptSettingsSection from "@/tools/scripts/ScriptSettingsSection.vue";
import ScriptsStage from "@/tools/scripts/ScriptsStage.vue";
import { scriptsSession } from "@/tools/scripts/session";

export type { ToolDefinition } from "@/platform/toolTypes";

/** 仅注册已上线工具；未上线不占 Launcher 位。 */
export const tools: ToolDefinition[] = [
  {
    id: "ocr",
    labelKey: "tools.ocr",
    descriptionKey: "tools.ocrDesc",
    icon: "ocr",
    order: 10,
    stage: OcrStage,
    persistOnDeactivate: true,
    settingsSection: OcrSettingsSection,
    session: ocrSession,
  },
  {
    id: "scripts",
    labelKey: "tools.scripts",
    descriptionKey: "tools.scriptsDesc",
    icon: "scripts",
    order: 20,
    stage: ScriptsStage,
    persistOnDeactivate: true,
    settingsSection: ScriptSettingsSection,
    session: scriptsSession,
  },
];
