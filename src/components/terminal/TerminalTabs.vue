<script setup lang="ts">
/** 多会话标签宿主：N 个 TerminalView 常驻（v-show，切标签不销毁）+「+」新建通用 shell。
 *  两种用法：内嵌（设置页展开区，addSession 按 venv 追加标签）/ 子窗口
 *  （attachOnlyIds 附着既有会话，或「+」新建）。标签关闭即销毁该会话（D3）。 */

import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
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

/** 「+」：新建通用 shell 会话（无 venv 激活）。 */
async function newTab() {
  seq += 1;
  const index = tabs.value.length + 1;
  await addSession(
    `shell:${seq}-${Date.now()}`,
    {},
    `${t("terminal.tabTitle")} ${index}`,
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

/** 拖出用：全部标签的会话 id（attachOnly 直取；创建型查登记表，
 *  TerminalView 首挂载完成前该标签跳过——调用方在挂载后取）。 */
function sessionIds(): string[] {
  const ids: string[] = [];
  for (const tab of tabs.value) {
    if (tab.attachId) {
      ids.push(tab.attachId);
    } else {
      const state = getSession(tab.key);
      if (state) ids.push(state.id);
    }
  }
  return ids;
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

defineExpose({ addSession, newTab, sessionIds, activeKey });
</script>

<template>
  <div class="terminal-tabs">
    <div class="tabbar" role="tablist">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        type="button"
        role="tab"
        class="tab"
        :class="{ active: tab.key === activeKey }"
        :aria-selected="tab.key === activeKey"
        @click="activeKey = tab.key"
      >
        <span class="tab-title">{{ tab.title }}</span>
        <span
          v-if="tab.venv"
          class="tab-venv"
        >{{ tab.venv }}</span>
        <span
          class="tab-close"
          role="button"
          :aria-label="t('terminal.close')"
          @click.stop="closeTab(tab)"
        >×</span>
      </button>
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

.tab {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--text-sm);
  padding: 4px var(--space-2);
  cursor: pointer;
  white-space: nowrap;
}

.tab:hover {
  color: var(--text);
}

.tab.active {
  color: var(--text);
  border-bottom-color: var(--accent);
}

.tab-venv {
  font-size: var(--text-xs, 12px);
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  border-radius: var(--radius);
  padding: 0 4px;
}

.tab-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: var(--radius);
  font-size: 13px;
  line-height: 1;
  color: var(--text-muted);
}

.tab-close:hover {
  background: color-mix(in srgb, var(--danger) 14%, transparent);
  color: var(--danger);
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
