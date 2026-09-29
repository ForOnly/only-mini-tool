<script setup lang="ts">
import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { onBeforeUnmount, onMounted, ref, watch } from "vue";

import { useAppearance } from "@/composables/useAppearance";

self.MonacoEnvironment = {
  getWorker() {
    return new editorWorker();
  },
};

const props = defineProps<{
  modelValue: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

const host = ref<HTMLDivElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
const { appearance } = useAppearance();

onMounted(() => {
  if (!host.value) return;
  const scheme = appearance.value?.resolved ?? "light";
  editor = monaco.editor.create(host.value, {
    value: props.modelValue,
    language: "python",
    automaticLayout: true,
    minimap: { enabled: false },
    fontSize: 13,
    theme: scheme === "dark" ? "vs-dark" : "vs",
    scrollBeyondLastLine: false,
  });
  editor.onDidChangeModelContent(() => {
    emit("update:modelValue", editor?.getValue() ?? "");
  });
});

watch(
  () => props.modelValue,
  (value) => {
    if (!editor) return;
    if (editor.getValue() !== value) {
      editor.setValue(value);
    }
  },
);

watch(
  () => appearance.value?.resolved,
  (scheme) => {
    monaco.editor.setTheme(scheme === "dark" ? "vs-dark" : "vs");
  },
);

onBeforeUnmount(() => {
  editor?.dispose();
  editor = null;
});
</script>

<template>
  <div ref="host" class="editor-host" />
</template>

<style scoped>
.editor-host {
  width: 100%;
  height: 100%;
  min-height: 0;
}
</style>
