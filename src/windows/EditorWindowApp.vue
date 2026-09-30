<script setup lang="ts">
/** 脚本编辑器子窗口：按脚本 ID 独立加载/编辑/保存（复用通用 CodeEditor +
 *  python 领域 providers——子窗为独立 JS 上下文，各自注册一次天然幂等）。
 *  已知限制（last-save-wins）：与主窗并行编辑同一脚本，后保存者胜出，无跨窗同步。
 *  dirty 关窗三分支确认；主题 3s 轮询跟随主窗。 */

import { computed, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppConfirm from "@/components/common/AppConfirm.vue";
import CodeEditor from "@/components/editor/CodeEditor.vue";
import { loadMonaco } from "@/components/editor/monacoLoader";
import { getScript, updateScript } from "@/api/scripts";
import type { ScriptDto, ScriptUpdate } from "@/api/types";
import { useAppearance } from "@/composables/useAppearance";
import { useMessage } from "@/composables/useMessage";
import { formatAppError } from "@/utils/error";
import { registerPythonProviders, setParamsSchema } from "@/tools/scripts/monacoPython";
import { snapshotKey } from "@/tools/scripts/useScripts";
import ChildWindowTitleBar from "@/windows/ChildWindowTitleBar.vue";

const props = defineProps<{
  scriptId: number;
}>();

const { t } = useI18n();
const { success, error } = useMessage();
const { refresh: refreshAppearance } = useAppearance();

const script = ref<ScriptDto | null>(null);
const body = ref("");
const savedBody = ref("");
const saving = ref(false);
const loadError = ref<string | null>(null);
const closeConfirm = ref(false);

const dirty = computed(() => body.value !== savedBody.value);

let unlistenClose: (() => void) | null = null;
let themeTimer: number | undefined;

function report(err: unknown) {
  error(formatAppError(err, (key) => t(key)));
}

/** 全字段投影（以加载时 Dto 为基准，仅覆写 body——名称等字段改动走主窗）。 */
function toUpdate(s: ScriptDto, nextBody: string): ScriptUpdate {
  return {
    name: s.name,
    description: s.description,
    body: nextBody,
    workspacePath: s.workspacePath,
    venvName: s.venvName,
    env: s.env ?? {},
    paramsSchema: s.paramsSchema ?? [],
    argsTemplate: s.argsTemplate,
  };
}

async function load() {
  try {
    const s = await getScript(props.scriptId);
    script.value = s;
    body.value = s.body;
    savedBody.value = s.body;
    setParamsSchema(s.paramsSchema ?? []);
    const m = await loadMonaco();
    registerPythonProviders(m, (err) => error(formatAppError(err, (key) => t(key))));
  } catch (err) {
    loadError.value = formatAppError(err, (key) => t(key));
  }
}

async function save(): Promise<boolean> {
  if (!script.value || saving.value) return false;
  saving.value = true;
  const sentBody = body.value;
  try {
    const updated = await updateScript(props.scriptId, toUpdate(script.value, sentBody));
    script.value = updated;
    savedBody.value = updated.body;
    // 保存窗口期有并发键入则保留本地内容（保持 dirty）；无键入才回写
    if (body.value === sentBody) {
      body.value = updated.body;
    }
    success(t("scripts.saved"));
    return true;
  } catch (err) {
    report(err);
    return false;
  } finally {
    saving.value = false;
  }
}

/** 跨窗同步（focus 拉取式）：主窗保存后本窗重新聚焦时快照比对——
 *  非脏静默刷新；脏则三分支确认（不用 updatedAt——SQLite 秒级精度会漏检）。 */
const remoteConfirmOpen = ref(false);

async function syncFromRemote() {
  if (saving.value || !script.value) return;
  try {
    const remote = await getScript(props.scriptId);
    if (snapshotKey(remote) === snapshotKey(script.value)) return; // 无变化
    const dirtyNow = body.value !== savedBody.value;
    if (!dirtyNow) {
      script.value = remote;
      body.value = remote.body;
      savedBody.value = remote.body;
      return;
    }
    pendingRemote.value = remote;
    remoteConfirmOpen.value = true;
  } catch {
    /* 主窗可能已删除该脚本——保持现状 */
  }
}

const pendingRemote = ref<ScriptDto | null>(null);

function confirmUseRemote() {
  remoteConfirmOpen.value = false;
  const remote = pendingRemote.value;
  pendingRemote.value = null;
  if (remote) {
    script.value = remote;
    body.value = remote.body;
    savedBody.value = remote.body;
  }
}

function confirmKeepLocal() {
  remoteConfirmOpen.value = false;
  const remote = pendingRemote.value;
  pendingRemote.value = null;
  // 保留本地编辑；基准换远端（dirty 继续，下次保存整体覆盖——行为可预期）
  if (remote) {
    script.value = remote;
    savedBody.value = remote.body;
  }
}

function onWindowFocus() {
  void syncFromRemote();
}

function onVisibilityChange() {
  if (document.visibilityState === "visible") void syncFromRemote();
}

onMounted(async () => {
  await load();
  const win = getCurrentWindow();
  unlistenClose = await win.onCloseRequested(async (event) => {
    if (!dirty.value) return; // 干净直接关（不拦截）
    await event.preventDefault();
    closeConfirm.value = true;
  });
  window.addEventListener("focus", onWindowFocus);
  document.addEventListener("visibilitychange", onVisibilityChange);
  themeTimer = window.setInterval(() => {
    void refreshAppearance();
  }, 3000);
});

onUnmounted(() => {
  unlistenClose?.();
  window.removeEventListener("focus", onWindowFocus);
  document.removeEventListener("visibilitychange", onVisibilityChange);
  if (themeTimer != undefined) window.clearInterval(themeTimer);
});

async function confirmCloseSave() {
  closeConfirm.value = false;
  if (await save()) {
    await getCurrentWindow().destroy();
  }
}

function confirmCloseDiscard() {
  closeConfirm.value = false;
  void getCurrentWindow().destroy();
}
</script>

<template>
  <div class="editor-window">
    <ChildWindowTitleBar :title="script?.name ?? t('editor.windowTitle')" />
    <header class="toolbar">
      <span class="name">{{ script?.name ?? "" }}</span>
      <span
        v-if="dirty"
        class="dirty-dot"
        role="status"
        :aria-label="t('scripts.dirty')"
        :title="t('scripts.dirty')"
      />
      <div class="spacer" />
      <AppButton variant="ghost" type="button" :disabled="saving || !dirty" @click="save">
        {{ t("scripts.save") }}
      </AppButton>
    </header>
    <main class="body">
      <div v-if="loadError" class="state" data-scrollbar="thin">{{ loadError }}</div>
      <!-- script 就绪才挂载：create 初值即真实 body，undo 栈不留「空→全文」步
           （一次 Ctrl+Z 清空整个脚本的竞态根除） -->
      <div v-else-if="!script" class="state">{{ t("terminal.loading") }}</div>
      <CodeEditor
        v-else
        :model-value="body"
        language="python"
        @update:model-value="body = $event"
        @save="save"
      />
    </main>

    <AppConfirm
      :open="closeConfirm"
      :title="t('scripts.unsavedTitle')"
      :message="t('scripts.unsavedMessage')"
      :confirm-label="t('scripts.save')"
      :cancel-label="t('common.cancel')"
      :neutral-label="t('scripts.discard')"
      @confirm="confirmCloseSave"
      @neutral="confirmCloseDiscard"
      @cancel="closeConfirm = false"
    />

    <AppConfirm
      :open="remoteConfirmOpen"
      :title="t('scripts.remoteUpdated')"
      :message="t('scripts.remoteUpdatedMessage')"
      :confirm-label="t('scripts.useRemote')"
      :cancel-label="t('common.cancel')"
      :neutral-label="t('scripts.keepLocal')"
      @confirm="confirmUseRemote"
      @neutral="confirmKeepLocal"
      @cancel="remoteConfirmOpen = false"
    />
  </div>
</template>

<style scoped>
.editor-window {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  background: var(--bg);
}

.toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}

.name {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dirty-dot {
  width: 8px;
  height: 8px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--warning, #d97706);
}

.spacer {
  flex: 1;
}

.body {
  flex: 1;
  min-height: 0;
}

.state {
  padding: var(--space-4);
  font-size: var(--text-sm);
  color: var(--warning, #c47f17);
  word-break: break-all;
}
</style>
