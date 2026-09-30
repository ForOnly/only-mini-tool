<script setup lang="ts">
/** 终端单会话渲染核心：懒加载 xterm → 创建/附着会话 → 双向流。
 *  生命周期语义（D3）：unmount 只 detach（会话保留，重挂回放）；
 *  显式关闭（disposeSession/restart）才销毁会话。 */

import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Channel } from "@tauri-apps/api/core";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";

import AppButton from "@/components/common/AppButton.vue";
import AppContextMenu from "@/components/common/AppContextMenu.vue";
import type { ContextMenuItem } from "@/components/common/contextMenuTypes";
import { terminalAttach, terminalDetach, terminalDispose, terminalResize, terminalWrite } from "@/api/terminal";
import type { ColorScheme, TerminalCreatePayload, TerminalEvent } from "@/api/types";
import { useAppearance } from "@/composables/useAppearance";
import { useTerminalSessions } from "@/components/terminal/useTerminalSessions";
import { applyTheme } from "@/components/terminal/terminalTheme";
import { loadXterm } from "@/components/terminal/xtermLoader";
import { formatAppError } from "@/utils/error";

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
const { ensureSession, markExited, reapSession, scheduleExitCleanup, disposeSession } =
  useTerminalSessions();

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
/** boot 未完成即 unmount（视图早夭）——创建出的会话无人附着，延迟回收 */
let bootCancelled = false;

function scheme(): ColorScheme {
  return appearance.value?.resolved ?? "light";
}

function onEvent(event: TerminalEvent) {
  if (event.kind === "replay" || event.kind === "output") {
    term?.write(event.data);
  } else if (event.kind === "resize") {
    // 采纳式同步：对齐共享 PTY 的尺寸且不回发（防乒乓；fitNow 差异比对天然兼容）
    if (event.cols != null && event.rows != null && term) {
      try {
        term.resize(event.cols, event.rows);
        lastCols = event.cols;
        lastRows = event.rows;
      } catch {
        // 布局未就绪——下次 fit 校正
      }
    }
  } else if (event.kind === "exit") {
    exited.value = true;
    exitCode.value = event.exitCode ?? null;
    if (sessionId) {
      // markExited 内部调度 Exit 后 10s 自动回收（退出的 shell 不常驻 conhost）
      markExited(sessionId, exitCode.value);
    }
    emit("exited", exitCode.value);
  }
}

/** 剪贴板读取（WebView2 权限被拒时告警——降级路径见右键菜单提示）。 */
async function readClipboard(): Promise<string | null> {
  try {
    return await navigator.clipboard.readText();
  } catch {
    console.warn("[terminal] clipboard read denied");
    return null;
  }
}

async function copySelection(terminal: Terminal) {
  const text = terminal.getSelection();
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    /* 剪贴板不可用时静默 */
  }
  terminal.clearSelection();
}

async function pasteFromClipboard(terminal: Terminal) {
  const text = await readClipboard();
  if (text) terminal.paste(text);
}

/** 智能复制粘贴：Ctrl+C 有选区=复制（无选区=放行发 ^C 中断）；Ctrl+V 粘贴。 */
function attachKeyHandlers(terminal: Terminal) {
  terminal.attachCustomKeyEventHandler((event: KeyboardEvent) => {
    if (event.type !== "keydown") return true;
    const ctrl = event.ctrlKey || event.metaKey;
    if (!ctrl) return true;
    const key = event.key.toLowerCase();
    if (key === "c" && !event.shiftKey) {
      if (terminal.hasSelection()) {
        void copySelection(terminal);
        return false;
      }
      return true;
    }
    if (key === "v") {
      void pasteFromClipboard(terminal);
      return false;
    }
    return true;
  });
}

/** 终端右键菜单（全局 contextmenu 守卫在容器层 stop + 自有菜单）。 */
const menuOpen = ref(false);
const menuX = ref(0);
const menuY = ref(0);

const menuItems = computed<ContextMenuItem[]>(() => [
  { id: "copy", label: t("terminal.copy"), disabled: !term?.hasSelection() },
  { id: "paste", label: t("terminal.paste") },
  { id: "clear", label: t("terminal.clear") },
]);

function onContextMenu(event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  menuX.value = event.clientX;
  menuY.value = event.clientY;
  menuOpen.value = true;
}

function onMenuSelect(id: string) {
  if (!term) return;
  if (id === "copy") void copySelection(term);
  else if (id === "paste") void pasteFromClipboard(term);
  else if (id === "clear") term.clear();
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
  // boot 未完成即卸载（视图早夭）：让在途 boot 在 ensureSession 后走回收分支
  if (!attachId) bootCancelled = true;
  attachId = null;
  dataDisposable?.dispose();
  dataDisposable = null;
  term?.dispose();
  term = null;
  fit = null;
}

async function boot() {
  if (!host.value) return;
  bootCancelled = false;
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
    if (bootCancelled) {
      // 视图已卸载且无人附着——延迟回收防孤儿（同键复用者会先 dispose）
      scheduleExitCleanup(id, 5_000);
      sessionId = id;
      teardownTerm();
      return;
    }
    sessionId = id;
    attachKeyHandlers(terminal);
    const ch = new Channel<TerminalEvent>();
    ch.onmessage = onEvent;
    attachId = await terminalAttach(id, ch);
    dataDisposable = terminal.onData((data) => {
      if (exited.value || sessionClosed.value) return; // 终态短路（防 write 失败刷警告）
      void terminalWrite(id, data).catch((err) => console.warn("[terminal] write failed:", err));
    });
    emit("session", id);
    fitNow();
    terminal.focus();
  } catch (err) {
    const code = (err as { code?: string })?.code;
    if (code === "terminal.not_found") {
      // 会话已被销毁（如拖出窗口后关闭）——清死键后覆盖层展示，restart 重建
      if (sessionId) reapSession(sessionId);
      sessionClosed.value = true;
    } else {
      console.error("[terminal] boot failed:", err);
      loadError.value = formatAppError(err, (key) => t(key));
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

/** 加载失败重试（boot 非破坏性重启——无需销毁会话）。 */
function retry() {
  void boot();
}

defineExpose({ focus, restart });
</script>

<template>
  <div class="terminal-view" @contextmenu="onContextMenu">
    <div v-if="loadError" class="state" data-scrollbar="thin">
      <span>{{ loadError }}</span>
      <AppButton variant="ghost" type="button" @click="retry">
        {{ t("terminal.retry") }}
      </AppButton>
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
    <AppContextMenu
      :open="menuOpen"
      :x="menuX"
      :y="menuY"
      :items="menuItems"
      @close="menuOpen = false"
      @select="onMenuSelect"
    />
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
  /* padding 在容器层：fit-addon 量测的 .host 不含 padding，列数不再偏大 */
  padding: var(--space-2);
  background: var(--surface);
}

.host {
  flex: 1;
  min-height: 0;
}

.state {
  display: flex;
  align-items: center;
  gap: var(--space-2);
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
