<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";

import type { UiTheme } from "@/api/types";
import { useAppearance } from "@/composables/useAppearance";
import { useMessage } from "@/composables/useMessage";

const { t } = useI18n();
const { appearance, errorMessage, refresh, setTheme } = useAppearance();
const { error } = useMessage();

const options: { value: UiTheme; labelKey: string }[] = [
  { value: "system", labelKey: "settings.themeSystem" },
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" },
];

function reportError() {
  if (errorMessage.value) {
    error(errorMessage.value);
  }
}

onMounted(() => {
  void refresh().then(reportError);
});

async function onThemeChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value as UiTheme;
  await setTheme(value);
  reportError();
}
</script>

<template>
  <label class="field">
    <span>{{ t("settings.theme") }}</span>
    <select :value="appearance?.preference ?? 'system'" @change="onThemeChange">
      <option v-for="opt in options" :key="opt.value" :value="opt.value">
        {{ t(opt.labelKey) }}
      </option>
    </select>
  </label>
</template>

<style scoped>
.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-bottom: var(--space-4);
}

select {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  max-width: 420px;
  font-size: var(--text-md);
}
</style>
