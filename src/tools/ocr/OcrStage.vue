<script setup lang="ts">
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { onActivated, onDeactivated, onMounted, onUnmounted, watch } from "vue";
import { useI18n } from "vue-i18n";

import { useMessage } from "@/composables/useMessage";
import { useWorkbench } from "@/composables/useWorkbench";
import { extractClipboardImageBlob } from "@/tools/ocr/copyText";
import { useOcr } from "@/tools/ocr/useOcr";
import OcrWorkspace from "@/tools/ocr/OcrWorkspace.vue";

defineOptions({ name: "OcrStage" });

const OCR_ERROR_KEY = "ocr.lastError";

const { t } = useI18n();
const { mainView } = useWorkbench();
const { pasteImage, pasteFromBlob, openDroppedPaths, lastError, busy, result } = useOcr();
const { success, error, dismissByKey } = useMessage();

let unlistenDrop: (() => void) | undefined;
let listenersBound = false;
let bindGen = 0;

function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}

function onPaste(event: ClipboardEvent) {
  if (mainView.value !== "tool") {
    return;
  }
  if (isEditableTarget(event.target)) {
    return;
  }
  event.preventDefault();
  const blob = extractClipboardImageBlob(event.clipboardData);
  if (blob) {
    void pasteFromBlob(blob);
    return;
  }
  void pasteImage();
}

async function bindListeners() {
  if (listenersBound) {
    return;
  }
  listenersBound = true;
  const gen = ++bindGen;
  window.addEventListener("paste", onPaste, true);
  try {
    const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      if (mainView.value !== "tool") {
        return;
      }
      if (event.payload.type === "drop") {
        void openDroppedPaths(event.payload.paths);
      }
    });
    if (gen !== bindGen) {
      unlisten();
      return;
    }
    unlistenDrop = unlisten;
  } catch {
    /* web / 无 drop API 时忽略 */
  }
}

function unbindListeners() {
  if (!listenersBound) {
    return;
  }
  listenersBound = false;
  bindGen += 1;
  window.removeEventListener("paste", onPaste, true);
  unlistenDrop?.();
  unlistenDrop = undefined;
}

onMounted(() => {
  void bindListeners();
});
onActivated(() => {
  void bindListeners();
});
onUnmounted(unbindListeners);
onDeactivated(unbindListeners);

watch(lastError, (msg) => {
  if (msg) {
    error(msg, { key: OCR_ERROR_KEY });
  } else {
    dismissByKey(OCR_ERROR_KEY);
  }
});

watch(result, (value) => {
  if (value && !busy.value && !lastError.value) {
    success(t("ocr.success"));
  }
});
</script>

<template>
  <section class="ocr-stage">
    <OcrWorkspace />
  </section>
</template>

<style scoped>
.ocr-stage {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
</style>
