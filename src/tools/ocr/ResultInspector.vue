<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppPanel from "@/components/common/AppPanel.vue";
import { useMessage } from "@/composables/useMessage";
import { useInspectorSize } from "@/tools/ocr/useInspectorSize";
import { useOcr } from "@/tools/ocr/useOcr";
import { useOcrSettings } from "@/tools/ocr/useOcrSettings";

defineOptions({ name: "ResultInspector" });

const emit = defineEmits<{
  collapsedChange: [collapsed: boolean];
}>();

const { t } = useI18n();
const { success, error } = useMessage();
const {
  words,
  fullText,
  highlightIndex,
  setHighlight,
  hasImage,
  busy,
  lastError,
  runRecognize,
  cancelInFlight,
  revealSeq,
  revealIndex,
  copyLog,
  copyText: copyAndLog,
} = useOcr();
const { inspectorPlacement } = useOcrSettings();
const { widthPx, heightPx, setWidth, setHeight } = useInspectorSize();

const open = ref(true);
/** 词条 / 全文 / 复制日志 */
const resultTab = ref<"full" | "words" | "log">("words");
const listEl = ref<HTMLElement | null>(null);
const rootEl = ref<HTMLElement | null>(null);

const statusText = computed(() => {
  if (busy.value) {
    return t("ocr.statusBusy");
  }
  if (lastError.value) {
    return t("ocr.statusError");
  }
  if (hasImage.value) {
    return t("ocr.statusReady");
  }
  return t("ocr.statusIdle");
});

const emptyHint = computed(() => {
  if (!hasImage.value) {
    return t("ocr.inspectorEmpty");
  }
  if (busy.value) {
    return t("ocr.inspectorBusy");
  }
  if (lastError.value) {
    return t("ocr.inspectorFailed");
  }
  return t("ocr.inspectorNoWords");
});

const showRetry = computed(() => !!lastError.value && hasImage.value && !busy.value);
const showCancel = computed(() => busy.value);

const expandGlyph = computed(() => {
  switch (inspectorPlacement.value) {
    case "left":
      return "›";
    case "bottom":
      return "˄";
    default:
      return "‹";
  }
});

const resizeCursor = computed(() =>
  inspectorPlacement.value === "bottom" ? "row-resize" : "col-resize",
);

function formatLogTime(at: number): string {
  try {
    return new Date(at).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return "";
  }
}

function toggle() {
  open.value = !open.value;
}

watch(
  open,
  (isOpen) => {
    emit("collapsedChange", !isOpen);
  },
  { immediate: true },
);

async function copyText(text: string) {
  if (!text) {
    return;
  }
  try {
    await copyAndLog(text);
    success(t("ocr.copied"));
  } catch {
    error(t("ocr.copyFailed"));
  }
}

watch(revealSeq, async () => {
  if (!open.value) {
    return;
  }
  const index = revealIndex.value;
  if (index === null) {
    return;
  }
  if (resultTab.value !== "words") {
    resultTab.value = "words";
  }
  await nextTick();
  const root = listEl.value;
  if (!root) {
    return;
  }
  const item = root.querySelector<HTMLElement>(`[data-word-index="${index}"]`);
  item?.scrollIntoView({ block: "nearest", behavior: "smooth" });
});

function workspaceHeight(): number {
  const ws = rootEl.value?.closest(".workspace") as HTMLElement | null;
  return ws?.clientHeight ?? 0;
}

function onResizePointerDown(event: PointerEvent) {
  if (!open.value || event.button !== 0) {
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  const target = event.currentTarget as HTMLElement;
  const placement = inspectorPlacement.value;
  const startX = event.clientX;
  const startY = event.clientY;
  const startW = widthPx.value;
  const startH = heightPx.value ?? Math.round(workspaceHeight() * 0.4);

  target.setPointerCapture(event.pointerId);

  const onMove = (e: PointerEvent) => {
    if (placement === "bottom") {
      setHeight(startH + (startY - e.clientY), workspaceHeight());
      return;
    }
    if (placement === "left") {
      setWidth(startW + (e.clientX - startX));
      return;
    }
    setWidth(startW + (startX - e.clientX));
  };

  const onUp = (e: PointerEvent) => {
    target.releasePointerCapture(e.pointerId);
    target.removeEventListener("pointermove", onMove);
    target.removeEventListener("pointerup", onUp);
    target.removeEventListener("pointercancel", onUp);
  };

  target.addEventListener("pointermove", onMove);
  target.addEventListener("pointerup", onUp);
  target.addEventListener("pointercancel", onUp);
}
</script>

<template>
  <aside
    ref="rootEl"
    class="inspector"
    :class="[
      `dock-${inspectorPlacement}`,
      { collapsed: !open },
    ]"
  >
    <button
      v-if="!open"
      type="button"
      class="handle"
      :title="t('ocr.inspectorExpand')"
      :aria-label="t('ocr.inspectorExpand')"
      @click="toggle"
    >
      {{ expandGlyph }}
    </button>

    <div
      v-if="open"
      class="resize-handle"
      :class="`edge-${inspectorPlacement}`"
      :style="{ cursor: resizeCursor }"
      role="separator"
      :aria-orientation="inspectorPlacement === 'bottom' ? 'horizontal' : 'vertical'"
      :aria-label="t('ocr.inspectorResize')"
      @pointerdown="onResizePointerDown"
    />

    <AppPanel v-show="open" class="panel-fill">
      <template #header>
        <div class="head-row">
          <div class="head-title">
            <h3>{{ t("ocr.inspectorTitle") }}</h3>
            <span
              class="status"
              :class="{
                error: !!lastError && !busy,
                busy,
                ready: hasImage && !busy && !lastError,
              }"
            >
              <span v-if="busy" class="status-spinner" aria-hidden="true" />
              {{ statusText }}
            </span>
          </div>
          <div class="head-actions">
            <AppButton v-if="showCancel" variant="ghost" @click="cancelInFlight">
              {{ t("ocr.cancel") }}
            </AppButton>
            <AppButton v-if="showRetry" variant="ghost" @click="runRecognize">
              {{ t("ocr.retry") }}
            </AppButton>
            <AppButton variant="ghost" :disabled="!fullText" @click="copyText(fullText)">
              {{ t("ocr.copyAll") }}
            </AppButton>
            <AppButton variant="ghost" @click="toggle">
              {{ t("ocr.inspectorCollapse") }}
            </AppButton>
          </div>
        </div>
      </template>

      <div class="content">
        <div class="tabs" role="tablist">
          <button
            type="button"
            class="tab"
            role="tab"
            :aria-selected="resultTab === 'words'"
            :class="{ active: resultTab === 'words' }"
            @click="resultTab = 'words'"
          >
            {{ t("ocr.wordList") }}
          </button>
          <button
            type="button"
            class="tab"
            role="tab"
            :aria-selected="resultTab === 'full'"
            :class="{ active: resultTab === 'full' }"
            @click="resultTab = 'full'"
          >
            {{ t("ocr.fullText") }}
          </button>
          <button
            type="button"
            class="tab"
            role="tab"
            :aria-selected="resultTab === 'log'"
            :class="{ active: resultTab === 'log' }"
            @click="resultTab = 'log'"
          >
            {{ t("ocr.copyLog") }}
          </button>
        </div>

        <template v-if="resultTab === 'log'">
          <section class="list" role="tabpanel">
            <p v-if="copyLog.length === 0" class="empty">{{ t("ocr.copyLogEmpty") }}</p>
            <ul v-else>
              <li v-for="entry in copyLog" :key="entry.id">
                <div class="log-main">
                  <span class="log-time">{{ formatLogTime(entry.at) }}</span>
                  <button type="button" class="word" @dblclick="copyText(entry.text)">
                    {{ entry.text }}
                  </button>
                </div>
                <AppButton variant="ghost" @click="copyText(entry.text)">
                  {{ t("ocr.copy") }}
                </AppButton>
              </li>
            </ul>
          </section>
        </template>
        <template v-else-if="!hasImage || words.length === 0">
          <p class="empty">{{ emptyHint }}</p>
        </template>
        <template v-else>
          <section v-if="resultTab === 'full'" class="full" role="tabpanel">
            <pre class="full-text">{{ fullText }}</pre>
          </section>
          <section v-else class="list" role="tabpanel">
            <ul ref="listEl">
              <li
                v-for="(word, index) in words"
                :key="index"
                :data-word-index="index"
                :class="{ active: highlightIndex === index }"
                @mouseenter="setHighlight(index)"
                @mouseleave="setHighlight(null)"
              >
                <button type="button" class="word" @dblclick="copyText(word.text)">
                  {{ word.text }}
                </button>
                <AppButton variant="ghost" @click="copyText(word.text)">
                  {{ t("ocr.copy") }}
                </AppButton>
              </li>
            </ul>
          </section>
        </template>
      </div>
    </AppPanel>
  </aside>
</template>

<style scoped>
.inspector {
  flex-shrink: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
  background: var(--surface);
}

.inspector.dock-right,
.inspector.dock-left {
  width: 100%;
  height: 100%;
}

.inspector.dock-right {
  border-left: 1px solid var(--border);
}

.inspector.dock-left {
  border-right: 1px solid var(--border);
}

.inspector.dock-bottom {
  width: 100%;
  height: 100%;
  border-top: 1px solid var(--border);
}

.inspector.collapsed {
  width: auto;
  height: auto;
  min-height: 0;
  background: transparent;
  border: none;
  overflow: visible;
}

.inspector.dock-bottom.collapsed {
  width: auto;
}

.resize-handle {
  position: absolute;
  z-index: 3;
  background: transparent;
  touch-action: none;
}

.resize-handle:hover,
.resize-handle:active {
  background: color-mix(in srgb, var(--accent) 35%, transparent);
}

.resize-handle.edge-right {
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
}

.resize-handle.edge-left {
  right: 0;
  top: 0;
  bottom: 0;
  width: 4px;
}

.resize-handle.edge-bottom {
  left: 0;
  right: 0;
  top: 0;
  height: 4px;
}

.panel-fill {
  flex: 1;
  width: 100%;
  min-height: 0;
  height: 100%;
  border: none;
  border-radius: 0;
}

.head-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  flex-wrap: wrap;
  min-width: 0;
}

.head-title {
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  min-width: 0;
  flex: 1;
}

.head-actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex-shrink: 0;
}

.head-actions :deep(.btn),
.list :deep(.btn) {
  min-height: 28px;
  padding: 0 var(--space-2);
  font-size: var(--text-sm);
}

.head-row h3 {
  margin: 0;
  font-size: var(--text-md);
  font-weight: var(--font-weight-title);
  letter-spacing: 0.01em;
  white-space: nowrap;
}

.status {
  margin: 0;
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  flex-shrink: 0;
  max-width: 50%;
  padding: 1px var(--space-2);
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--surface-2) 88%, transparent);
  font-size: var(--text-xs);
  color: var(--text-muted);
  line-height: 1.25;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status.busy {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
  background: color-mix(in srgb, var(--accent) 10%, var(--surface));
}

.status.error {
  color: var(--danger);
  border-color: color-mix(in srgb, var(--danger) 35%, var(--border));
  background: color-mix(in srgb, var(--danger) 8%, var(--surface));
}

.status.ready {
  color: var(--text);
}

.status-spinner {
  box-sizing: border-box;
  width: 10px;
  height: 10px;
  flex-shrink: 0;
  border: 2px solid color-mix(in srgb, var(--accent) 28%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: ocr-status-spin 0.7s linear infinite;
}

@keyframes ocr-status-spin {
  to {
    transform: rotate(360deg);
  }
}

.content {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-height: 0;
  height: 100%;
}

.tabs {
  display: flex;
  gap: var(--space-1);
  flex-shrink: 0;
  border-bottom: 1px solid var(--border);
}

.tab {
  appearance: none;
  margin: 0;
  padding: 4px var(--space-2);
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  background: transparent;
  color: color-mix(in srgb, var(--text-muted) 85%, transparent);
  font-size: var(--text-sm);
  font-weight: var(--font-weight-title);
  cursor: pointer;
}

.tab:hover {
  color: var(--text);
}

.tab.active {
  color: var(--accent);
  border-bottom-color: var(--accent);
}

.tab:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.empty {
  margin: auto;
  padding: var(--space-3) var(--space-4);
  color: var(--text-muted);
  font-size: var(--text-sm);
  line-height: 1.45;
  text-align: center;
  max-width: 14rem;
}

.full,
.list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-height: 0;
  flex: 1;
}

.full-text {
  margin: 0;
  flex: 1;
  min-height: 0;
  padding: var(--space-2) var(--space-3);
  border: 1px solid color-mix(in srgb, var(--border) 80%, transparent);
  border-radius: var(--radius);
  background: var(--surface-2);
  font-size: var(--text-sm);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  overflow: auto;
}

ul {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
  overflow: auto;
  min-height: 0;
  flex: 1;
}

li {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  min-height: 30px;
  padding: 2px var(--space-2);
  border-radius: var(--radius);
  border-left: 2px solid transparent;
  transition: background var(--motion-fast);
}

li:hover {
  background: color-mix(in srgb, var(--accent) 6%, var(--surface-2));
}

li.active {
  background: color-mix(in srgb, var(--accent) 10%, var(--surface-2));
  border-left-color: var(--accent);
}

.log-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.log-time {
  font-size: var(--text-xs);
  color: var(--text-muted);
  line-height: 1.2;
}

.word {
  flex: 1;
  min-width: 0;
  text-align: left;
  border: none;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  font-size: var(--text-sm);
  padding: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.handle {
  appearance: none;
  box-sizing: border-box;
  min-width: 32px;
  height: 28px;
  padding: 0 var(--space-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  color: var(--text);
  cursor: pointer;
  font-size: var(--text-md);
  line-height: 1;
  flex-shrink: 0;
}

.handle:hover {
  border-color: var(--accent);
}

.handle:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

@media (prefers-reduced-motion: reduce) {
  .status-spinner {
    animation: none;
    border-top-color: color-mix(in srgb, var(--accent) 28%, transparent);
    border-color: var(--accent);
  }
}
</style>
