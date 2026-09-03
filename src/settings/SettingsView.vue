<script setup lang="ts">
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import { useWorkbench } from "@/composables/useWorkbench";
import AppearanceSection from "@/settings/sections/AppearanceSection.vue";
import DebugSection from "@/settings/sections/DebugSection.vue";

const { t } = useI18n();
const { goBack } = useWorkbench();
</script>

<template>
  <section class="settings">
    <div class="settings-inner">
      <header class="head">
        <h1>{{ t("settings.title") }}</h1>
        <AppButton variant="ghost" @click="goBack">{{ t("settings.back") }}</AppButton>
      </header>

      <AppearanceSection />
      <DebugSection />
    </div>
  </section>
</template>

<style scoped>
.settings {
  width: 100%;
  height: 100%;
  overflow: auto;
  background: var(--bg);
}

.settings-inner {
  max-width: 720px;
  margin-inline: auto;
  padding: var(--space-5);
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-5);
}

h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--text-lg);
  font-weight: var(--font-weight-title);
}
</style>
