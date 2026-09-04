<script setup lang="ts">
defineProps<{
  isMaximized: boolean;
}>();

const emit = defineEmits<{
  minimize: [];
  "toggle-maximize": [];
  close: [];
}>();
</script>

<template>
  <div class="controls">
    <button
      type="button"
      class="ctrl"
      :title="$t('shell.windowMinimize')"
      @click="emit('minimize')"
    >
      <!-- Lucide Minus 形 -->
      <svg class="ico" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path
          d="M5 12h14"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>
    <button
      type="button"
      class="ctrl"
      :title="
        isMaximized ? $t('shell.windowRestore') : $t('shell.windowMaximize')
      "
      @click="emit('toggle-maximize')"
    >
      <!-- Lucide Copy 形（还原） -->
      <svg
        v-if="isMaximized"
        class="ico ico-restore"
        viewBox="0 0 24 24"
        fill="none"
        aria-hidden="true"
      >
        <rect
          x="8"
          y="8"
          width="12"
          height="12"
          rx="2"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <path
          d="M4 16V6a2 2 0 0 1 2-2h10"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <!-- Lucide Square 形（最大化） -->
      <svg
        v-else
        class="ico ico-max"
        viewBox="0 0 24 24"
        fill="none"
        aria-hidden="true"
      >
        <rect
          x="5"
          y="5"
          width="14"
          height="14"
          rx="2"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>
    <button
      type="button"
      class="ctrl close"
      :title="$t('shell.windowClose')"
      @click="emit('close')"
    >
      <!-- Lucide X 形 -->
      <svg class="ico" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path
          d="M18 6L6 18M6 6l12 12"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.controls {
  display: flex;
  align-items: stretch;
  height: 100%;
  flex-shrink: 0;
}

.ctrl {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 100%;
  margin: 0;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--text);
  cursor: pointer;
}

.ico {
  width: 16px;
  height: 16px;
  display: block;
}

.ico-max,
.ico-restore {
  width: 14px;
  height: 14px;
}

.ctrl:hover {
  background: var(--surface-2);
}

.ctrl.close:hover {
  background: var(--danger);
  color: #fff;
}
</style>
