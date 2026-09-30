<script setup lang="ts">
/** 脚本编辑器：通用 CodeEditor + python 领域 providers（params 补全/关键字/black）。 */

import { onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";

import CodeEditor from "@/components/editor/CodeEditor.vue";
import { loadMonaco } from "@/components/editor/monacoLoader";
import type { ScriptParamDef } from "@/api/types";
import { useMessage } from "@/composables/useMessage";
import { formatAppError } from "@/utils/error";
import { registerPythonProviders, setParamsSchema } from "@/tools/scripts/monacoPython";

const props = defineProps<{
  modelValue: string;
  paramsSchema?: ScriptParamDef[];
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  save: [];
}>();

const { t } = useI18n();
const { error: errorMessage } = useMessage();

onMounted(async () => {
  try {
    const m = await loadMonaco();
    registerPythonProviders(m, (err) => errorMessage(formatAppError(err, (k) => t(k))));
  } catch (err) {
    // 加载失败由 CodeEditor 显式展示，此处仅记录
    console.error("[monaco] python providers init failed:", err);
  }
});

watch(
  () => props.paramsSchema,
  (schema) => {
    setParamsSchema(schema ?? []);
  },
  { immediate: true, deep: true },
);
</script>

<template>
  <CodeEditor
    :model-value="modelValue"
    language="python"
    @update:model-value="emit('update:modelValue', $event)"
    @save="emit('save')"
  />
</template>
