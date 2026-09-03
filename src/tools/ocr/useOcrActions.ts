import { computed } from "vue";
import { useI18n } from "vue-i18n";

import type { ContextMenuItem } from "@/components/common/contextMenuTypes";
import { useOcr } from "@/tools/ocr/useOcr";

export type OcrActionId = "open" | "paste" | "toggleBoxes" | "rotate" | "clear" | "retry";

/** OCR 画布右键 / ⋯ 共用动作表。 */
export function useOcrActions() {
  const { t } = useI18n();
  const {
    busy,
    hasImage,
    boxesVisible,
    lastError,
    openFile,
    pasteImage,
    toggleBoxes,
    rotateClockwise,
    clear,
    runRecognize,
  } = useOcr();

  const items = computed<ContextMenuItem[]>(() => {
    const list: ContextMenuItem[] = [
      { id: "open", label: t("ocr.open"), disabled: busy.value },
      { id: "paste", label: t("ocr.paste"), disabled: busy.value },
      {
        id: "toggleBoxes",
        label: boxesVisible.value ? t("ocr.hideBoxes") : t("ocr.showBoxes"),
        disabled: !hasImage.value,
      },
      {
        id: "rotate",
        label: t("ocr.rotate"),
        disabled: busy.value || !hasImage.value,
      },
      {
        id: "clear",
        label: t("ocr.clear"),
        disabled: !hasImage.value && !busy.value,
        danger: true,
      },
    ];
    if (lastError.value && hasImage.value) {
      list.push({
        id: "retry",
        label: t("ocr.retry"),
        disabled: busy.value,
      });
    }
    return list;
  });

  async function run(id: string): Promise<void> {
    switch (id as OcrActionId) {
      case "open":
        await openFile();
        break;
      case "paste":
        await pasteImage();
        break;
      case "toggleBoxes":
        toggleBoxes();
        break;
      case "rotate":
        await rotateClockwise();
        break;
      case "clear":
        await clear();
        break;
      case "retry":
        await runRecognize();
        break;
      default:
        break;
    }
  }

  return { items, run };
}
