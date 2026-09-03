<script setup lang="ts">
defineProps<{
  title?: string;
}>();
</script>

<template>
  <section class="panel">
    <header v-if="title || $slots.header" class="head">
      <slot name="header">
        <h3>{{ title }}</h3>
      </slot>
    </header>
    <div class="body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  width: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface);
  overflow: hidden;
}

.head {
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}

h3 {
  margin: 0;
  font-size: var(--text-md);
  font-weight: var(--font-weight-title);
}

.body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--space-3);
}
</style>
