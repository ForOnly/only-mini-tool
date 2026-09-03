<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { openDevtools } from "@/api/debug";
import { clearAppLog, getLogsDir, openLogsDir, readAppLog } from "@/api/logs";
import AppButton from "@/components/common/AppButton.vue";
import { useDebug } from "@/composables/useDebug";
import { useMessage } from "@/composables/useMessage";
import { formatAppError } from "@/utils/error";

const { t } = useI18n();
const { debugEnabled, setDebugEnabled } = useDebug();
const { error } = useMessage();

const logText = ref("");
const logsPath = ref("");
const debugHint = ref<string | null>(null);
const busy = ref(false);

function report(err: unknown) {
  error(formatAppError(err, (key) => t(key)));
}

onMounted(() => {
  if (debugEnabled.value) {
    debugHint.value = t("settings.debugRestartHint");
    void refreshLogs();
  }
});

async function onDebugToggle(event: Event) {
  const checked = (event.target as HTMLInputElement).checked;
  try {
    await setDebugEnabled(checked);
    debugHint.value = checked ? t("settings.debugRestartHint") : null;
    if (checked) {
      await refreshLogs();
    } else {
      logText.value = "";
      logsPath.value = "";
    }
  } catch (err) {
    report(err);
  }
}

async function refreshLogs() {
  if (!debugEnabled.value) {
    return;
  }
  busy.value = true;
  try {
    logText.value = await readAppLog();
    logsPath.value = await getLogsDir();
  } catch (err) {
    report(err);
  } finally {
    busy.value = false;
  }
}

async function onClearLogs() {
  if (!debugEnabled.value) {
    return;
  }
  busy.value = true;
  try {
    await clearAppLog();
    await refreshLogs();
  } catch (err) {
    report(err);
  } finally {
    busy.value = false;
  }
}

async function onOpenLogsDir() {
  try {
    await openLogsDir();
  } catch (err) {
    report(err);
  }
}

async function onOpenDevtools() {
  busy.value = true;
  try {
    await openDevtools();
  } catch (err) {
    report(err);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="debug-section">
    <label class="field check">
      <input type="checkbox" :checked="debugEnabled" @change="onDebugToggle" />
      <span>{{ t("settings.debugEnable") }}</span>
    </label>
    <p v-if="debugHint" class="hint">{{ debugHint }}</p>

    <div v-if="debugEnabled" class="debug-panel">
      <div class="debug-head">
        <h2>{{ t("settings.logsTitle") }}</h2>
        <div class="actions">
          <AppButton variant="ghost" :disabled="busy" @click="onOpenDevtools">
            {{ t("settings.debugOpenDevtools") }}
          </AppButton>
          <AppButton variant="ghost" :disabled="busy" @click="refreshLogs">
            {{ t("settings.logsRefresh") }}
          </AppButton>
          <AppButton variant="ghost" :disabled="busy" @click="onOpenLogsDir">
            {{ t("settings.logsOpenDir") }}
          </AppButton>
          <AppButton variant="ghost" :disabled="busy" @click="onClearLogs">
            {{ t("settings.logsClear") }}
          </AppButton>
        </div>
      </div>
      <p v-if="logsPath" class="path">{{ logsPath }}</p>
      <pre class="log">{{ logText || t("settings.logsEmpty") }}</pre>
    </div>
  </div>
</template>

<style scoped>
.field.check {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}

.hint {
  margin: calc(var(--space-2) * -1) 0 var(--space-4);
  color: var(--text-muted);
  font-size: var(--text-md);
  line-height: 1.45;
}

.debug-panel {
  margin-top: var(--space-2);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.debug-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  flex-wrap: wrap;
}

h2 {
  margin: 0;
  font-size: var(--text-base);
  font-weight: var(--font-weight-title);
  color: var(--text);
}

.actions {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.path {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--text-muted);
  word-break: break-all;
}

.log {
  margin: 0;
  max-height: 320px;
  overflow: auto;
  padding: var(--space-3);
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--surface);
  font-size: var(--text-xs);
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
