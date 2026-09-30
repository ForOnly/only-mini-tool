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
  popout: [];
}>();

const { t } = useI18n();
</script>

<template>
  <div class="run-bar">
    <!-- 左侧图标组：返回 + 拖出编辑器（状态文字已移底栏头部；250px 窄栏稳定单行） -->
    <div class="icon-group">
      <AppButton
        variant="ghost"
        type="button"
        class="icon-btn"
        :title="t('scripts.backToList')"
        :aria-label="t('scripts.backToList')"
        @click="emit('back')"
      >
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" class="icon">
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
      </AppButton>
      <AppButton
        variant="ghost"
        type="button"
        class="icon-btn"
        :title="t('scripts.popOutEditor')"
        :aria-label="t('scripts.popOutEditor')"
        @click="emit('popout')"
      >
        <!-- Lucide ExternalLink 形 -->
        <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" class="icon">
          <path
            d="M15 3h6v6"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <path
            d="M10 14L21 3"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <path
            d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </AppButton>
    </div>
    <div class="spacer" />
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
}

.icon-group {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.spacer {
  flex: 1;
}

.icon-btn {
  min-width: 32px;
  padding: var(--space-1);
}

.icon {
  width: 16px;
  height: 16px;
  display: block;
}
</style>
