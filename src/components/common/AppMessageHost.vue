<script setup lang="ts">
import AppMessage from "@/components/common/AppMessage.vue";
import { useMessage } from "@/composables/useMessage";

const { messages, dismiss } = useMessage();
</script>

<template>
  <div class="host" aria-live="polite">
    <TransitionGroup name="msg" tag="div" class="stack">
      <AppMessage
        v-for="item in messages"
        :key="item.id"
        :item="item"
        @close="dismiss(item.id)"
      />
    </TransitionGroup>
  </div>
</template>

<style scoped>
.host {
  position: fixed;
  top: var(--space-3);
  inset-inline: 0;
  z-index: var(--z-message);
  display: flex;
  flex-direction: column;
  align-items: center;
  pointer-events: none;
  padding-inline: var(--space-4);
}

.stack {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  max-width: min(400px, 100%);
}

.stack :deep(.msg) {
  pointer-events: auto;
}

.msg-enter-active,
.msg-leave-active {
  transition:
    opacity var(--motion-normal),
    transform var(--motion-normal);
}

.msg-move {
  transition: transform var(--motion-normal);
}

.msg-enter-from {
  opacity: 0;
  transform: translateY(-8px);
}

.msg-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

.msg-leave-active {
  position: absolute;
  width: min(400px, calc(100vw - 2 * var(--space-4)));
}

@media (prefers-reduced-motion: reduce) {
  .msg-enter-active,
  .msg-leave-active,
  .msg-move {
    transition: opacity var(--motion-fast);
  }

  .msg-enter-from,
  .msg-leave-to {
    transform: none;
  }
}
</style>
