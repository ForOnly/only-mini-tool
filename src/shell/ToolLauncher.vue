<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppContextMenu from "@/components/common/AppContextMenu.vue";
import type { ContextMenuItem } from "@/components/common/contextMenuTypes";
import ToolCard from "@/components/common/ToolCard.vue";
import { useWorkbench } from "@/composables/useWorkbench";

const MENU_HALF = 68;

const { t } = useI18n();
const {
  tools,
  openTool,
  openSettings,
  openToolConfig,
  closeTool,
  isToolRunning,
  isToolDisposing,
} = useWorkbench();

const menuId = ref<string | null>(null);
const menuX = ref(0);
const menuY = ref(0);

const menuOpen = computed(() => menuId.value !== null);

const menuItems = computed<ContextMenuItem[]>(() => {
  const id = menuId.value;
  if (!id) {
    return [];
  }
  const tool = tools.value.find((item) => item.id === id);
  const busy = isToolDisposing(id);
  return [
    { id: "open", label: t("launcher.open"), disabled: busy },
    {
      id: "configure",
      label: t("launcher.configure"),
      disabled: busy || !tool?.settingsSection,
    },
    {
      id: "close",
      label: t("launcher.close"),
      disabled: busy || !isToolRunning(id),
      danger: true,
    },
  ];
});

function openMenuForCard(id: string, event: MouseEvent) {
  if (menuId.value === id) {
    menuId.value = null;
    return;
  }
  const card = (event.currentTarget as HTMLElement | null) ?? null;
  const rect = card?.getBoundingClientRect();
  if (rect) {
    menuX.value = rect.left + rect.width / 2 - MENU_HALF;
    menuY.value = rect.bottom + 4;
  } else {
    menuX.value = event.clientX;
    menuY.value = event.clientY;
  }
  menuId.value = id;
}

function closeMenu() {
  menuId.value = null;
}

async function onMenuSelect(actionId: string) {
  const id = menuId.value;
  if (!id) {
    return;
  }
  closeMenu();
  if (actionId === "open") {
    await openTool(id);
    return;
  }
  if (actionId === "configure") {
    openToolConfig(id);
    return;
  }
  if (actionId === "close") {
    await closeTool(id);
  }
}
</script>

<template>
  <div class="launcher">
    <header class="head">
      <h1 class="title">{{ t("app.name") }}</h1>
      <AppButton
        class="gear-btn"
        variant="ghost"
        :title="t('settings.title')"
        @click="openSettings('home')"
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
    </header>

    <div class="grid-wrap">
      <div class="grid">
        <div v-for="tool in tools" :key="tool.id" class="card-wrap">
          <ToolCard
            :title="t(tool.labelKey)"
            :description="tool.descriptionKey ? t(tool.descriptionKey) : undefined"
            :icon="tool.icon"
            :running="isToolRunning(tool.id)"
            :aria-expanded="menuId === tool.id"
            @click="openMenuForCard(tool.id, $event)"
          />
        </div>
      </div>
    </div>

    <AppContextMenu
      :open="menuOpen"
      :x="menuX"
      :y="menuY"
      :items="menuItems"
      @close="closeMenu"
      @select="onMenuSelect"
    />
  </div>
</template>

<style scoped>
.launcher {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: var(--space-5);
  overflow: hidden;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-5);
  flex-shrink: 0;
}

.title {
  margin: 0;
  font-family: var(--font-display);
  font-size: var(--text-xl);
  font-weight: var(--font-weight-display);
  letter-spacing: -0.02em;
}

.grid-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.grid {
  display: grid;
  grid-template-columns: repeat(
    auto-fill,
    minmax(var(--launcher-card-min), var(--launcher-card-max))
  );
  gap: var(--launcher-gap);
  align-content: start;
  justify-content: start;
}

.card-wrap {
  position: relative;
}

.gear-btn {
  position: relative;
}

.gear {
  width: 20px;
  height: 20px;
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
