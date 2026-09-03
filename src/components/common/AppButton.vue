<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: "primary" | "ghost";
    type?: "button" | "submit";
  }>(),
  {
    variant: "primary",
    type: "button",
  },
);
</script>

<template>
  <button class="btn" :class="variant" :type="type">
    <slot />
  </button>
</template>

<style scoped>
.btn {
  position: relative;
  box-sizing: border-box;
  border: 1px solid transparent;
  border-radius: var(--radius);
  min-height: 36px;
  padding: var(--space-2) var(--space-3);
  cursor: pointer;
  font-size: var(--text-md);
  line-height: 1.25;
  transition:
    background var(--motion-normal),
    color var(--motion-normal),
    border-color var(--motion-normal);
}

.primary {
  background: var(--accent);
  color: var(--on-accent);
  border-color: var(--accent);
}

.primary:hover {
  background: var(--accent-hover);
  border-color: var(--accent-hover);
}

.ghost {
  background: transparent;
  color: var(--text);
  border-color: var(--border);
}

.ghost:hover {
  background: var(--surface-2);
}

.btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
</style>
