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
  installScriptVenvPackages,
  listScriptVenvs,
  saveScriptsSettings,
} from "@/api/scripts";
import { setSetting } from "@/api/settings";
import type { ScriptRunResult, ScriptVenvSummary } from "@/api/types";
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
/** 当前展开安装面板的 venv 名（null = 无展开） */
const expandedInstall = ref<string | null>(null);
const installPkgs = ref("");
const installReq = ref("");
const installing = ref(false);
const installResult = ref<ScriptRunResult | null>(null);

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

/** 展开/收起某 venv 的安装面板（单展开；收起时清输入） */
function toggleInstall(name: string) {
  if (expandedInstall.value === name) {
    expandedInstall.value = null;
  } else {
    expandedInstall.value = name;
    installPkgs.value = "";
    installReq.value = "";
    installResult.value = null;
  }
}

/** 浏览选择 requirements 文件（仅选择，杜绝相对路径歧义） */
async function pickRequirements() {
  const selected = await open({ multiple: false, filters: [{ name: "requirements", extensions: ["txt"] }] });
  if (typeof selected === "string") {
    installReq.value = selected;
  }
}

/** 执行安装（无取消——pip 中途被杀留半装环境；超时后端兜底） */
async function onInstall() {
  const name = expandedInstall.value;
  if (!name || installing.value) return;
  const pkgs = installPkgs.value.split(/\s+/).map((s) => s.trim()).filter(Boolean);
  const req = installReq.value.trim() || undefined;
  if (!pkgs.length && !req) return;
  installing.value = true;
  installResult.value = null;
  try {
    installResult.value = await installScriptVenvPackages(name, pkgs, req);
    if (installResult.value.exitCode === 0) {
      success(t("scripts.venvInstallDone"));
    }
  } catch (err) {
    report(err);
  } finally {
    installing.value = false;
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
      <div v-for="v in venvs" :key="v.name" class="venv-item">
        <div class="venv-row">
          <div class="venv-main">
            <span class="venv-name">
              {{ v.name }}<template v-if="v.pythonVersion"> · {{ v.pythonVersion }}</template
              ><template v-if="v.workspace">（workspace）</template>
            </span>
            <span class="venv-path">{{ v.pythonPath }}</span>
          </div>
          <div class="row venv-actions">
            <AppButton variant="ghost" type="button" @click="toggleInstall(v.name)">
              {{ t("scripts.venvInstall") }}
            </AppButton>
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
        <div v-if="expandedInstall === v.name" class="install-panel">
          <div class="row">
            <AppInput
              v-model="installPkgs"
              :placeholder="t('scripts.venvInstallPkgs')"
              :disabled="installing"
              class="install-input"
            />
            <AppInput
              :model-value="installReq"
              :placeholder="t('scripts.venvInstallReq')"
              readonly
              class="install-input"
            />
            <AppButton variant="ghost" type="button" :disabled="installing" @click="pickRequirements">
              {{ t("scripts.browse") }}
            </AppButton>
            <AppButton
              variant="primary"
              type="button"
              :disabled="installing || (!installPkgs.trim() && !installReq.trim())"
              @click="onInstall"
            >
              {{ installing ? t("scripts.venvInstalling") : t("scripts.venvInstallRun") }}
            </AppButton>
          </div>
          <p v-if="installResult" class="hint install-command">
            {{ installResult.command }}
          </p>
          <pre
            v-if="installResult"
            class="install-output"
            data-scrollbar="thin"
          >{{ installResult.exitCode === 0 ? "" : t("scripts.exitCode", { code: installResult.exitCode ?? "—" }) + "\n\n—— stdout ——\n" }}{{ installResult.stdout || (installResult.exitCode === 0 ? "(empty)" : "") }}{{ installResult.stderr ? "\n\n—— stderr ——\n" + installResult.stderr : "" }}</pre>
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

.venv-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.venv-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.venv-actions {
  flex-wrap: wrap;
  justify-content: flex-end;
}

.install-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  border-top: 1px dashed var(--border);
}

.install-input {
  min-width: 140px;
}

.install-command {
  margin: 0;
  font-size: var(--text-xs, 12px);
  word-break: break-all;
}

.install-output {
  margin: 0;
  max-height: 240px;
  overflow: auto;
  padding: var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
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
