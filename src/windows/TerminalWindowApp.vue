<script setup lang="ts">
/** 终端子窗口：TerminalTabs 宿主（多会话标签 + 「+」新建）。
 *  会话归属（D3）：本窗名下全部会话随关窗销毁；全部标签关闭即自动关窗。
 *  主题跟随：3s 轮询 get_appearance（项目无事件系统的零基建方案）。 */

import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "vue-i18n";

import TerminalTabs from "@/components/terminal/TerminalTabs.vue";
import { useTerminalSessions } from "@/components/terminal/useTerminalSessions";
import { useAppearance } from "@/composables/useAppearance";
import ChildWindowTitleBar from "@/windows/ChildWindowTitleBar.vue";

const props = defineProps<{
  sessionIds: string[];
  titles?: string[];
}>();

const { t } = useI18n();
const { disposeById } = useTerminalSessions();
const { refresh: refreshAppearance } = useAppearance();

const tabs = ref<InstanceType<typeof TerminalTabs> | null>(null);
let unlistenClose: (() => void) | null = null;
let themeTimer: number | undefined;
let emptyTimer: number | undefined;

const title = computed(() => {
  const first = props.titles?.[0];
  return first ? t("terminal.windowTitle", { name: first }) : t("terminal.tabTitle");
});

onMounted(async () => {
  const win = getCurrentWindow();
  unlistenClose = await win.onCloseRequested(async (event) => {
    await event.preventDefault();
    // 销毁本窗名下全部会话（attachOnly + 「+」新建的），再强制关窗
    const payload = tabs.value?.sessionsPayload();
    for (const id of payload?.ids ?? props.sessionIds) {
      await disposeById(id);
    }
    await win.destroy();
  });
  themeTimer = window.setInterval(() => {
    void refreshAppearance();
  }, 3000);
});

onUnmounted(() => {
  unlistenClose?.();
  if (themeTimer != undefined) window.clearInterval(themeTimer);
  if (emptyTimer != undefined) window.clearTimeout(emptyTimer);
});

/** 全部标签关闭 → 自动关窗（destroy 不触发 close-requested，无重入）。
 *  延迟窗内复查：用户点「+」新建了标签则不关（否则新会话漏 dispose）。 */
function onEmptied() {
  if (emptyTimer != undefined) window.clearTimeout(emptyTimer);
  emptyTimer = window.setTimeout(() => {
    if (tabs.value?.isEmpty) {
      void getCurrentWindow().destroy();
    }
  }, 300);
}
</script>

<template>
  <div class="terminal-window">
    <ChildWindowTitleBar :title="title" />
    <main class="body">
      <TerminalTabs
        ref="tabs"
        :attach-only-ids="sessionIds"
        :attach-only-titles="titles"
        @emptied="onEmptied"
      />
    </main>
  </div>
</template>

<style scoped>
.terminal-window {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background: var(--bg);
}

.body {
  flex: 1;
  min-height: 0;
}
</style>
