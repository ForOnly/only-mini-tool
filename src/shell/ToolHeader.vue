<script setup lang="ts">
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import { useWorkbench } from "@/composables/useWorkbench";

const { t } = useI18n();
const { activeTool, goBack } = useWorkbench();
</script>

<template>
  <header class="header">
    <AppButton class="back" variant="ghost" :title="t('shell.backHome')" @click="goBack">
      ←
    </AppButton>
    <h1 v-if="activeTool" class="title">{{ t(activeTool.labelKey) }}</h1>
  </header>
</template>

<style scoped>
.header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--header-height-tool);
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  flex-shrink: 0;
}

.back {
  flex-shrink: 0;
  min-width: 2rem;
  padding-inline: var(--space-2);
}

.title {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--text-md);
  font-weight: var(--font-weight-title);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
