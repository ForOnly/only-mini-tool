<script setup lang="ts">
import { useI18n } from "vue-i18n";

import AppearanceSection from "@/settings/sections/AppearanceSection.vue";
import DebugSection from "@/settings/sections/DebugSection.vue";

const { t } = useI18n();
</script>

<template>
  <section class="settings">
    <div class="settings-inner">
      <header class="head">
        <h1>{{ t("settings.title") }}</h1>
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
  margin-bottom: var(--space-5);
}

h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--text-lg);
  font-weight: var(--font-weight-title);
}
</style>
