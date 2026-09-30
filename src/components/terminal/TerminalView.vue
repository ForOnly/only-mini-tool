<script setup lang="ts">
/** 终端单会话渲染核心：懒加载 xterm → 创建/附着会话 → 双向流。
 *  生命周期语义（D3）：unmount 只 detach（会话保留，重挂回放）；
 *  显式关闭（disposeSession/restart）才销毁会话。 */

import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";

import AppButton from "@/components/common/AppButton.vue";
import { terminalAttach, terminalDetach, terminalDispose, terminalResize, terminalWrite } from "@/api/terminal";
import type { ColorScheme, TerminalCreatePayload, TerminalEvent } from "@/api/types";
import { useAppearance } from "@/composables/useAppearance";
import { useTerminalSessions } from "@/components/terminal/useTerminalSessions";
import { applyTheme } from "@/components/terminal/terminalTheme";
import { loadXterm } from "@/components/terminal/xtermLoader";

const props = defineProps<{
  /** 业务会话键（同键幂等复用；attachOnly 存在时忽略） */
  sessionKey: string;
  /** 会话创建配置（首挂载创建用） */
  config?: TerminalCreatePayload;
  /** 只附着既有会话（子窗口/拖出场景），不创建、不可重建 */
  attachOnly?: string;
}>();

const emit = defineEmits<{
  session: [id: string];
  exited: [code: number | null];
}>();

const { t } = useI18n();
const { appearance } = useAppearance();
const { ensureSession, markExited, disposeSession } = useTerminalSessions();

const host = ref<HTMLDivElement | null>(null);
const loadError = ref<string | null>(null);
const exited = ref(false);
const exitCode = ref<number | null>(null);
/** 会话已被销毁（如拖出窗口后关闭）——与自然退出区分文案，可重建 */
const sessionClosed = ref(false);

let term: Terminal | null = null;
let fit: FitAddon | null = null;
let attachId: string | null = null;
let sessionId: string | null = null;
let dataDisposable: { dispose(): void } | null = null;
let resizeObserver: ResizeObserver | null = null;
let fitTimer: number | undefined;
let lastCols = 0;
let lastRows = 0;

function scheme(): ColorScheme {
  return appearance.value?.resolved ?? "light";
}

function onEvent(event: TerminalEvent) {
  if (event.kind === "replay" || event.kind === "output") {
    term?.write(event.data);
  } else if (event.kind === "exit") {
    exited.value = true;
    exitCode.value = event.exitCode ?? null;
    if (sessionId) markExited(sessionId, exitCode.value);
    emit("exited", exitCode.value);
  }
}

/** fit + 尺寸同步（隐藏时跳过；重新可见时 ResizeObserver 会补一次）。 */
function fitNow() {
  if (!term || !fit || !host.value) return;
  if (host.value.offsetWidth === 0 || host.value.offsetHeight === 0) return;
  try {
    fit.fit();
  } catch {
    // 布局未就绪——下次观察回调再试
    return;
  }
  if (term.cols !== lastCols || term.rows !== lastRows) {
    lastCols = term.cols;
    lastRows = term.rows;
    if (sessionId) {
      void terminalResize(sessionId, term.cols, term.rows).catch(() => {
        // 会话终结后的 resize——忽略
      });
    }
  }
}

function teardownTerm() {
  if (sessionId && attachId) {
    void terminalDetach(sessionId, attachId).catch(() => {});
  }
  attachId = null;
  dataDisposable?.dispose();
  dataDisposable = null;
  term?.dispose();
  term = null;
  fit = null;
}

async function boot() {
  if (!host.value) return;
  loadError.value = null;
  exited.value = false;
  exitCode.value = null;
  sessionClosed.value = false;
  try {
    const bundle = await loadXterm();
    const terminal = new bundle.Terminal({
      scrollback: 2000,
      fontSize: 13,
      fontFamily: "var(--font-mono, ui-monospace, monospace)",
      cursorBlink: true,
    });
    const fitAddon = new bundle.FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.loadAddon(new bundle.WebLinksAddon());
    terminal.open(host.value);
    term = terminal;
    fit = fitAddon;
    applyTheme(terminal, scheme());

    const id = props.attachOnly ?? (await ensureSession(props.sessionKey, () => props.config ?? {}));
    sessionId = id;
    const ch = new Channel<TerminalEvent>();
    ch.onmessage = onEvent;
    attachId = await terminalAttach(id, ch);
    dataDisposable = terminal.onData((data) => {
      void terminalWrite(id, data).catch((err) => console.warn("[terminal] write failed:", err));
    });
    emit("session", id);
    fitNow();
    terminal.focus();
  } catch (err) {
    const code = (err as { code?: string })?.code;
    if (code === "terminal.not_found") {
      // 会话已被销毁（如拖出窗口后关闭）——覆盖层展示；restart 清登记表后重建
      sessionClosed.value = true;
    } else {
      console.error("[terminal] boot failed:", err);
      loadError.value = String(err);
    }
  }
}

/** 关闭旧会话并重建（覆盖层「新建会话」按钮；attachOnly 模式不可重建）。 */
async function restart() {
  if (props.attachOnly) {
    if (sessionId) {
      await terminalDispose(sessionId).catch(() => {});
    }
  } else {
    await disposeSession(props.sessionKey);
  }
  teardownTerm();
  await boot();
}

watch(
  () => appearance.value?.resolved,
  (value) => {
    if (term && value) applyTheme(term, value);
  },
);

onMounted(() => {
  resizeObserver = new ResizeObserver(() => {
    window.clearTimeout(fitTimer);
    fitTimer = window.setTimeout(fitNow, 100);
  });
  if (host.value) resizeObserver.observe(host.value);
  void boot();
});

onBeforeUnmount(() => {
  window.clearTimeout(fitTimer);
  resizeObserver?.disconnect();
  resizeObserver = null;
  teardownTerm();
});

function focus() {
  term?.focus();
}

defineExpose({ focus, restart });
</script>

<template>
  <div class="terminal-view">
    <div v-if="loadError" class="state" data-scrollbar="thin">
      {{ t("terminal.loadFailed") }}: {{ loadError }}
    </div>
    <template v-else>
      <div ref="host" class="host" />
      <div v-if="exited || sessionClosed" class="overlay">
        <span class="overlay-text">{{
          sessionClosed
            ? t("terminal.sessionClosed")
            : exitCode == null
              ? t("terminal.exitedNoCode")
              : t("terminal.exited", { code: exitCode })
        }}</span>
        <AppButton v-if="!attachOnly" variant="ghost" type="button" @click="restart">
          {{ t("terminal.restart") }}
        </AppButton>
      </div>
    </template>
  </div>
</template>

<style scoped>
.terminal-view {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  background: var(--surface);
}

.host {
  flex: 1;
  min-height: 0;
  padding: var(--space-2);
}

.state {
  padding: var(--space-3);
  font-size: var(--text-sm);
  color: var(--warning, #c47f17);
  word-break: break-all;
}

.overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  background: color-mix(in srgb, var(--surface) 78%, transparent);
  backdrop-filter: blur(2px);
}

.overlay-text {
  font-size: var(--text-sm);
  color: var(--text-muted);
}
</style>
