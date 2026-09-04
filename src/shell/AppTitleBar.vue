<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import { useWindowControls } from "@/composables/useWindowControls";
import { useWorkbench } from "@/composables/useWorkbench";
import WindowControls from "@/shell/WindowControls.vue";

const { t } = useI18n();
const { mainView, activeTool, configTool, openSettings } = useWorkbench();
const { isMaximized, minimize, toggleMaximize, close } = useWindowControls();

const showSettings = computed(() => mainView.value !== "settings");

const titleText = computed(() => {
  if (mainView.value === "home") {
    return t("app.name");
  }
  if (mainView.value === "settings") {
    return t("settings.title");
  }
  if (mainView.value === "toolConfig") {
    const name = configTool.value
      ? t(configTool.value.labelKey)
      : t("settings.toolConfig");
    return `${t("settings.toolConfig")} · ${name}`;
  }
  if (mainView.value === "tool" && activeTool.value) {
    return t(activeTool.value.labelKey);
  }
  return t("app.name");
});

function onTitlebarDblClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.closest("button, a, [role='button']")) {
    return;
  }
  void toggleMaximize();
}

function onOpenSettings() {
  const from = mainView.value === "tool" ? "tool" : "home";
  openSettings(from);
}
</script>

<template>
  <header class="titlebar" @dblclick="onTitlebarDblClick">
    <div class="leading">
      <h1 class="title" data-tauri-drag-region>{{ titleText }}</h1>
    </div>

    <div class="drag" data-tauri-drag-region />

    <div class="trailing">
      <AppButton
        v-if="showSettings"
        class="icon-btn"
        variant="ghost"
        :title="t('settings.title')"
        @click="onOpenSettings"
      >
        <svg class="gear" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path
            d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z"
            stroke="currentColor"
            stroke-width="1.5"
          />
          <path
            d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.2a1.7 1.7 0 0 0-1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.2a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3h.1A1.7 1.7 0 0 0 11 3.2V3a2 2 0 1 1 4 0v.2a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.2a1.7 1.7 0 0 0-1.5 1Z"
            stroke="currentColor"
            stroke-width="1.5"
          />
        </svg>
        <span class="sr">{{ t("settings.title") }}</span>
      </AppButton>

      <WindowControls
        :is-maximized="isMaximized"
        @minimize="minimize"
        @toggle-maximize="toggleMaximize"
        @close="close"
      />
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: stretch;
  height: var(--titlebar-height);
  flex-shrink: 0;
  background: var(--surface);
  border-bottom: 1px solid var(--border);
  user-select: none;
}

.leading {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  min-width: 0;
  padding-left: var(--space-2);
  flex-shrink: 1;
}

.title {
  margin: 0;
  padding-inline: var(--space-1);
  font-family: var(--font-display);
  font-size: var(--text-md);
  font-weight: var(--font-weight-title);
  line-height: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.drag {
  flex: 1;
  min-width: var(--space-4);
}

.trailing {
  display: flex;
  align-items: stretch;
  flex-shrink: 0;
}

.icon-btn {
  position: relative;
  align-self: center;
  min-height: 28px;
  min-width: 28px;
  padding: var(--space-1);
  border-color: transparent;
  margin-right: var(--space-1);
}

.gear {
  width: 16px;
  height: 16px;
  display: block;
}

.sr {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}
</style>
