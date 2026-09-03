<script setup lang="ts">
import AppMessageHost from "@/components/common/AppMessageHost.vue";
import SettingsView from "@/settings/SettingsView.vue";
import ToolConfigView from "@/shell/ToolConfigView.vue";
import ToolHeader from "@/shell/ToolHeader.vue";
import ToolLauncher from "@/shell/ToolLauncher.vue";
import ToolStage from "@/shell/ToolStage.vue";
import { useWorkbench } from "@/composables/useWorkbench";

const { mainView } = useWorkbench();
</script>

<template>
  <div class="shell">
    <div class="body">
      <Transition name="view-fade" mode="out-in">
        <ToolLauncher v-if="mainView === 'home'" key="home" class="view-root" />
        <div v-else-if="mainView === 'settings'" key="settings" class="view-root">
          <SettingsView />
        </div>
        <div v-else-if="mainView === 'toolConfig'" key="toolConfig" class="view-root">
          <ToolConfigView />
        </div>
      </Transition>

      <!-- 常驻挂载：回桌面时仅隐藏，KeepAlive 缓存不丢 -->
      <div v-show="mainView === 'tool'" class="tool-view view-root">
        <ToolHeader />
        <ToolStage />
      </div>
    </div>

    <AppMessageHost />
  </div>
</template>

<style scoped>
.shell {
  height: 100%;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
}

.body {
  flex: 1;
  min-height: 0;
  position: relative;
}

.view-root {
  height: 100%;
  min-height: 0;
}

.tool-view {
  display: flex;
  flex-direction: column;
  position: absolute;
  inset: 0;
  z-index: 1;
}

.view-fade-enter-active,
.view-fade-leave-active {
  transition: opacity var(--motion-normal);
}

.view-fade-enter-from,
.view-fade-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .view-fade-enter-active,
  .view-fade-leave-active {
    transition: none;
  }
}
</style>
