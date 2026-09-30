<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppConfirm from "@/components/common/AppConfirm.vue";
import ScriptBottomPanel from "@/tools/scripts/ScriptBottomPanel.vue";
import ScriptEditor from "@/tools/scripts/ScriptEditor.vue";
import ScriptMetaPanel from "@/tools/scripts/ScriptMetaPanel.vue";
import ScriptParamForm from "@/tools/scripts/ScriptParamForm.vue";
import ScriptRunBar from "@/tools/scripts/ScriptRunBar.vue";
import { getScript, listScriptVenvs } from "@/api/scripts";
import type { ScriptDto, ScriptVenvSummary } from "@/api/types";
import { useMessage } from "@/composables/useMessage";
import { useWorkbench } from "@/composables/useWorkbench";
import { openChildWindow } from "@/platform/childWindow";
import { usePanelResize } from "@/tools/scripts/usePanelResize";
import { useScriptRun } from "@/tools/scripts/useScriptRun";
import { snapshotKey, useScripts } from "@/tools/scripts/useScripts";
import { formatAppError } from "@/utils/error";

const { t } = useI18n();
const { success, error } = useMessage();
const { mainView } = useWorkbench();

/** 左栏宽度：窄窗安全 clamp（min 不大于 max，避免挤爆 Monaco） */
const LEFT_STORAGE_KEY = "scripts.leftPanelWidth";
const LEFT_HARD_MIN = 260;
const LEFT_HARD_MAX = 560;
const LEFT_SPLITTER_PX = 6;
const mainEl = ref<HTMLElement | null>(null);

function mainWidth(): number {
  return mainEl.value?.clientWidth ?? window.innerWidth;
}

function leftMax(): number {
  const usable = Math.max(0, mainWidth() - LEFT_SPLITTER_PX);
  return Math.max(0, Math.min(LEFT_HARD_MAX, Math.floor(usable * 0.45)));
}

function leftMin(): number {
  return Math.min(LEFT_HARD_MIN, leftMax());
}

const {
  value: leftWidth,
  dragging: leftDragging,
  atMax: leftAtMax,
  onPointerDown: onLeftResizePointerDown,
  onKeydown: onLeftResizeKeydown,
  reclamp: reclampLeft,
} = usePanelResize({
  storageKey: LEFT_STORAGE_KEY,
  axis: "x",
  getMin: leftMin,
  getMax: leftMax,
  defaultValue: () => {
    const usable = Math.max(0, mainWidth() - LEFT_SPLITTER_PX);
    return Math.min(340, Math.floor(usable * 0.38));
  },
});

/** draft 就绪后 .main 才入 DOM，需按实测宽度再 clamp */
watch(mainEl, (el) => {
  if (el) void nextTick(() => reclampLeft());
});

/** 可绑定 venv 列表（命名 venv，供脚本级下拉） */
const venvs = ref<ScriptVenvSummary[]>([]);

async function loadVenvs() {
  try {
    venvs.value = (await listScriptVenvs()).filter((v) => !v.workspace);
  } catch (err) {
    error(formatAppError(err, (key) => t(key)));
  }
}

onMounted(() => {
  void loadVenvs();
});
const {
  current,
  draft,
  dirty,
  paramValues,
  scriptIdNum,
  setParamValue,
  save,
  discardDraft,
  openList,
  persistParamValues,
} = useScripts();
const { running, startedAt, lastResult, run, cancel } = useScriptRun();

const saving = ref(false);
const leaveOpen = ref(false);
const leaveAction = ref<"list" | null>(null);

const leftDraft = computed({
  get: () => draft.value!,
  set: (value) => {
    draft.value = value;
  },
});

function report(err: unknown) {
  error(formatAppError(err, (key) => t(key)));
}

async function onSave(): Promise<boolean> {
  saving.value = true;
  try {
    await save();
    success(t("scripts.saved"));
    return true;
  } catch (err) {
    report(err);
    return false;
  } finally {
    saving.value = false;
  }
}

function requestBack() {
  if (dirty.value) {
    leaveAction.value = "list";
    leaveOpen.value = true;
    return;
  }
  void openList();
}

async function confirmLeaveSave() {
  const action = leaveAction.value;
  leaveOpen.value = false;
  const ok = await onSave();
  if (ok && action === "list") {
    await openList();
  }
  leaveAction.value = null;
}

function confirmLeaveDiscard() {
  leaveOpen.value = false;
  discardDraft();
  if (leaveAction.value === "list") void openList();
  leaveAction.value = null;
}

// 弹窗 Teleport 到 body：离开工具视图（如标题栏返回桌面）时关闭，避免残留在桌面页上方
watch(mainView, (view) => {
  if (view !== "tool") {
    leaveOpen.value = false;
    leaveAction.value = null;
  }
});

async function onRun() {
  if (!draft.value) return;
  try {
    if (dirty.value) {
      saving.value = true;
      try {
        await save();
      } finally {
        saving.value = false;
      }
    }
    const id = scriptIdNum(draft.value.id);
    persistParamValues(id);
    await run(id, { ...paramValues.value });
  } catch (err) {
    report(err);
  }
}

async function onCancel() {
  try {
    await cancel();
  } catch (err) {
    report(err);
  }
}

/** 拖出编辑器子窗口：不强制保存（强制保存制造「主窗干净」假象，回补几字
 *  保存即覆盖子窗成果）；子窗加载已存版本，跨窗冲突由 focus 快照同步守卫。 */
async function onPopOutEditor() {
  if (!draft.value) return;
  try {
    const id = scriptIdNum(draft.value.id);
    await openChildWindow({
      kind: "editor",
      label: `editor-script-${id}`,
      title: draft.value.name,
      params: { scriptId: String(id) },
    });
  } catch (err) {
    report(err);
  }
}

/** 跨窗同步（focus 拉取式）：子窗保存后本窗重新聚焦时快照比对——
 *  非脏静默刷新 draft；脏则三分支确认（不用 updatedAt——SQLite 秒级精度漏检）。 */
const remoteConfirmOpen = ref(false);

async function syncFromRemote() {
  if (saving.value || running.value || !current.value || !draft.value) return;
  try {
    const remote = await getScript(scriptIdNum(current.value.id));
    if (snapshotKey(remote) === snapshotKey(current.value)) return; // 无变化
    if (!dirty.value) {
      current.value = remote;
      draft.value = remote;
      return;
    }
    pendingRemote.value = remote;
    remoteConfirmOpen.value = true;
  } catch {
    /* 脚本可能已在别处删除——保持现状 */
  }
}

const pendingRemote = ref<ScriptDto | null>(null);

function confirmUseRemote() {
  remoteConfirmOpen.value = false;
  const remote = pendingRemote.value;
  pendingRemote.value = null;
  if (remote) {
    current.value = remote;
    draft.value = remote;
  }
}

function confirmKeepLocal() {
  remoteConfirmOpen.value = false;
  const remote = pendingRemote.value;
  pendingRemote.value = null;
  // 保留本地编辑；基准换远端（dirty 继续，下次保存整体覆盖——行为可预期）
  if (remote) current.value = remote;
}

function onWindowFocus() {
  void syncFromRemote();
}

function onVisibilityChange() {
  if (document.visibilityState === "visible") void syncFromRemote();
}

onMounted(() => {
  window.addEventListener("focus", onWindowFocus);
  document.addEventListener("visibilitychange", onVisibilityChange);
});

onBeforeUnmount(() => {
  window.removeEventListener("focus", onWindowFocus);
  document.removeEventListener("visibilitychange", onVisibilityChange);
});

/** 状态文字（未保存 · 计时器）——LogPanel 头部唯一持久状态位 */
const now = ref(Date.now());
let statusTimer: number | undefined;
watch(running, (isRunning) => {
  if (isRunning && statusTimer == undefined) {
    statusTimer = window.setInterval(() => {
      now.value = Date.now();
    }, 1000);
  } else if (!isRunning && statusTimer != undefined) {
    window.clearInterval(statusTimer);
    statusTimer = undefined;
  }
}, { immediate: true });
onBeforeUnmount(() => {
  if (statusTimer != undefined) window.clearInterval(statusTimer);
});

const elapsed = computed(() => {
  if (!running.value || startedAt.value == null) return null;
  return Math.max(0, Math.floor((now.value - startedAt.value) / 1000));
});

const statusText = computed(() => {
  const parts: string[] = [];
  if (dirty.value) parts.push(t("scripts.dirty"));
  if (elapsed.value != null) parts.push(t("scripts.runTimer", { secs: elapsed.value }));
  return parts.join(" · ");
});
</script>

<template>
  <div v-if="draft" class="editor-page">
    <div ref="mainEl" class="main">
      <aside class="left" :style="{ width: `${leftWidth}px` }">
        <ScriptRunBar
          :dirty="dirty"
          :running="running"
          :saving="saving"
          @back="requestBack"
          @save="onSave"
          @run="onRun"
          @cancel="onCancel"
          @popout="onPopOutEditor"
        />
        <div class="left-scroll" data-scrollbar="thin">
          <ScriptMetaPanel v-model:draft="leftDraft" :venvs="venvs" />
          <ScriptParamForm
            :schema="draft.paramsSchema"
            :values="paramValues"
            @change="setParamValue"
          />
        </div>
      </aside>
      <div
        class="left-splitter"
        :class="{ dragging: leftDragging, atMax: leftAtMax }"
        role="separator"
        aria-orientation="vertical"
        tabindex="0"
        :aria-label="t('scripts.leftResize')"
        :aria-valuenow="leftWidth"
        :aria-valuemin="leftMin()"
        :aria-valuemax="leftMax()"
        @pointerdown="onLeftResizePointerDown"
        @keydown="onLeftResizeKeydown"
      />
      <section class="center">
        <ScriptEditor
          :model-value="draft.body"
          :params-schema="draft.paramsSchema"
          @update:model-value="draft.body = $event"
          @save="onSave"
        />
      </section>
    </div>
    <ScriptBottomPanel
      :result="lastResult"
      :running="running"
      :status-text="statusText"
      :status-dirty="dirty"
      :script-id="scriptIdNum(draft.id)"
      :venv-binding="current?.venvName ?? ''"
    />

    <AppConfirm
      :open="leaveOpen"
      :title="t('scripts.unsavedTitle')"
      :message="t('scripts.unsavedMessage')"
      :confirm-label="t('scripts.save')"
      :cancel-label="t('common.cancel')"
      :neutral-label="t('scripts.discard')"
      @confirm="confirmLeaveSave"
      @neutral="confirmLeaveDiscard"
      @cancel="leaveOpen = false; leaveAction = null"
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
.editor-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.main {
  flex: 1;
  min-height: 0;
  display: flex;
  /* LogPanel 拖高钳制的兜底：编辑区内容溢出时裁剪而非压盖日志面板 */
  overflow: hidden;
}

.left {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
  background: var(--surface);
}

/* 竖向分隔条：承担原 .left border-right，避免双线 */
.left-splitter {
  flex-shrink: 0;
  width: 6px;
  margin: 0;
  padding: 0;
  border: 0;
  cursor: col-resize;
  touch-action: none;
  user-select: none;
  outline: none;
  background: color-mix(in srgb, var(--border) 70%, transparent);
}

.left-splitter:hover,
.left-splitter:focus-visible,
.left-splitter.dragging {
  background: color-mix(in srgb, var(--accent) 55%, transparent);
}

.left-splitter.atMax {
  background: color-mix(in srgb, var(--accent) 70%, transparent);
}

.left-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--space-3);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.left :deep(.run-bar) {
  padding: var(--space-2) var(--space-3);
  border-top: 0;
  border-bottom: 1px solid var(--border);
}

.center {
  flex: 1;
  min-width: 0;
  min-height: 0;
}
</style>
