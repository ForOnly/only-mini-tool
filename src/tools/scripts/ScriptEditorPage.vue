<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppConfirm from "@/components/common/AppConfirm.vue";
import ScriptEditor from "@/tools/scripts/ScriptEditor.vue";
import ScriptLogPanel from "@/tools/scripts/ScriptLogPanel.vue";
import ScriptMetaPanel from "@/tools/scripts/ScriptMetaPanel.vue";
import ScriptParamForm from "@/tools/scripts/ScriptParamForm.vue";
import ScriptRunBar from "@/tools/scripts/ScriptRunBar.vue";
import { useMessage } from "@/composables/useMessage";
import { useScriptRun } from "@/tools/scripts/useScriptRun";
import { useScripts } from "@/tools/scripts/useScripts";
import { formatAppError } from "@/utils/error";

const { t } = useI18n();
const { success, error } = useMessage();
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
</script>

<template>
  <div v-if="draft" class="editor-page">
    <div class="main">
      <aside class="left">
        <ScriptRunBar
          :dirty="dirty"
          :running="running"
          :saving="saving"
          :run-started-at="startedAt"
          @back="requestBack"
          @save="onSave"
          @run="onRun"
          @cancel="onCancel"
        />
        <div class="left-scroll" data-scrollbar="thin">
          <ScriptMetaPanel v-model:draft="leftDraft" />
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
    <ScriptLogPanel :result="lastResult" :running="running" />

    <AppConfirm
      :open="leaveOpen"
      :title="t('scripts.unsavedTitle')"
      :message="t('scripts.unsavedMessage')"
      :confirm-label="t('scripts.save')"
      :cancel-label="t('common.cancel')"
      @confirm="confirmLeaveSave"
      @cancel="leaveOpen = false; leaveAction = null"
    >
      <AppButton variant="ghost" type="button" @click="confirmLeaveDiscard">
        {{ t("scripts.discard") }}
      </AppButton>
    </AppConfirm>
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
}

.left {
  width: min(340px, 38%);
  min-width: 260px;
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
