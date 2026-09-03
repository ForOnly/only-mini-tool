<script setup lang="ts">
import { computed } from "vue";

import { useWorkbench } from "@/composables/useWorkbench";

const { activeTool, isToolRunning, toolEpoch } = useWorkbench();

const stage = computed(() => activeTool.value?.stage);
const toolId = computed(() => activeTool.value?.id ?? null);
const persist = computed(() => activeTool.value?.persistOnDeactivate !== false);
const cacheKey = computed(() => {
  const id = toolId.value;
  if (!id) {
    return "none";
  }
  return `${id}-${toolEpoch(id)}`;
});
const showStage = computed(() => {
  const id = toolId.value;
  return !!id && !!stage.value && isToolRunning(id);
});
</script>

<template>
  <main class="stage">
    <KeepAlive v-if="persist" :max="8">
      <component :is="stage" v-if="showStage" :key="cacheKey" />
    </KeepAlive>
    <component :is="stage" v-else-if="showStage" :key="cacheKey" />
  </main>
</template>

<style scoped>
.stage {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  background: var(--bg);
  display: flex;
  flex-direction: column;
}
</style>
