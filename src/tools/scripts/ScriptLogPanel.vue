<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import { useMessage } from "@/composables/useMessage";
import type { ScriptRunResult } from "@/api/types";

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
const TITLEBAR = 32;
const MIN_HEIGHT = 96;
const MAX_HEIGHT = 420;
/** 编辑区（Monaco + RunBar）的最小保护高度——拖拽/窗口收缩均不可侵占 */
const MIN_EDITOR_AREA = 200;

const height = ref(clamp(Number(localStorage.getItem(STORAGE_KEY) || 160)));
const dragging = ref(false);

/** 上限随窗口高度动态收缩：小窗下拖满也不把 Monaco 压成一条线 */
function maxHeight() {
  const stage = window.innerHeight - TITLEBAR;
  return Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, stage - MIN_EDITOR_AREA));
}

function clamp(value: number) {
  return Math.min(maxHeight(), Math.max(MIN_HEIGHT, value));
}

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

function onPointerDown(event: PointerEvent) {
  dragging.value = true;
  (event.target as HTMLElement).setPointerCapture?.(event.pointerId);
}

function onPointerMove(event: PointerEvent) {
  if (!dragging.value) return;
  height.value = clamp(window.innerHeight - event.clientY);
}

function onPointerUp() {
  if (!dragging.value) return;
  dragging.value = false;
  localStorage.setItem(STORAGE_KEY, String(height.value));
}

/** 窗口收缩时重 clamp（持久化的极端高度跨会话/跨窗口尺寸不残留） */
function onWindowResize() {
  height.value = clamp(height.value);
}

onMounted(() => {
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  window.addEventListener("resize", onWindowResize);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("resize", onWindowResize);
});
</script>

<template>
  <aside class="log-panel" :style="{ height: `${height}px` }">
    <div class="handle" @pointerdown="onPointerDown" />
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
  min-height: 96px;
  border-top: 1px solid var(--border);
  background: var(--surface);
}

.handle {
  height: 6px;
  cursor: ns-resize;
  touch-action: none;
  user-select: none;
  background: color-mix(in srgb, var(--border) 70%, transparent);
}

.handle:hover {
  background: color-mix(in srgb, var(--accent) 55%, transparent);
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
