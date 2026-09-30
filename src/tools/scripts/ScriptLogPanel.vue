<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import { useMessage } from "@/composables/useMessage";
import type { ScriptRunResult } from "@/api/types";
import { usePanelResize } from "@/tools/scripts/usePanelResize";

const props = defineProps<{
  result: ScriptRunResult | null;
  running: boolean;
  /** 编辑页状态文字（未保存 · 计时器）——LogPanel 头部是唯一持久状态位 */
  statusText?: string;
  statusDirty?: boolean;
}>();

const { t } = useI18n();
const { success } = useMessage();

const STORAGE_KEY = "scripts.logPanelHeight";
const MIN_HEIGHT = 96;
const MAX_HEIGHT = 420;
/** 编辑区（Monaco + RunBar）的最小保护高度——拖拽/窗口收缩均不可侵占 */
const MIN_EDITOR_AREA = 200;
const TITLEBAR_FALLBACK = 32;

const rootEl = ref<HTMLElement | null>(null);

function parentHeight(): number {
  const parent = rootEl.value?.parentElement;
  if (parent && parent.clientHeight > 0) return parent.clientHeight;
  return Math.max(0, window.innerHeight - TITLEBAR_FALLBACK);
}

function getMin() {
  return MIN_HEIGHT;
}

function getMax() {
  return Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, parentHeight() - MIN_EDITOR_AREA));
}

const { value: height, dragging, atMax, onPointerDown, onKeydown } = usePanelResize({
  storageKey: STORAGE_KEY,
  axis: "y",
  getMin,
  getMax,
  defaultValue: () => 160,
});

const command = computed(() => props.result?.command ?? "");

const text = computed(() => {
  if (props.running) return t("scripts.runRunning");
  if (!props.result) return t("scripts.logEmpty");
  const parts = [
    props.result.cancelled
      ? t("scripts.runCancelled")
      : t("scripts.exitCode", { code: props.result.exitCode ?? "—" }),
    "",
    "—— stdout ——",
    props.result.stdout || "(empty)",
    "",
    "—— stderr ——",
    props.result.stderr || "(empty)",
  ];
  return parts.join("\n");
});

async function copyCommand() {
  if (!command.value) return;
  try {
    await navigator.clipboard.writeText(command.value);
    success(t("scripts.commandCopied"));
  } catch {
    /* 剪贴板不可用时静默 */
  }
}
</script>

<template>
  <aside
    ref="rootEl"
    class="log-panel"
    :style="{ height: `${height}px` }"
  >
    <div
      class="handle"
      :class="{ dragging, atMax }"
      role="separator"
      aria-orientation="horizontal"
      tabindex="0"
      :aria-label="t('scripts.logResize')"
      :aria-valuenow="height"
      :aria-valuemin="MIN_HEIGHT"
      :aria-valuemax="MAX_HEIGHT"
      @pointerdown="onPointerDown"
      @keydown="onKeydown"
    />
    <header class="head">
      <h3>{{ t("scripts.logTitle") }}</h3>
      <span
        v-if="statusText"
        class="status"
        :class="{ dirty: statusDirty }"
      >{{ statusText }}</span>
    </header>
    <div v-if="command" class="command-row">
      <span class="command-label">{{ t("scripts.commandLabel") }}</span>
      <code class="command-text">{{ command }}</code>
      <AppButton variant="ghost" type="button" @click="copyCommand">
        {{ t("scripts.copyCommand") }}
      </AppButton>
    </div>
    <pre class="body" data-scrollbar="thin">{{ text }}</pre>
  </aside>
</template>

<style scoped>
.log-panel {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  min-height: 96px;
  border-top: 1px solid var(--border);
  background: var(--surface);
}

.handle {
  height: 6px;
  flex-shrink: 0;
  cursor: ns-resize;
  touch-action: none;
  user-select: none;
  outline: none;
  background: color-mix(in srgb, var(--border) 70%, transparent);
}

.handle:hover,
.handle:focus-visible,
.handle.dragging {
  background: color-mix(in srgb, var(--accent) 55%, transparent);
}

/* 已到可拖上限——与 hover 区分，提示「到顶」而非失灵 */
.handle.atMax {
  background: color-mix(in srgb, var(--accent) 70%, transparent);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--border);
}

.head h3 {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: 600;
}

/* 定宽防计时器位数增长回流；dirty 警示色 */
.status {
  font-size: var(--text-sm);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  min-width: 5.5em;
  text-align: right;
}

.status.dirty {
  color: var(--warning, #c47f17);
}

.command-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  border-bottom: 1px solid var(--border);
  min-width: 0;
}

.command-label {
  font-size: var(--text-xs, 12px);
  color: var(--text-muted);
  white-space: nowrap;
}

.command-text {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.body {
  margin: 0;
  padding: var(--space-3);
  flex: 1;
  min-height: 0;
  overflow: auto;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
