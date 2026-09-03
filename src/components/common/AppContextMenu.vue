<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";

import type { ContextMenuItem } from "@/components/common/contextMenuTypes";

const props = defineProps<{
  open: boolean;
  x: number;
  y: number;
  items: ContextMenuItem[];
}>();

const emit = defineEmits<{
  close: [];
  select: [id: string];
}>();

const rootEl = ref<HTMLElement | null>(null);
const pos = ref({ left: 0, top: 0 });

const visibleItems = computed(() => props.items);

function clampPosition() {
  const el = rootEl.value;
  const pad = 8;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  let left = props.x;
  let top = props.y;
  if (el) {
    const rect = el.getBoundingClientRect();
    if (left + rect.width > vw - pad) {
      left = Math.max(pad, vw - pad - rect.width);
    }
    if (top + rect.height > vh - pad) {
      top = Math.max(pad, vh - pad - rect.height);
    }
  }
  pos.value = { left, top };
}

watch(
  () => [props.open, props.x, props.y, props.items] as const,
  async ([open]) => {
    if (!open) {
      return;
    }
    pos.value = { left: props.x, top: props.y };
    await nextTick();
    clampPosition();
  },
);

function onSelect(item: ContextMenuItem) {
  if (item.disabled) {
    return;
  }
  emit("select", item.id);
  emit("close");
}

function onPointerDown(event: PointerEvent) {
  if (!props.open) {
    return;
  }
  const target = event.target as HTMLElement | null;
  if (!target) {
    return;
  }
  // 交给触发按钮自行 toggle，避免 pointerdown 先关、click 再开
  if (target.closest?.("[data-menu-anchor]")) {
    return;
  }
  if (rootEl.value && rootEl.value.contains(target)) {
    return;
  }
  emit("close");
}

function onKeydown(event: KeyboardEvent) {
  if (!props.open) {
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    emit("close");
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", onPointerDown, true);
  window.addEventListener("keydown", onKeydown, true);
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", onPointerDown, true);
  window.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="rootEl"
      class="ctx-menu"
      role="menu"
      :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
      @contextmenu.prevent
    >
      <button
        v-for="item in visibleItems"
        :key="item.id"
        type="button"
        role="menuitem"
        :disabled="item.disabled"
        :class="{ danger: item.danger }"
        @click="onSelect(item)"
      >
        {{ item.label }}
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.ctx-menu {
  position: fixed;
  z-index: calc(var(--z-message) + 1);
  min-width: 8.5rem;
  padding: var(--space-1);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface);
  box-shadow: var(--shadow-menu);
  display: flex;
  flex-direction: column;
  animation: ctx-enter var(--motion-fast) both;
}

@keyframes ctx-enter {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes ctx-enter-fade {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

button {
  appearance: none;
  border: 0;
  background: transparent;
  color: var(--text);
  text-align: left;
  padding: var(--space-2) var(--space-3);
  border-radius: calc(var(--radius) - 2px);
  font-size: var(--text-md);
  cursor: pointer;
}

button:hover:not(:disabled) {
  background: var(--surface-2);
}

button.danger:hover:not(:disabled) {
  color: var(--danger);
}

button:disabled {
  color: var(--text-muted);
  cursor: not-allowed;
}

button:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: -2px;
}

@media (prefers-reduced-motion: reduce) {
  .ctx-menu {
    animation-name: ctx-enter-fade;
  }
}
</style>
