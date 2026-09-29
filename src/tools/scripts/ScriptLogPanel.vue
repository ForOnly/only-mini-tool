<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import type { ScriptRunResult } from "@/api/types";

const props = defineProps<{
  result: ScriptRunResult | null;
  running: boolean;
}>();

const { t } = useI18n();

const STORAGE_KEY = "scripts.logPanelHeight";
const height = ref(Number(localStorage.getItem(STORAGE_KEY) || 160));
const dragging = ref(false);

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

function onPointerDown(event: PointerEvent) {
  dragging.value = true;
  (event.target as HTMLElement).setPointerCapture?.(event.pointerId);
}

function onPointerMove(event: PointerEvent) {
  if (!dragging.value) return;
  const next = Math.min(420, Math.max(96, window.innerHeight - event.clientY));
  height.value = next;
}

function onPointerUp() {
  if (!dragging.value) return;
  dragging.value = false;
  localStorage.setItem(STORAGE_KEY, String(height.value));
}

onMounted(() => {
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
});
</script>

<template>
  <aside class="log-panel" :style="{ height: `${height}px` }">
    <div class="handle" @pointerdown="onPointerDown" />
    <header class="head">
      <h3>{{ t("scripts.logTitle") }}</h3>
    </header>
    <pre class="body">{{ text }}</pre>
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
  background: color-mix(in srgb, var(--border) 70%, transparent);
}

.head {
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--border);
}

.head h3 {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: 600;
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
