<script setup lang="ts">
defineProps<{
  title: string;
  description?: string;
  icon?: string;
  running?: boolean;
}>();

defineEmits<{
  click: [event: MouseEvent];
}>();
</script>

<template>
  <button
    type="button"
    class="card"
    data-menu-anchor
    :title="description || undefined"
    @click="$emit('click', $event)"
  >
    <span class="icon-wrap">
      <!-- OCR：扫描角 + 字迹，无内框，避免与 icon-wrap 双套留白 -->
      <svg v-if="icon === 'ocr'" class="icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path
          d="M4 8V5.5A1.5 1.5 0 0 1 5.5 4H8"
          stroke="currentColor"
          stroke-width="1.75"
          stroke-linecap="round"
        />
        <path
          d="M16 4h2.5A1.5 1.5 0 0 1 20 5.5V8"
          stroke="currentColor"
          stroke-width="1.75"
          stroke-linecap="round"
        />
        <path
          d="M20 16v2.5a1.5 1.5 0 0 1-1.5 1.5H16"
          stroke="currentColor"
          stroke-width="1.75"
          stroke-linecap="round"
        />
        <path
          d="M8 20H5.5A1.5 1.5 0 0 1 4 18.5V16"
          stroke="currentColor"
          stroke-width="1.75"
          stroke-linecap="round"
        />
        <path
          d="M8 9.5h8M8 12.5h6M8 15.5h4"
          stroke="currentColor"
          stroke-width="1.75"
          stroke-linecap="round"
        />
      </svg>
      <span v-else class="icon-fallback" aria-hidden="true">·</span>
      <span
        class="status"
        :class="running ? 'on' : 'off'"
        :aria-label="running ? $t('launcher.running') : $t('launcher.stopped')"
      />
    </span>
    <span class="label">{{ title }}</span>
  </button>
</template>

<style scoped>
.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2);
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) + 2px);
  background: var(--surface);
  color: var(--text);
  cursor: pointer;
  transition:
    transform var(--motion-fast),
    border-color var(--motion-fast),
    box-shadow var(--motion-fast);
  min-height: var(--launcher-card-min);
  aspect-ratio: 1;
  box-sizing: border-box;
}

.card:hover {
  border-color: var(--accent);
  box-shadow: var(--shadow-panel);
  transform: translateY(-2px);
}

.card[aria-expanded="true"] {
  border-color: var(--accent);
  box-shadow: var(--shadow-panel);
  transform: none;
}

.card[aria-expanded="true"]:hover {
  transform: none;
}

.card:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.icon-wrap {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--launcher-icon-size);
  height: var(--launcher-icon-size);
  border-radius: calc(var(--radius) + 2px);
  background: color-mix(in srgb, var(--accent) 14%, var(--surface));
  color: var(--accent);
  flex-shrink: 0;
}

.icon {
  width: 80%;
  height: 80%;
  display: block;
}

.icon-fallback {
  font-size: var(--text-lg);
  font-weight: var(--font-weight-display);
  line-height: 1;
}

.status {
  position: absolute;
  top: 0;
  right: 0;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 14%, var(--surface));
  transform: translate(15%, -15%);
}

.status.on {
  background: var(--status-running);
}

.status.off {
  background: var(--status-stopped);
}

.label {
  font-family: var(--font-display);
  font-weight: var(--font-weight-title);
  font-size: var(--text-md);
  line-height: 1.2;
  text-align: center;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (prefers-reduced-motion: reduce) {
  .card:hover,
  .card[aria-expanded="true"]:hover {
    transform: none;
  }
}
</style>
