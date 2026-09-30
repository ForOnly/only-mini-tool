<script setup lang="ts">
/** 通用 Monaco 编辑器组件：懒加载单例 + 语言按需注册 + 主题跟随 + v-model 契约。
 *  options prop 浅合并覆盖默认值（嵌套对象整体替换）；领域 provider 由调用方
 *  经 registerMonacoProviders 注册（见 tools/scripts/monacoPython.ts 示例）。 */

import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { editor } from "monaco-editor/esm/vs/editor/editor.api";

import { useAppearance } from "@/composables/useAppearance";
import { ensureLanguage, loadMonaco } from "@/components/editor/monacoLoader";
import { defineOnlyThemes, themeNameFor, watchMonacoTheme } from "@/components/editor/monacoThemes";

const props = defineProps<{
  modelValue: string;
  language: string;
  options?: editor.IStandaloneEditorConstructionOptions;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  save: [];
}>();

const { t } = useI18n();
const { appearance } = useAppearance();

const host = ref<HTMLDivElement | null>(null);
const loadError = ref<string | null>(null);
let codeEditor: editor.IStandaloneCodeEditor | null = null;
let stopThemeWatch: (() => void) | null = null;

/** 基线选项（自原 ScriptEditor 迁入；options prop 浅合并覆盖） */
const BASE_OPTIONS: editor.IStandaloneEditorConstructionOptions = {
  automaticLayout: true,
  minimap: { enabled: false },
  fontSize: 13,
  scrollBeyondLastLine: false,
  // 补全显式触发（防默认漂移）：字母输入弹出 + 触发字符
  quickSuggestions: { other: true, comments: false, strings: false },
  suggestOnTriggerCharacters: true,
  // Monaco 自绘 DOM 滚动条不吃 CSS 伪元素，走 option 对齐全局滚动条
  scrollbar: {
    verticalScrollbarSize: 10, // 对齐 --scrollbar-size
    horizontalScrollbarSize: 10,
    useShadows: false,
  },
  tabSize: 4,
  insertSpaces: true,
  bracketPairColorization: { enabled: true },
  guides: { bracketPairs: true, indentation: true },
  stickyScroll: { enabled: true },
  wordBasedSuggestions: "off",
  smoothScrolling: true,
  padding: { top: 8 },
  renderWhitespace: "selection",
};

onMounted(async () => {
  if (!host.value) return;
  // 动态链任一环节失败：显式暴露（Vue 吞 async onMounted 错误——不 catch 即静默空白）
  try {
    const [m] = await Promise.all([loadMonaco(), ensureLanguage(props.language)]);
    defineOnlyThemes(m);
    const scheme = appearance.value?.resolved ?? "light";
    codeEditor = m.editor.create(host.value, {
      ...BASE_OPTIONS,
      ...props.options,
      value: props.modelValue,
      language: props.language,
      theme: themeNameFor(scheme),
    });
    codeEditor.onDidChangeModelContent(() => {
      emit("update:modelValue", codeEditor?.getValue() ?? "");
    });
    // Ctrl+S → 保存（与调用方保存动作同函数）
    codeEditor.addCommand(m.KeyMod.CtrlCmd | m.KeyCode.KeyS, () => emit("save"));
    stopThemeWatch = watchMonacoTheme();
  } catch (err) {
    console.error("[monaco] load failed:", err);
    loadError.value = String(err);
  }
});

watch(
  () => props.modelValue,
  (value) => {
    if (!codeEditor) return;
    if (codeEditor.getValue() !== value) {
      // pushEditOperations 保留 undo 栈（「丢弃」后 Ctrl+Z 可撤销对照）
      const model = codeEditor.getModel();
      if (!model) return;
      model.pushEditOperations(
        [],
        [
          {
            range: model.getFullModelRange(),
            text: value,
          },
        ],
        () => null,
      );
    }
  },
);

onBeforeUnmount(() => {
  stopThemeWatch?.();
  stopThemeWatch = null;
  codeEditor?.dispose();
  codeEditor = null;
});
</script>

<template>
  <div v-if="loadError" class="editor-error">
    Monaco {{ t("editor.loadFailed") }}: {{ loadError }}
  </div>
  <div v-show="!loadError" ref="host" class="editor-host" />
</template>

<style scoped>
.editor-host {
  width: 100%;
  height: 100%;
  min-height: 0;
}

.editor-error {
  padding: var(--space-4);
  font-size: var(--text-sm);
  color: var(--warning, #c47f17);
  word-break: break-all;
}
</style>
