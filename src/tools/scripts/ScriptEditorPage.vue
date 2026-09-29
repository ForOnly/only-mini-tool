<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppConfirm from "@/components/common/AppConfirm.vue";
import ScriptEditor from "@/tools/scripts/ScriptEditor.vue";
import ScriptLogPanel from "@/tools/scripts/ScriptLogPanel.vue";
import ScriptMetaPanel from "@/tools/scripts/ScriptMetaPanel.vue";
import ScriptParamForm from "@/tools/scripts/ScriptParamForm.vue";
import ScriptRunBar from "@/tools/scripts/ScriptRunBar.vue";
import { listScriptVenvs } from "@/api/scripts";
import type { ScriptVenvSummary } from "@/api/types";
import { useMessage } from "@/composables/useMessage";
import { useWorkbench } from "@/composables/useWorkbench";
import { useScriptRun } from "@/tools/scripts/useScriptRun";
import { useScripts } from "@/tools/scripts/useScripts";
import { formatAppError } from "@/utils/error";

const { t } = useI18n();
const { success, error } = useMessage();
const { mainView } = useWorkbench();

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
    <div class="main">
      <aside class="left">
        <ScriptRunBar
          :dirty="dirty"
          :running="running"
          :saving="saving"
          @back="requestBack"
          @save="onSave"
          @run="onRun"
          @cancel="onCancel"
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
      <section class="center">
        <ScriptEditor
          :model-value="draft.body"
          @update:model-value="draft.body = $event"
        />
      </section>
    </div>
    <ScriptLogPanel
      :result="lastResult"
      :running="running"
      :status-text="statusText"
      :status-dirty="dirty"
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
  width: min(340px, 38%);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--surface);
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
