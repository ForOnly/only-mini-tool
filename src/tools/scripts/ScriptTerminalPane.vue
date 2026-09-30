<script setup lang="ts">
/** 编辑页「终端」标签内容：按脚本解析 cwd + venv（复用 run 解析链），
 *  自动激活后交给通用 TerminalView。
 *  会话归属（D3）：sessionKey = `script:<id>`——挂载期间切脚本销毁旧会话，
 *  同脚本往返列表不销毁（重进回放）；显式关闭进入占位态。 */

import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import TerminalView from "@/components/terminal/TerminalView.vue";
import { resolveScriptTerminal } from "@/api/terminal";
import type { TerminalCreatePayload } from "@/api/types";
import { useTerminalSessions } from "@/components/terminal/useTerminalSessions";
import { openChildWindow } from "@/platform/childWindow";
import { useMessage } from "@/composables/useMessage";
import { formatAppError } from "@/utils/error";

const props = defineProps<{
  scriptId: number;
}>();

const { t } = useI18n();
const { error } = useMessage();
const { disposeSession, getSession } = useTerminalSessions();

const config = ref<TerminalCreatePayload | null>(null);
const venvName = ref<string | null>(null);
/** 关闭/重建换 key（TerminalView 仅在挂载时 boot，不 watch sessionKey） */
const epoch = ref(0);
const sessionClosed = ref(false);
let loadSeq = 0;

function sessionKey(id: number): string {
  return `script:${id}`;
}

async function load(id: number) {
  const seq = ++loadSeq;
  try {
    const conf = await resolveScriptTerminal(id);
    if (seq !== loadSeq) return; // 期间又切了脚本
    venvName.value = conf.venvName ?? null;
    config.value = {
      cwd: conf.cwd,
      venv: conf.venvName ?? undefined,
      title: `script:${id}`,
    };
    sessionClosed.value = false;
  } catch (err) {
    if (seq !== loadSeq) return;
    error(formatAppError(err, (key) => t(key)));
  }
}

watch(
  () => props.scriptId,
  (id, old) => {
    // 挂载期间切换脚本：销毁前一脚本会话（unmount 不销毁——往返列表保留回放）
    if (old != null) void disposeSession(sessionKey(old));
    void load(id);
  },
  { immediate: true },
);

async function closeSession() {
  await disposeSession(sessionKey(props.scriptId));
  sessionClosed.value = true;
}

function reopen() {
  sessionClosed.value = false;
  epoch.value += 1;
}

/** 弹出为独立终端窗口：会话所有权移交子窗（关窗即销毁）；
 *  dock 侧保留双附着实时同步，窗口关闭后经 Exit/not_found 收敛到占位态。 */
async function popOut() {
  const state = getSession(sessionKey(props.scriptId));
  if (!state) return;
  const name = venvName.value ?? t("terminal.tabTitle");
  await openChildWindow({
    kind: "terminal",
    label: `terminal-${state.id.slice(0, 8)}`,
    title: t("terminal.windowTitle", { name }),
    params: { sessionIds: state.id, titles: name },
  });
}
</script>

<template>
  <div class="terminal-pane">
    <header class="pane-head">
      <span class="venv-badge" :class="{ global: !venvName }">
        {{ venvName ?? t("terminal.venvNone") }}
      </span>
      <div class="actions">
        <AppButton
          variant="ghost"
          type="button"
          :disabled="sessionClosed || !getSession(sessionKey(scriptId))"
          @click="popOut"
        >
          {{ t("terminal.popOut") }}
        </AppButton>
        <AppButton variant="ghost" type="button" :disabled="sessionClosed" @click="closeSession">
          {{ t("terminal.close") }}
        </AppButton>
      </div>
    </header>
    <div class="pane-body">
      <div v-if="sessionClosed || !config" class="closed">
        <span>{{
          sessionClosed ? t("terminal.sessionClosed") : t("terminal.loading")
        }}</span>
        <AppButton v-if="sessionClosed" variant="ghost" type="button" @click="reopen">
          {{ t("terminal.restart") }}
        </AppButton>
      </div>
      <TerminalView
        v-else
        :key="`${sessionKey(scriptId)}-${epoch}`"
        :session-key="sessionKey(scriptId)"
        :config="config"
      />
    </div>
  </div>
</template>

<style scoped>
.terminal-pane {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
}

.pane-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  border-bottom: 1px solid var(--border);
}

.venv-badge {
  font-size: var(--text-xs, 12px);
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  border-radius: var(--radius);
  padding: 1px 6px;
}

.venv-badge.global {
  color: var(--text-muted);
  background: color-mix(in srgb, var(--text-muted) 12%, transparent);
}

.actions {
  display: flex;
  gap: var(--space-1);
}

.pane-body {
  flex: 1;
  min-height: 0;
  position: relative;
}

.closed {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  font-size: var(--text-sm);
  color: var(--text-muted);
}
</style>
