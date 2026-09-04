<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppContextMenu from "@/components/common/AppContextMenu.vue";
import type { ContextMenuItem } from "@/components/common/contextMenuTypes";
import ToolCard from "@/components/common/ToolCard.vue";
import { useWorkbench } from "@/composables/useWorkbench";

const MENU_HALF = 68;

const { t } = useI18n();
const {
  tools,
  openTool,
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
</style>
