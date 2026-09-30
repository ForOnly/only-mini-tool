<script setup lang="ts">
/** 编辑页底栏：拖高手柄 + 标签头（运行日志 | 终端）+ 内容区。
 *  拖高行为与旧 ScriptLogPanel 完全一致（key/范围/键盘/持久化原样上移）；
 *  终端首激活后保持挂载（v-show 切换不销毁 xterm 会话）。 */

import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import ScriptLogPanel from "@/tools/scripts/ScriptLogPanel.vue";
import ScriptTerminalPane from "@/tools/scripts/ScriptTerminalPane.vue";
import { usePanelResize } from "@/tools/scripts/usePanelResize";
import type { ScriptRunResult } from "@/api/types";

defineProps<{
  result: ScriptRunResult | null;
  running: boolean;
  /** 编辑页状态文字（未保存 · 计时器）——头部唯一持久状态位 */
  statusText?: string;
  statusDirty?: boolean;
  scriptId: number;
  /** 已保存的 venv 绑定（变化 → 终端换环境） */
  venvBinding: string;
}>();

const { t } = useI18n();

const STORAGE_KEY = "scripts.logPanelHeight";
const MIN_HEIGHT = 96;
const MAX_HEIGHT = 420;
/** 编辑区（Monaco + RunBar）的最小保护高度——拖拽/窗口收缩均不可侵占 */
const MIN_EDITOR_AREA = 200;
const TITLEBAR_FALLBACK = 32;

const rootEl = ref<HTMLElement | null>(null);
type Tab = "log" | "terminal";
const activeTab = ref<Tab>("log");
/** 终端首激活后才挂载（避免无谓的 PTY spawn） */
const terminalMounted = ref(false);

watch(activeTab, (tab) => {
  if (tab === "terminal") terminalMounted.value = true;
});

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
</script>

<template>
  <aside
    ref="rootEl"
    class="bottom-panel"
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
      <div class="tabs" role="tablist">
        <button
          type="button"
          role="tab"
          class="tab"
          :class="{ active: activeTab === 'log' }"
          :aria-selected="activeTab === 'log'"
          @click="activeTab = 'log'"
        >
          {{ t("scripts.logTitle") }}
        </button>
        <button
          type="button"
          role="tab"
          class="tab"
          :class="{ active: activeTab === 'terminal' }"
          :aria-selected="activeTab === 'terminal'"
          @click="activeTab = 'terminal'"
        >
          {{ t("terminal.tabTitle") }}
        </button>
      </div>
      <span
        v-if="statusText"
        class="status"
        :class="{ dirty: statusDirty }"
      >{{ statusText }}</span>
    </header>
    <div class="content">
      <ScriptLogPanel v-show="activeTab === 'log'" :result="result" :running="running" />
      <ScriptTerminalPane
        v-if="terminalMounted"
        v-show="activeTab === 'terminal'"
        :script-id="scriptId"
        :venv-binding="venvBinding"
      />
    </div>
  </aside>
</template>

<style scoped>
.bottom-panel {
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
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--border);
  min-height: 32px;
}

.tabs {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.tab {
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--text-sm);
  font-weight: 600;
  padding: 5px var(--space-2);
  cursor: pointer;
}

.tab:hover {
  color: var(--text);
}

.tab.active {
  color: var(--text);
  border-bottom-color: var(--accent);
}

/* 宽防计时器位数增长回流；dirty 警示色 */
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

.content {
  flex: 1;
  min-height: 0;
  display: flex;
}

.content > * {
  flex: 1;
  min-width: 0;
}
</style>
