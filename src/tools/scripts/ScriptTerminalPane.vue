<script setup lang="ts">
/** 编辑页「终端」标签内容：按脚本解析 cwd + venv（复用 run 解析链），
 *  自动激活后交给通用 TerminalView。
 *  会话键含环境指纹 `script:<id>:<venv|global>:<cwd>`——挂载期间切脚本或
 *  改 venv 绑定（保存后）即销毁旧会话重建；同环境往返列表不销毁（重进回放）。 */

import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import TerminalView from "@/components/terminal/TerminalView.vue";
import { resolveScriptTerminal, terminalList } from "@/api/terminal";
import type { TerminalCreatePayload } from "@/api/types";
import { useTerminalSessions } from "@/components/terminal/useTerminalSessions";
import { openChildWindow } from "@/platform/childWindow";
import { useMessage } from "@/composables/useMessage";
import { formatAppError } from "@/utils/error";

const props = defineProps<{
  scriptId: number;
  /** 已保存的脚本 venv 绑定（变化 = 保存了新绑定 → 终端换环境） */
  venvBinding: string;
}>();

const { t } = useI18n();
const { error } = useMessage();
const { disposeSession, getSession, reapSession } = useTerminalSessions();

const config = ref<TerminalCreatePayload | null>(null);
const venvName = ref<string | null>(null);
/** 当前生效会话键（含环境指纹；加载完成后才有效） */
const loadedKey = ref<string | null>(null);
/** 关闭/重建换 key（TerminalView 仅在挂载时 boot，不 watch sessionKey） */
const epoch = ref(0);
const sessionClosed = ref(false);
let loadSeq = 0;

async function load(id: number) {
  const seq = ++loadSeq;
  try {
    const conf = await resolveScriptTerminal(id);
    if (seq !== loadSeq) return; // 期间环境又变了
    venvName.value = conf.venvName ?? null;
    config.value = {
      cwd: conf.cwd,
      venv: conf.venvName ?? undefined,
      title: `script:${id}`,
    };
    loadedKey.value = `script:${id}:${conf.venvName ?? "global"}:${conf.cwd}`;
    sessionClosed.value = false;
  } catch (err) {
    if (seq !== loadSeq) return;
    error(formatAppError(err, (key) => t(key)));
  }
}

watch(
  [() => props.scriptId, () => props.venvBinding],
  () => {
    // 挂载期间切脚本/换绑定：销毁旧环境会话（unmount 不销毁——往返列表保留回放）
    if (loadedKey.value) void disposeSession(loadedKey.value);
    void load(props.scriptId);
  },
  { immediate: true },
);

async function closeSession() {
  if (loadedKey.value) await disposeSession(loadedKey.value);
  sessionClosed.value = true;
}

function reopen() {
  sessionClosed.value = false;
  epoch.value += 1;
}

/** 弹出为独立终端窗口：开窗前探活（跨窗 dispose 后主窗登记表可能是死键）；
 *  会话所有权移交子窗（关窗即销毁），dock 侧保留双附着实时同步。 */
async function popOut() {
  const key = loadedKey.value;
  const state = key ? getSession(key) : null;
  if (!state) return;
  try {
    const alive = (await terminalList()).some((s) => s.id === state.id);
    if (!alive) {
      reapSession(state.id);
      sessionClosed.value = true;
      error(t("errors.terminal.not_found"));
      return;
    }
  } catch (err) {
    error(formatAppError(err, (k) => t(k)));
    return;
  }
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
          :disabled="sessionClosed || !loadedKey || !getSession(loadedKey)"
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
      <div v-if="sessionClosed || !config || !loadedKey" class="closed">
        <span>{{
          sessionClosed ? t("terminal.sessionClosed") : t("terminal.loading")
        }}</span>
        <AppButton v-if="sessionClosed" variant="ghost" type="button" @click="reopen">
          {{ t("terminal.restart") }}
        </AppButton>
      </div>
      <TerminalView
        v-else
        :key="`${loadedKey}-${epoch}`"
        :session-key="loadedKey"
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
