<script setup lang="ts">
import { nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppContextMenu from "@/components/common/AppContextMenu.vue";
import AppEmpty from "@/components/common/AppEmpty.vue";
import { useMessage } from "@/composables/useMessage";
import { useWorkbench } from "@/composables/useWorkbench";
import OcrBox from "@/tools/ocr/OcrBox.vue";
import { useOcr } from "@/tools/ocr/useOcr";
import { useOcrActions } from "@/tools/ocr/useOcrActions";

defineOptions({ name: "ImageCanvas" });

const { t } = useI18n();
const { success, error } = useMessage();
const { mainView } = useWorkbench();
const canvasEl = ref<HTMLElement | null>(null);

const menuOpen = ref(false);
const menuX = ref(0);
const menuY = ref(0);

const {
  hasImage,
  imageUrl,
  words,
  boxesVisible,
  highlightIndex,
  zoom,
  panX,
  panY,
  busy,
  imageNaturalWidth,
  imageNaturalHeight,
  setHighlight,
  revealWord,
  zoomAt,
  zoomIn,
  zoomOut,
  fitToView,
  panBy,
  setImageSize,
  openFile,
  pasteImage,
  copyText,
} = useOcr();

const { items: actionItems, run: runAction } = useOcrActions();

function openMenuAt(x: number, y: number) {
  menuX.value = x;
  menuY.value = y;
  menuOpen.value = true;
}

function onContextMenu(event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  openMenuAt(event.clientX, event.clientY);
}

function closeMenu() {
  menuOpen.value = false;
}

async function onMenuSelect(id: string) {
  await runAction(id);
}

const FIT_MIN_SIDE = 32;
const FIT_MAX_TRIES = 10;

function runFit(naturalW: number, naturalH: number, tryIndex = 0) {
  const el = canvasEl.value;
  if (!el || naturalW <= 0 || naturalH <= 0) {
    return;
  }
  const rect = el.getBoundingClientRect();
  // 空态切有图时布局可能尚未就绪，禁止用近零尺寸算出极小 zoom
  if (rect.width < FIT_MIN_SIDE || rect.height < FIT_MIN_SIDE) {
    if (tryIndex < FIT_MAX_TRIES) {
      requestAnimationFrame(() => runFit(naturalW, naturalH, tryIndex + 1));
    }
    return;
  }
  fitToView(rect.width, rect.height, naturalW, naturalH);
}

function onFitClick() {
  if (!hasImage.value) {
    return;
  }
  runFit(imageNaturalWidth.value, imageNaturalHeight.value);
}

function onWheel(event: WheelEvent) {
  if (mainView.value !== "tool" || !hasImage.value) {
    return;
  }
  event.preventDefault();
  const el = event.currentTarget as HTMLElement;
  const rect = el.getBoundingClientRect();
  const x = event.clientX - rect.left;
  const y = event.clientY - rect.top;

  if (event.ctrlKey) {
    const delta = event.deltaY > 0 ? -0.1 : 0.1;
    zoomAt(zoom.value + delta, x, y);
    return;
  }
  panBy(-(event.deltaX || 0), -event.deltaY);
}

function onPointerDown(event: PointerEvent) {
  if (!hasImage.value || event.button !== 0) {
    return;
  }
  const target = event.target as HTMLElement | null;
  if (target?.closest?.(".box") || target?.closest?.(".zoom-bar")) {
    return;
  }
  const startX = event.clientX;
  const startY = event.clientY;
  const originX = panX.value;
  const originY = panY.value;
  const move = (e: PointerEvent) => {
    panX.value = originX + (e.clientX - startX);
    panY.value = originY + (e.clientY - startY);
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

function onImgLoad(event: Event) {
  const img = event.target as HTMLImageElement;
  setImageSize(img.naturalWidth, img.naturalHeight);
  void nextTick(() => runFit(img.naturalWidth, img.naturalHeight));
}

async function copyWord(index: number) {
  const word = words.value[index];
  if (!word) {
    return;
  }
  revealWord(index);
  try {
    await copyText(word.text);
    success(t("ocr.copied"));
  } catch {
    error(t("ocr.copyFailed"));
  }
}
</script>

<template>
  <div
    ref="canvasEl"
    class="canvas"
    :class="{ empty: !hasImage }"
    @pointerdown="onPointerDown"
    @wheel="onWheel"
    @contextmenu="onContextMenu"
  >
    <div v-if="hasImage" class="zoom-bar" @pointerdown.stop>
      <button
        type="button"
        class="zoom-btn"
        :title="t('ocr.zoomOut')"
        :aria-label="t('ocr.zoomOut')"
        @click="zoomOut"
      >
        −
      </button>
      <button
        type="button"
        class="zoom-btn"
        :title="t('ocr.zoomIn')"
        :aria-label="t('ocr.zoomIn')"
        @click="zoomIn"
      >
        +
      </button>
      <button
        type="button"
        class="zoom-btn fit"
        :title="t('ocr.zoomFit')"
        :aria-label="t('ocr.zoomFit')"
        @click="onFitClick"
      >
        {{ t("ocr.zoomFit") }}
      </button>
    </div>

    <AppEmpty
      v-if="!hasImage"
      :title="t('ocr.placeholderTitle')"
      :description="t('ocr.placeholderBody')"
    >
      <AppButton variant="primary" :disabled="busy" @click="openFile">
        {{ t("ocr.open") }}
      </AppButton>
      <AppButton variant="ghost" :disabled="busy" @click="pasteImage">
        {{ t("ocr.paste") }}
      </AppButton>
    </AppEmpty>

    <div
      v-else
      class="viewport"
      :style="{
        transform: `translate(${panX}px, ${panY}px) scale(${zoom})`,
      }"
    >
      <div class="stage">
        <img
          v-if="imageUrl"
          class="photo"
          :src="imageUrl"
          alt=""
          draggable="false"
          @load="onImgLoad"
        />
        <div v-if="boxesVisible" class="overlay">
          <OcrBox
            v-for="(word, index) in words"
            :key="index"
            :left="word.rect.left"
            :top="word.rect.top"
            :width="word.rect.width"
            :height="word.rect.height"
            :active="highlightIndex === index"
            @enter="setHighlight(index)"
            @leave="setHighlight(null)"
            @dblclick="copyWord(index)"
          />
        </div>
      </div>
    </div>

    <div
      v-if="hasImage && busy"
      class="busy-layer"
      aria-live="polite"
      aria-busy="true"
    >
      <span class="busy-spinner" aria-hidden="true" />
      <span class="busy-label">{{ t("ocr.statusBusy") }}</span>
    </div>

    <AppContextMenu
      :open="menuOpen"
      :x="menuX"
      :y="menuY"
      :items="actionItems"
      @close="closeMenu"
      @select="onMenuSelect"
    />
  </div>
</template>

<style scoped>
.canvas {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg);
  cursor: grab;
  user-select: none;
}

.canvas.empty {
  cursor: default;
  padding: var(--space-5);
}

.canvas:not(.empty):active {
  cursor: grabbing;
}

.zoom-bar {
  position: absolute;
  left: var(--space-2);
  bottom: var(--space-2);
  z-index: 2;
  display: flex;
  gap: var(--space-1);
}

.zoom-btn {
  appearance: none;
  min-width: 32px;
  height: 28px;
  padding: 0 var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  color: var(--text);
  font-size: var(--text-md);
  line-height: 1;
  cursor: pointer;
}

.zoom-btn.fit {
  font-size: var(--text-sm);
}

.zoom-btn:hover {
  border-color: var(--accent);
}

.zoom-btn:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.viewport {
  position: absolute;
  left: 0;
  top: 0;
  transform-origin: 0 0;
  will-change: transform;
}

.stage {
  position: relative;
  display: inline-block;
  line-height: 0;
}

.photo {
  display: block;
  max-width: none;
  pointer-events: none;
}

.overlay {
  position: absolute;
  inset: 0;
}

.busy-layer {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  pointer-events: none;
  background: color-mix(in srgb, var(--bg) 55%, transparent);
}

.busy-spinner {
  box-sizing: border-box;
  width: 28px;
  height: 28px;
  border: 2px solid color-mix(in srgb, var(--accent) 28%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: ocr-spin 0.7s linear infinite;
}

.busy-label {
  font-size: var(--text-md);
  font-weight: var(--font-weight-title);
  color: var(--text);
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--surface) 88%, transparent);
  border: 1px solid var(--border);
}

@keyframes ocr-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .busy-spinner {
    animation: none;
    border-top-color: color-mix(in srgb, var(--accent) 28%, transparent);
    border-color: var(--accent);
  }
}
</style>
