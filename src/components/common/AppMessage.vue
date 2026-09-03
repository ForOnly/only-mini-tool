<script setup lang="ts">
import { useI18n } from "vue-i18n";

import type { MessageItem } from "@/composables/useMessage";

defineProps<{
  item: MessageItem;
}>();

defineEmits<{
  close: [];
}>();

const { t } = useI18n();
</script>

<template>
  <div
    class="msg"
    :class="item.type"
    :role="item.type === 'error' || item.type === 'warning' ? 'alert' : 'status'"
  >
    <span class="bar" aria-hidden="true" />
    <span class="text">{{ item.text }}</span>
    <button
      v-if="item.closable"
      type="button"
      class="close"
      :title="t('shell.messageClose')"
      :aria-label="t('shell.messageClose')"
      @click="$emit('close')"
    >
      ×
    </button>
  </div>
</template>

<style scoped>
.msg {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  width: min(400px, 100%);
  padding: 10px 12px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--surface);
  box-shadow: var(--shadow-message);
  font-size: var(--text-md);
  line-height: 1.45;
  color: var(--text);
  word-break: break-word;
  overflow: hidden;
}

.bar {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: var(--accent);
}

.text {
  flex: 1;
  min-width: 0;
  padding-inline-start: 2px;
}

.close {
  flex-shrink: 0;
  box-sizing: border-box;
  width: 24px;
  height: 24px;
  margin: -2px -4px -2px 0;
  padding: 0;
  border: none;
  border-radius: calc(var(--radius) - 2px);
  background: transparent;
  color: var(--text-muted);
  font-size: var(--text-base);
  line-height: 1;
  cursor: pointer;
}

.close:hover {
  background: var(--surface-2);
  color: var(--text);
}

.close:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 1px;
}

.msg.success {
  border-color: color-mix(in srgb, var(--ocr-box) 28%, var(--border));
  background: color-mix(in srgb, var(--ocr-box) 7%, var(--surface));
}

.msg.success .bar {
  background: var(--ocr-box);
}

.msg.error {
  border-color: color-mix(in srgb, var(--danger) 28%, var(--border));
  background: color-mix(in srgb, var(--danger) 7%, var(--surface));
}

.msg.error .bar {
  background: var(--danger);
}

.msg.warning {
  border-color: color-mix(in srgb, var(--warning) 28%, var(--border));
  background: color-mix(in srgb, var(--warning) 7%, var(--surface));
}

.msg.warning .bar {
  background: var(--warning);
}

.msg.info {
  border-color: color-mix(in srgb, var(--accent) 28%, var(--border));
  background: color-mix(in srgb, var(--accent) 7%, var(--surface));
}

.msg.info .bar {
  background: var(--accent);
}
</style>
