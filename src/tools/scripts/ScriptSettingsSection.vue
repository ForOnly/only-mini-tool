<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppConfirm from "@/components/common/AppConfirm.vue";
import AppInput from "@/components/common/AppInput.vue";
import {
  createScriptVenv,
  createScriptsVenv,
  deleteScriptVenv,
  getScriptsSettings,
  listScriptVenvs,
  saveScriptsSettings,
} from "@/api/scripts";
import { setSetting } from "@/api/settings";
import type { ScriptVenvSummary } from "@/api/types";
import { useMessage } from "@/composables/useMessage";
import { formatAppError } from "@/utils/error";

const { t } = useI18n();
const { success, error } = useMessage();

const pythonPath = ref("python");
const defaultWorkspace = ref("");
const envPrefix = ref("PARAM_");
const activeVenv = ref("");
const env = ref<{ key: string; value: string }[]>([]);
const busy = ref(false);
const venvCreating = ref(false);
const venvs = ref<ScriptVenvSummary[]>([]);
const newVenvName = ref("");
const deleteVenvName = ref<string | null>(null);

const envMap = computed(() => {
  const out: Record<string, string> = {};
  for (const row of env.value) {
    const k = row.key.trim();
    if (k) out[k] = row.value;
  }
  return out;
});

/** 细节版错误展示：泛化文案 + 原始 message（后端细节如 exit code/stderr/校验规则）。
 * formatAppError 命中 i18n code 键时只返回泛化文案，会吞掉细节——此处组合展示。 */
function report(err: unknown) {
  const localized = formatAppError(err, (key) => t(key));
  const raw =
    err && typeof err === "object" && "message" in err
      ? String((err as { message?: unknown }).message ?? "")
      : "";
  const detail = raw.length > 300 ? `${raw.slice(0, 300)}…` : raw;
  error(detail && detail !== localized ? `${localized} — ${detail}` : localized);
}

async function loadVenvs() {
  venvs.value = await listScriptVenvs();
}

async function load() {
  busy.value = true;
  try {
    const bundle = await getScriptsSettings();
    pythonPath.value = bundle.pythonPath || "python";
    defaultWorkspace.value = bundle.defaultWorkspace || "";
    envPrefix.value = bundle.envPrefix || "PARAM_";
    activeVenv.value = bundle.venv || "";
    env.value = Object.entries(bundle.env ?? {}).map(([key, value]) => ({
      key,
      value: value ?? "",
    }));
    await loadVenvs();
  } finally {
    busy.value = false;
  }
}

async function onSave() {
  busy.value = true;
  try {
    await saveScriptsSettings({
      pythonPath: pythonPath.value.trim() || "python",
      defaultWorkspace: defaultWorkspace.value.trim(),
      envPrefix: envPrefix.value.trim() || "PARAM_",
      venv: activeVenv.value,
      env: envMap.value,
    });
    success(t("scripts.settingsSaved"));
  } catch (err) {
    report(err);
  } finally {
    busy.value = false;
  }
}

/** 设为默认：单键写入，不连带表单中未保存的其他字段 */
async function onSetDefaultVenv(name: string) {
  const next = activeVenv.value === name ? "" : name;
  try {
    await setSetting("scripts.venv", next);
    activeVenv.value = next;
  } catch (err) {
    report(err);
  }
}

async function onCreateVenv() {
  const name = newVenvName.value.trim();
  if (!name) return;
  venvCreating.value = true;
  try {
    await createScriptVenv(name);
    newVenvName.value = "";
    await loadVenvs();
    success(t("scripts.venvCreated"));
  } catch (err) {
    report(err);
  } finally {
    venvCreating.value = false;
  }
}

/** workspace `.venv` 快捷创建（等价列表中该条目的创建） */
async function onCreateWorkspaceVenv() {
  venvCreating.value = true;
  try {
    await createScriptsVenv();
    await loadVenvs();
    success(t("scripts.venvCreated"));
  } catch (err) {
    report(err);
  } finally {
    venvCreating.value = false;
  }
}

async function confirmDeleteVenv() {
  const name = deleteVenvName.value;
  deleteVenvName.value = null;
  if (name == null) return;
  try {
    await deleteScriptVenv(name);
    if (activeVenv.value === name) activeVenv.value = "";
    await loadVenvs();
    success(t("scripts.venvDeleted"));
  } catch (err) {
    report(err);
  }
}

async function pickWorkspace() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    defaultWorkspace.value = selected;
  }
}

onMounted(() => {
  void load().catch(report);
});
</script>

<template>
  <section class="section">
    <h2>{{ t("scripts.settingsSection") }}</h2>
    <label class="field">
      <span>{{ t("scripts.pythonPath") }}</span>
      <AppInput v-model="pythonPath" :disabled="busy" />
      <span class="hint">{{ t("scripts.pythonPathHint") }}</span>
    </label>
    <label class="field">
      <span>{{ t("scripts.defaultWorkspace") }}</span>
      <div class="row">
        <AppInput v-model="defaultWorkspace" :disabled="busy" />
        <AppButton variant="ghost" type="button" :disabled="busy" @click="pickWorkspace">
          {{ t("scripts.browse") }}
        </AppButton>
      </div>
    </label>
    <label class="field">
      <span>{{ t("scripts.envPrefix") }}</span>
      <AppInput v-model="envPrefix" :disabled="busy" :placeholder="'PARAM_'" />
      <span class="hint">{{ t("scripts.envPrefixHint") }}</span>
    </label>
    <div class="block">
      <div class="block-head">
        <h3>{{ t("scripts.globalEnv") }}</h3>
        <AppButton
          variant="ghost"
          type="button"
          @click="env.push({ key: '', value: '' })"
        >
          {{ t("scripts.addEnv") }}
        </AppButton>
      </div>
      <div v-for="(row, index) in env" :key="index" class="row">
        <AppInput v-model="row.key" :placeholder="t('scripts.envKey')" />
        <AppInput v-model="row.value" :placeholder="t('scripts.envValue')" />
        <AppButton variant="ghost" type="button" @click="env.splice(index, 1)">
          {{ t("scripts.remove") }}
        </AppButton>
      </div>
    </div>
    <div class="block">
      <div class="block-head">
        <h3>{{ t("scripts.venvTitle") }}</h3>
        <div class="row">
          <AppInput
            v-model="newVenvName"
            :placeholder="t('scripts.venvNamePlaceholder')"
            :disabled="venvCreating"
            class="venv-name-input"
            @keydown.enter.prevent="onCreateVenv"
          />
          <AppButton
            variant="ghost"
            type="button"
            :disabled="venvCreating || !newVenvName.trim()"
            @click="onCreateVenv"
          >
            {{ venvCreating ? t("scripts.venvCreating") : t("scripts.venvCreate") }}
          </AppButton>
        </div>
      </div>
      <p v-if="!venvs.length" class="hint">{{ t("scripts.venvNotCreated") }}</p>
      <AppButton
        v-if="defaultWorkspace.trim() && !venvs.some((v) => v.workspace)"
        variant="ghost"
        type="button"
        :disabled="venvCreating"
        @click="onCreateWorkspaceVenv"
      >
        {{ venvCreating ? t("scripts.venvCreating") : t("scripts.venvCreateWorkspace") }}
      </AppButton>
      <div v-for="v in venvs" :key="v.name" class="venv-row">
        <div class="venv-main">
          <span class="venv-name">
            {{ v.name }}<template v-if="v.pythonVersion"> · {{ v.pythonVersion }}</template
            ><template v-if="v.workspace">（workspace）</template>
          </span>
          <span class="venv-path">{{ v.pythonPath }}</span>
        </div>
        <div class="row">
          <AppButton
            v-if="!v.workspace"
            variant="ghost"
            type="button"
            @click="onSetDefaultVenv(v.name)"
          >
            {{
              activeVenv === v.name
                ? t("scripts.venvDisableDefault")
                : t("scripts.venvSetDefault")
            }}
          </AppButton>
          <AppButton variant="ghost" type="button" @click="deleteVenvName = v.name">
            {{ t("scripts.venvDelete") }}
          </AppButton>
        </div>
      </div>
      <p class="hint">{{ t("scripts.venvHint") }}</p>
      <p class="hint">
        <template v-if="activeVenv">
          {{ t("scripts.venvActive", { name: activeVenv }) }}
        </template>
        <template v-else>{{ t("scripts.venvNoneActive") }}</template>
      </p>
    </div>
    <AppButton variant="primary" :disabled="busy" @click="onSave">
      {{ t("scripts.saveSettings") }}
    </AppButton>

    <AppConfirm
      :open="deleteVenvName != null"
      :title="t('scripts.venvDeleteTitle')"
      :message="t('scripts.venvDeleteMessage', { name: deleteVenvName ?? '' })"
      :confirm-label="t('scripts.venvDelete')"
      :cancel-label="t('common.cancel')"
      danger
      @confirm="confirmDeleteVenv"
      @cancel="deleteVenvName = null"
    />
  </section>
</template>

<style scoped>
.section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.section h2 {
  margin: 0;
  font-size: var(--text-lg);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--text-sm);
  color: var(--text-muted);
}

.row {
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

.row > :deep(.app-input) {
  flex: 1;
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.block-head h3 {
  margin: 0;
  font-size: var(--text-md);
  color: var(--text);
}

.venv-name-input {
  min-width: 160px;
}

.venv-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.venv-main {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.venv-name {
  font-size: var(--text-sm);
  font-weight: 600;
}

.venv-path {
  font-size: var(--text-xs, 12px);
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hint {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-muted);
  word-break: break-all;
}
</style>
