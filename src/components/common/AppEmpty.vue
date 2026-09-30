<script setup lang="ts">
defineProps<{
  title: string;
  description?: string;
  /** 紧凑模式：面板内小空态/错误态（小号标题、去装饰间距） */
  compact?: boolean;
}>();
</script>

<template>
  <div class="empty" :class="{ compact }">
    <h2>{{ title }}</h2>
    <p v-if="description">{{ description }}</p>
    <div v-if="$slots.default" class="actions">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.empty {
  text-align: center;
  max-width: 420px;
}

.empty.compact {
  max-width: none;
}

h2 {
  margin: 0 0 var(--space-2);
  font-family: var(--font-display);
  font-size: var(--text-lg);
  font-weight: var(--font-weight-title);
  color: var(--text);
}

.compact h2 {
  font-size: var(--text-md);
  margin-bottom: var(--space-1);
}

p {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-md);
  line-height: 1.5;
}

.actions {
  margin-top: var(--space-4);
  display: flex;
  gap: var(--space-2);
  justify-content: center;
  flex-wrap: wrap;
}
</style>
