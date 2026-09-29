<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";

const props = defineProps<{
  dirty: boolean;
  running: boolean;
  saving?: boolean;
  /** 本次运行开始时间戳（ms）；来自 useScriptRun，跨页面切换保持准确 */
  runStartedAt?: number | null;
}>();

const emit = defineEmits<{
  back: [];
  save: [];
  run: [];
  cancel: [];
}>();

const { t } = useI18n();

const now = ref(Date.now());
let timer: number | undefined;

watch(
  () => props.running,
  (running) => {
    if (running && timer == undefined) {
      timer = window.setInterval(() => {
        now.value = Date.now();
      }, 1000);
    } else if (!running && timer != undefined) {
      window.clearInterval(timer);
      timer = undefined;
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (timer != undefined) window.clearInterval(timer);
});

const elapsed = computed(() => {
  if (!props.running || props.runStartedAt == null) return null;
  return Math.max(0, Math.floor((now.value - props.runStartedAt) / 1000));
});
</script>

<template>
  <div class="run-bar">
    <AppButton variant="ghost" type="button" @click="emit('back')">
      {{ t("scripts.backToList") }}
    </AppButton>
    <div class="spacer" />
    <span v-if="dirty" class="dirty">{{ t("scripts.dirty") }}</span>
    <span v-if="elapsed != null" class="timer">
      {{ t("scripts.runTimer", { secs: elapsed }) }}
    </span>
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

.timer {
  font-size: var(--text-sm);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
