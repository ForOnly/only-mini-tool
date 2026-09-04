<script setup lang="ts">
import { computed, onActivated, onMounted, ref } from "vue";

import ImageCanvas from "@/tools/ocr/ImageCanvas.vue";
import ResultInspector from "@/tools/ocr/ResultInspector.vue";
import { useInspectorSize } from "@/tools/ocr/useInspectorSize";
import { useOcrSettings } from "@/tools/ocr/useOcrSettings";

defineOptions({ name: "OcrWorkspace" });

const { inspectorPlacement, load } = useOcrSettings();
const { widthPx, heightPx, ensureHeight } = useInspectorSize();
const inspectorCollapsed = ref(false);
const workspaceEl = ref<HTMLElement | null>(null);

function refreshPlacement() {
  void load().catch(() => {
    /* 布局回退默认 right，已在 normalize 中处理 */
  });
}

function syncHeightDefault() {
  const h = workspaceEl.value?.clientHeight ?? 0;
  if (h > 0) {
    ensureHeight(h);
  }
}

function onCollapsedChange(collapsed: boolean) {
  inspectorCollapsed.value = collapsed;
}

const slotStyle = computed(() => {
  if (inspectorCollapsed.value) {
    return undefined;
  }
  if (inspectorPlacement.value === "bottom") {
    return heightPx.value != null ? { height: `${heightPx.value}px` } : undefined;
  }
  return { width: `${widthPx.value}px` };
});

onMounted(() => {
  refreshPlacement();
  syncHeightDefault();
});
onActivated(() => {
  refreshPlacement();
  syncHeightDefault();
});
</script>

<template>
  <div
    ref="workspaceEl"
    class="workspace"
    :class="[`placement-${inspectorPlacement}`, { 'is-collapsed': inspectorCollapsed }]"
  >
    <ImageCanvas class="canvas-slot" />
    <ResultInspector
      class="inspector-slot"
      :style="slotStyle"
      @collapsed-change="onCollapsedChange"
    />
  </div>
</template>

<style scoped>
.workspace {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
  position: relative;
}

.workspace.placement-right {
  flex-direction: row;
}

.workspace.placement-left {
  flex-direction: row-reverse;
}

.workspace.placement-bottom {
  flex-direction: column;
}

.canvas-slot {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

/* 底部展开：高度由内联 style */
.workspace.placement-bottom .inspector-slot {
  width: 100%;
  min-height: 0;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
}

/* 收起：浮层按钮，不占 flex 列/行（对齐 zoom-bar） */
.workspace.is-collapsed .inspector-slot {
  position: absolute;
  z-index: 5;
  width: auto;
  height: auto;
  margin: 0;
  display: block;
}

.workspace.placement-right.is-collapsed .inspector-slot {
  top: var(--space-2);
  right: var(--space-2);
}

.workspace.placement-left.is-collapsed .inspector-slot {
  /* 让出壳层 BackFab（约 28px + 上下间距） */
  top: calc(var(--space-2) + 28px + var(--space-2));
  left: var(--space-2);
}

.workspace.placement-bottom.is-collapsed .inspector-slot {
  bottom: var(--space-2);
  right: var(--space-2);
}
</style>
