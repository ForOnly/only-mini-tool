<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppInput from "@/components/common/AppInput.vue";
import { useMessage } from "@/composables/useMessage";
import { useOcrSettings } from "@/tools/ocr/useOcrSettings";
import { formatAppError } from "@/utils/error";

const { t, te } = useI18n();
const { success, error } = useMessage();
const {
  engines,
  values,
  activeEngine,
  activeEngineInfo,
  timeoutMs,
  inspectorPlacement,
  busy,
  load,
  save,
  fieldLabelKey,
  engineLabelKey,
} = useOcrSettings();

const activeFields = computed(() => activeEngineInfo.value?.fields ?? []);

function report(err: unknown) {
  error(formatAppError(err, (key) => t(key)));
}

function labelForEngine(id: string): string {
  const key = engineLabelKey(id);
  return te(key) ? t(key) : id;
}

function labelForField(name: string): string {
  const key = fieldLabelKey(name);
  return te(key) ? t(key) : name;
}

function inputType(kind: string): "text" | "password" | "url" {
  if (kind === "password") return "password";
  if (kind === "url") return "url";
  return "text";
}

onMounted(() => {
  void load().catch(report);
});

async function onSave() {
  try {
    await save();
    success(t("settings.ocrSaved"));
  } catch (err) {
    report(err);
  }
}
</script>

<template>
  <section class="section">
    <h2>{{ t("settings.ocrSection") }}</h2>
    <p class="hint">{{ t("settings.ocrKeyringHint") }}</p>
    <label class="field">
      <span>{{ t("settings.ocrEngine") }}</span>
      <select v-model="activeEngine">
        <option v-for="eng in engines" :key="eng.id" :value="eng.id">
          {{ labelForEngine(eng.id) }}
        </option>
      </select>
    </label>

    <label v-for="field in activeFields" :key="field.settingKey" class="field">
      <span>
        {{ labelForField(field.name) }}
        <template v-if="!field.required">（{{ t("settings.ocrFieldOptional") }}）</template>
      </span>
      <AppInput
        v-model="values[field.settingKey]"
        :type="inputType(field.kind)"
        :placeholder="field.name === 'model' ? 'PP-OCRv5' : undefined"
      />
    </label>

    <label class="field">
      <span>{{ t("settings.ocrTimeout") }}</span>
      <AppInput v-model="timeoutMs" type="number" :min="1" :step="1000" />
    </label>
    <label class="field">
      <span>{{ t("settings.ocrInspectorPlacement") }}</span>
      <select v-model="inspectorPlacement">
        <option value="left">{{ t("settings.ocrInspectorPlacementLeft") }}</option>
        <option value="right">{{ t("settings.ocrInspectorPlacementRight") }}</option>
        <option value="bottom">{{ t("settings.ocrInspectorPlacementBottom") }}</option>
      </select>
    </label>
    <AppButton variant="primary" :disabled="busy" @click="onSave">
      {{ t("settings.ocrSave") }}
    </AppButton>
  </section>
</template>

<style scoped>
.section {
  margin-bottom: var(--space-5);
  padding-bottom: var(--space-4);
  border-bottom: 1px solid var(--border);
}

h2 {
  margin: 0 0 var(--space-2);
  font-size: var(--text-base);
  font-weight: var(--font-weight-title);
  color: var(--text);
}

.hint {
  margin: 0 0 var(--space-3);
  font-size: var(--text-sm);
  color: var(--text-muted);
  line-height: 1.4;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-bottom: var(--space-4);
}

select {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  max-width: 420px;
  font-size: var(--text-md);
}
</style>
