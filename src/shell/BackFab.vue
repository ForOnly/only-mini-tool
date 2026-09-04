<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import { useWorkbench } from "@/composables/useWorkbench";

const { t } = useI18n();
const { mainView, goBack } = useWorkbench();

const visible = computed(() => mainView.value !== "home");

const label = computed(() =>
  mainView.value === "tool" ? t("shell.backHome") : t("settings.back"),
);
</script>

<template>
  <button
    v-if="visible"
    type="button"
    class="back-fab"
    :title="label"
    :aria-label="label"
    @click="goBack"
  >
    <!-- 弯曲回退箭头（undo 形） -->
    <svg class="icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path
        d="M9 14L4 9l5-5"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M4 9h10.5a5.5 5.5 0 0 1 0 11H14"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  </button>
</template>

<style scoped>
/* 对齐 OCR zoom-btn 视觉；固定在客户区左上（标题栏下） */
.back-fab {
  appearance: none;
  position: absolute;
  top: var(--space-2);
  left: var(--space-2);
  z-index: 20;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 32px;
  height: 28px;
  padding: 0 var(--space-2);
  margin: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--surface) 92%, transparent);
  color: var(--text);
  cursor: pointer;
}

.back-fab:hover {
  border-color: var(--accent);
}

.back-fab:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.icon {
  width: 16px;
  height: 16px;
  display: block;
}
</style>
