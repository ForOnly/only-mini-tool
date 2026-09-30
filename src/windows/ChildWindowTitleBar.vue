<script setup lang="ts">
/** 子窗口简化标题栏：拖拽区 + 复用 WindowControls（getCurrentWindow 天然按窗生效）。
 *  关闭走 close()——各窗口 App 经 onCloseRequested 拦截做清理后再 destroy。 */

import { getCurrentWindow } from "@tauri-apps/api/window";

import WindowControls from "@/shell/WindowControls.vue";
import { useWindowControls } from "@/composables/useWindowControls";

defineProps<{
  title: string;
}>();

const { isMaximized, minimize, toggleMaximize } = useWindowControls();

async function requestClose() {
  try {
    await getCurrentWindow().close();
  } catch {
    /* 非 Tauri / 权限不足 */
  }
}
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <h1 class="title" data-tauri-drag-region>{{ title }}</h1>
    <WindowControls
      :is-maximized="isMaximized"
      @minimize="minimize"
      @toggle-maximize="toggleMaximize"
      @close="requestClose"
    />
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  height: var(--titlebar-height);
  flex-shrink: 0;
  padding-left: var(--space-3);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  user-select: none;
}

.title {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
