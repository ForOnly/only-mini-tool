<script setup lang="ts">
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";

defineProps<{
  dirty: boolean;
  running: boolean;
  saving?: boolean;
}>();

const emit = defineEmits<{
  back: [];
  save: [];
  run: [];
  cancel: [];
}>();

const { t } = useI18n();
</script>

<template>
  <div class="run-bar">
    <AppButton variant="ghost" type="button" @click="emit('back')">
      {{ t("scripts.backToList") }}
    </AppButton>
    <div class="spacer" />
    <span v-if="dirty" class="dirty">{{ t("scripts.dirty") }}</span>
    <AppButton variant="ghost" type="button" :disabled="saving || !dirty" @click="emit('save')">
      {{ t("scripts.save") }}
    </AppButton>
    <AppButton
      v-if="!running"
      variant="primary"
      type="button"
      :disabled="saving"
      @click="emit('run')"
    >
      {{ t("scripts.run") }}
    </AppButton>
    <AppButton v-else variant="primary" type="button" @click="emit('cancel')">
      {{ t("scripts.cancelRun") }}
    </AppButton>
  </div>
</template>

<style scoped>
.run-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-top: var(--space-2);
  border-top: 1px solid var(--border);
  flex-wrap: wrap;
}

.spacer {
  flex: 1;
}

.dirty {
  font-size: var(--text-sm);
  color: var(--warning, #c47f17);
}
</style>
