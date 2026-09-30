<script setup lang="ts">
/** 多会话标签宿主：N 个 TerminalView 常驻（v-show，切标签不销毁）+「+」新建通用 shell。
 *  两种用法：内嵌（设置页展开区，addSession 按 venv 追加标签）/ 子窗口
 *  （attachOnlyIds 附着既有会话，或「+」新建）。标签关闭即销毁该会话（D3）。 */

import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppTab from "@/components/common/AppTab.vue";
import TerminalView from "@/components/terminal/TerminalView.vue";
import { useTerminalSessions } from "@/components/terminal/useTerminalSessions";
import type { TerminalCreatePayload } from "@/api/types";

interface TabDef {
  /** TerminalView sessionKey（登记表键）或 attachOnly 专用键 */
  key: string;
  title: string;
  venv?: string;
  /** attachOnly 模式：直接附着的会话 id（不经登记表） */
  attachId?: string;
  config?: TerminalCreatePayload;
}

const props = defineProps<{
  /** 子窗口模式：仅附着这些既有会话（不新建同名标签） */
  attachOnlyIds?: string[];
  /** attachOnly 标签的展示名（与 attachOnlyIds 等长；缺省用 id 前 8 位） */
  attachOnlyTitles?: string[];
}>();

const emit = defineEmits<{
  /** 全部标签关闭（子窗口据此关窗） */
  emptied: [];
}>();

const { t } = useI18n();
const { disposeSession, disposeById, getSession } = useTerminalSessions();

const tabs = ref<TabDef[]>([]);
const activeKey = ref<string | null>(null);
/** 是否已空标签（供宿主判断「全部关闭」竞态） */
const isEmpty = computed(() => tabs.value.length === 0);
let seq = 0;

/** 追加会话标签（同键幂等：已存在则仅激活）。返回标签键。 */
async function addSession(
  key: string,
  config: TerminalCreatePayload,
  title: string,
  venv?: string,
): Promise<string> {
  const existing = tabs.value.find((tab) => tab.key === key);
  if (existing) {
    activeKey.value = key;
    return key;
  }
  tabs.value.push({ key, title, venv, config });
  activeKey.value = key;
  return key;
}

/** 「+」：新建通用 shell 会话（无 venv 激活）。标题用自增 seq（关中间标签不重号）。 */
async function newTab() {
  seq += 1;
  await addSession(
    `shell:${seq}-${Date.now()}`,
    {},
    `${t("terminal.tabTitle")} ${seq}`,
  );
}

async function closeTab(tab: TabDef) {
  if (tab.attachId) {
    await disposeById(tab.attachId);
  } else {
    await disposeSession(tab.key);
  }
  tabs.value = tabs.value.filter((item) => item.key !== tab.key);
  if (activeKey.value === tab.key) {
    activeKey.value = tabs.value.at(-1)?.key ?? null;
  }
  if (tabs.value.length === 0) {
    emit("emptied");
  }
}

/** 销毁全部标签会话（设置页「会话随页面走」——离开页面时由宿主调用）。 */
async function disposeAllTabs() {
  const current = [...tabs.value];
  tabs.value = [];
  activeKey.value = null;
  for (const tab of current) {
    if (tab.attachId) {
      await disposeById(tab.attachId);
    } else {
      await disposeSession(tab.key);
    }
  }
}

/** 仅清空标签不销毁会话（拖出后所有权移交子窗，防宿主离开时误杀）。 */
function releaseAllTabs() {
  tabs.value = [];
  activeKey.value = null;
}

/** 拖出用：全部标签的会话 id 与标题（attachOnly 直取；创建型查登记表，
 *  TerminalView 首挂载完成前该标签跳过——调用方在挂载后取）。 */
function sessionsPayload(): { ids: string[]; titles: string[] } {
  const ids: string[] = [];
  const titles: string[] = [];
  for (const tab of tabs.value) {
    if (tab.attachId) {
      ids.push(tab.attachId);
      titles.push(tab.title);
    } else {
      const state = getSession(tab.key);
      if (state) {
        ids.push(state.id);
        titles.push(tab.title);
      }
    }
  }
  return { ids, titles };
}

onMounted(() => {
  const ids = props.attachOnlyIds ?? [];
  ids.forEach((id, index) => {
    const title = props.attachOnlyTitles?.[index] ?? id.slice(0, 8);
    const key = `attach:${id}`;
    tabs.value.push({ key, title, attachId: id });
    if (index === 0) activeKey.value = key;
  });
});

defineExpose({ addSession, newTab, disposeAllTabs, releaseAllTabs, sessionsPayload, isEmpty, activeKey });
</script>

<template>
  <div class="terminal-tabs">
    <div class="tabbar" role="tablist">
      <AppTab
        v-for="tab in tabs"
        :key="tab.key"
        :active="tab.key === activeKey"
        @select="activeKey = tab.key"
        @close="closeTab(tab)"
      >
        {{ tab.title }}
        <template v-if="tab.venv" #badge>{{ tab.venv }}</template>
        <template #close>×</template>
      </AppTab>
      <AppButton variant="ghost" type="button" class="new-tab" @click="newTab">
        {{ t("terminal.newTab") }}
      </AppButton>
    </div>
    <div class="panes">
      <TerminalView
        v-for="tab in tabs"
        :key="tab.key"
        v-show="tab.key === activeKey"
        :session-key="tab.key"
        :config="tab.config"
        :attach-only="tab.attachId"
      />
    </div>
  </div>
</template>

<style scoped>
.terminal-tabs {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
}

.tabbar {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  flex-shrink: 0;
  padding: 0 var(--space-2);
  min-height: 30px;
  border-bottom: 1px solid var(--border);
  overflow-x: auto;
  scrollbar-width: none;
}

.tabbar::-webkit-scrollbar {
  display: none;
}

.new-tab {
  flex-shrink: 0;
  white-space: nowrap;
}

.panes {
  flex: 1;
  min-height: 0;
  position: relative;
}
</style>
