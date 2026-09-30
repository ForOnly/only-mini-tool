<script setup lang="ts">
/** 统一标签（TerminalTabs / ScriptBottomPanel 共用）：
 *  30px 密度、底部 2px accent 指示线；徽标与关闭按钮经插槽可选。 */

defineProps<{
  active?: boolean;
}>();

const emit = defineEmits<{
  select: [];
  close: [];
}>();
</script>

<template>
  <button
    type="button"
    role="tab"
    class="tab"
    :class="{ active }"
    :aria-selected="active"
    @click="emit('select')"
  >
    <span class="label"><slot /></span>
    <span v-if="$slots.badge" class="badge"><slot name="badge" /></span>
    <span
      v-if="$slots.close"
      role="button"
      class="close"
      :aria-label="$slots.close ? 'close' : undefined"
      @click.stop="emit('close')"
    >
      <slot name="close" />
    </span>
  </button>
</template>

<style scoped>
.tab {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  min-height: 30px;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--text-sm);
  font-weight: 600;
  padding: 0 var(--space-2);
  cursor: pointer;
  white-space: nowrap;
}

.tab:hover {
  color: var(--text);
}

.tab.active {
  color: var(--text);
  border-bottom-color: var(--accent);
}

.label {
  overflow: hidden;
  text-overflow: ellipsis;
}

.badge {
  font-size: var(--text-xs, 12px);
  font-weight: 500;
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  border-radius: var(--radius);
  padding: 0 5px;
}

.close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: var(--radius);
  font-size: 13px;
  line-height: 1;
  color: var(--text-muted);
}

.close:hover {
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  color: var(--danger);
}
</style>
