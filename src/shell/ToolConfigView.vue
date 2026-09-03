<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import { useWorkbench } from "@/composables/useWorkbench";

const { t } = useI18n();
const { configTool, goBack } = useWorkbench();

const section = computed(() => configTool.value?.settingsSection ?? null);
const title = computed(() =>
  configTool.value ? t(configTool.value.labelKey) : t("settings.toolConfig"),
);
</script>

<template>
  <section class="config">
    <div class="inner">
      <header class="head">
        <h1>{{ t("settings.toolConfig") }} · {{ title }}</h1>
        <AppButton variant="ghost" @click="goBack">{{ t("settings.back") }}</AppButton>
      </header>

      <component :is="section" v-if="section" />
    </div>
  </section>
</template>

<style scoped>
.config {
  width: 100%;
  height: 100%;
  overflow: auto;
  background: var(--bg);
}

.inner {
  max-width: 720px;
  margin-inline: auto;
  padding: var(--space-5);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-5);
  gap: var(--space-3);
}

h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--text-lg);
  font-weight: var(--font-weight-title);
}
</style>
