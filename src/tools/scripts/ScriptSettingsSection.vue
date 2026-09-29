<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppInput from "@/components/common/AppInput.vue";
import {
  createScriptsVenv,
  getScriptsSettings,
  getScriptsVenvStatus,
  saveScriptsSettings,
} from "@/api/scripts";
import { useMessage } from "@/composables/useMessage";
import { formatAppError } from "@/utils/error";

const { t } = useI18n();
const { success, error } = useMessage();

const pythonPath = ref("python");
const defaultWorkspace = ref("");
const envPrefix = ref("PARAM_");
const env = ref<{ key: string; value: string }[]>([]);
const busy = ref(false);
const venvCreating = ref(false);
const venvStatus = ref<{ workspace: string; venvPython: string | null } | null>(null);

const envMap = computed(() => {
  const out: Record<string, string> = {};
  for (const row of env.value) {
    const k = row.key.trim();
    if (k) out[k] = row.value;
  }
  return out;
});

function report(err: unknown) {
  error(formatAppError(err, (key) => t(key)));
}

async function load() {
  busy.value = true;
  try {
    const bundle = await getScriptsSettings();
    pythonPath.value = bundle.pythonPath || "python";
    defaultWorkspace.value = bundle.defaultWorkspace || "";
    envPrefix.value = bundle.envPrefix || "PARAM_";
    env.value = Object.entries(bundle.env ?? {}).map(([key, value]) => ({
      key,
      value: value ?? "",
    }));
    venvStatus.value = await getScriptsVenvStatus();
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
      env: envMap.value,
    });
    success(t("scripts.settingsSaved"));
  } catch (err) {
    report(err);
  } finally {
    busy.value = false;
  }
}

async function onCreateVenv() {
  venvCreating.value = true;
  try {
    await createScriptsVenv();
    venvStatus.value = await getScriptsVenvStatus();
    success(t("scripts.venvCreated"));
  } catch (err) {
    report(err);
  } finally {
    venvCreating.value = false;
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
        <AppButton
          variant="ghost"
          type="button"
          :disabled="busy || venvCreating || !defaultWorkspace.trim()"
          @click="onCreateVenv"
        >
          {{ venvCreating ? t("scripts.venvCreating") : t("scripts.venvCreate") }}
        </AppButton>
      </div>
      <p class="hint">
        <template v-if="venvStatus?.venvPython">
          {{ t("scripts.venvReady", { path: venvStatus.venvPython }) }}
        </template>
        <template v-else-if="venvStatus && !venvStatus.workspace">
          {{ t("scripts.venvNeedWorkspace") }}
        </template>
        <template v-else>
          {{ t("scripts.venvNotCreated") }}
        </template>
      </p>
      <p class="hint">{{ t("scripts.venvHint") }}</p>
    </div>
    <AppButton variant="primary" :disabled="busy" @click="onSave">
      {{ t("scripts.saveSettings") }}
    </AppButton>
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
}

.block-head h3 {
  margin: 0;
  font-size: var(--text-md);
  color: var(--text);
}

.hint {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-muted);
  word-break: break-all;
}
</style>
