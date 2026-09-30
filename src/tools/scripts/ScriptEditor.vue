<script setup lang="ts">
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { onBeforeUnmount, onMounted, ref, watch } from "vue";

import { useAppearance } from "@/composables/useAppearance";
import type { ScriptParamDef } from "@/api/types";
import { formatScriptCode } from "@/api/scripts";

// worker 与环境必须先于 monaco.create 就绪（静态赋值，不依赖 monaco 命名空间）
self.MonacoEnvironment = {
  getWorker() {
    return new editorWorker();
  },
};

type MonacoModule = typeof import("monaco-editor/esm/vs/editor/editor.api");

/** 动态加载的 monaco 模块（裁剪入口：仅 editor.api + python contribution，见 loadMonaco）。 */
let monaco: MonacoModule | null = null;
let providersRegistered = false;

/** 当前编辑器关联的参数 schema（params 补全数据源，由父组件写入）。 */
let paramsSchemaRef: ScriptParamDef[] = [];

const PY_KEYWORDS = [
  "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
  "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in",
  "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
  "with", "yield", "True", "False", "None",
];
const PY_BUILTINS = [
  "abs", "all", "any", "bool", "dict", "enumerate", "filter", "float", "format",
  "int", "isinstance", "len", "list", "map", "max", "min", "open", "print",
  "range", "repr", "reversed", "set", "sorted", "str", "sum", "tuple", "type", "zip",
];
const PY_SNIPPETS: Array<[string, string, string]> = [
  [
    "main",
    "if __name__ == \"__main__\":",
    'if __name__ == "__main__":\n    $0\n',
  ],
  ["def", "def name(...):", "def ${1:name}(${2:args}):\n    $0\n"],
  [
    "for",
    "for i in range(n):",
    "for ${1:i} in range(${2:n}):\n    $0\n",
  ],
  [
    "with",
    "with open(path) as f:",
    'with open(${1:path}, encoding="utf-8") as ${2:f}:\n    $0\n',
  ],
  [
    "try",
    "try/except:",
    "try:\n    $1\nexcept ${2:Exception} as ${3:e}:\n    $0\n",
  ],
];

/** 首次动态加载后注册 providers（幂等）：关键字/片段 + params 字段 + black 格式化。 */
function registerProviders(m: MonacoModule) {
  if (providersRegistered) return;
  providersRegistered = true;

  // 让位判定：行尾形如 `params.xxx` 时关键字 provider 返回空
  const inParamsContext = (textUntil: string) => /(?:^|[^.\w])params\.\w*$/.test(textUntil);

  m.languages.registerCompletionItemProvider("python", {
    triggerCharacters: ["."],
    provideCompletionItems(model, position) {
      if (!inParamsContext(model.getValueInRange({
        startLineNumber: position.lineNumber,
        startColumn: 1,
        endLineNumber: position.lineNumber,
        endColumn: position.column,
      }))) {
        return { suggestions: [] };
      }
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };
      const suggestions = paramsSchemaRef.map((def) => ({
        label: def.key,
        kind: m.languages.CompletionItemKind.Field,
        detail: `${def.type}${def.required ? " · required" : ""}`,
        documentation: `参数模块字段（onlytool_params.Params.${def.key}）`,
        insertText: def.key,
        range,
      }));
      return { suggestions };
    },
  });

  m.languages.registerCompletionItemProvider("python", {
    provideCompletionItems(model, position) {
      const lineText = model.getValueInRange({
        startLineNumber: position.lineNumber,
        startColumn: 1,
        endLineNumber: position.lineNumber,
        endColumn: position.column,
      });
      if (inParamsContext(lineText)) return { suggestions: [] };
      const word = model.getWordUntilPosition(position);
      if (!word.word) return { suggestions: [] };
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };
      const suggestions = [
        ...PY_KEYWORDS.map((k) => ({
          label: k,
          kind: m.languages.CompletionItemKind.Keyword,
          insertText: k,
          range,
        })),
        ...PY_BUILTINS.map((b) => ({
          label: b,
          kind: m.languages.CompletionItemKind.Function,
          insertText: b,
          range,
        })),
        ...PY_SNIPPETS.map(([label, detail, body]) => ({
          label,
          kind: m.languages.CompletionItemKind.Snippet,
          detail,
          insertText: body,
          insertTextRules: m.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          range,
        })),
      ];
      return { suggestions };
    },
  });

  // 格式化：后端 black（全局解释器子链），provider 收 model 即自包含
  m.languages.registerDocumentFormattingEditProvider("python", {
    async provideDocumentFormattingEdits(model) {
      let formatted: string;
      try {
        formatted = await formatScriptCode(model.getValue());
      } catch (err) {
        // 格式化失败（未装 black 等）不阻塞编辑——弹全局消息由调用侧? provider 内无 toast 通道，静默保持原文
        void err;
        return null;
      }
      return [
        {
          range: model.getFullModelRange(),
          text: formatted,
        },
      ];
    },
  });
}

/** 懒加载 monaco（裁剪入口：editor.api + 仅 python 语言贡献——css/html/json/ts 语言块不进产物）。 */
async function loadMonaco(): Promise<MonacoModule> {
  if (monaco) return monaco;
  const [m] = await Promise.all([
    import("monaco-editor/esm/vs/editor/editor.api"),
    import("monaco-editor/esm/vs/basic-languages/python/python.contribution"),
  ]);
  monaco = m as MonacoModule;
  return monaco;
}

const props = defineProps<{
  modelValue: string;
  paramsSchema?: ScriptParamDef[];
}>();

const emit = defineEmits<{
  "update:modelValue": [value: string];
  save: [];
}>();

const host = ref<HTMLDivElement | null>(null);
let editor: import("monaco-editor/esm/vs/editor/editor.api").editor.IStandaloneCodeEditor | null =
  null;
const { appearance } = useAppearance();

function defineThemes(m: MonacoModule) {
  m.editor.defineTheme("only-light", {
    base: "vs",
    inherit: true,
    rules: [],
    colors: { "editor.background": "#ffffff" },
  });
  m.editor.defineTheme("only-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: { "editor.background": "#1e293b" }, // 对齐 --surface
  });
}

onMounted(async () => {
  if (!host.value) return;
  const m = await loadMonaco();
  registerProviders(m);
  defineThemes(m);
  const scheme = appearance.value?.resolved ?? "light";
  editor = m.editor.create(host.value, {
    value: props.modelValue,
    language: "python",
    automaticLayout: true,
    minimap: { enabled: false },
    fontSize: 13,
    theme: scheme === "dark" ? "only-dark" : "only-light",
    scrollBeyondLastLine: false,
    // Monaco 自绘 DOM 滚动条不吃 CSS 伪元素，走 option 对齐全局滚动条
    scrollbar: {
      verticalScrollbarSize: 10, // 对齐 --scrollbar-size
      horizontalScrollbarSize: 10,
      useShadows: false,
    },
    // Python 编辑体验
    tabSize: 4,
    insertSpaces: true,
    bracketPairColorization: { enabled: true },
    guides: { bracketPairs: true, indentation: true },
    stickyScroll: { enabled: true },
    wordBasedSuggestions: "off",
    smoothScrolling: true,
    padding: { top: 8 },
    renderWhitespace: "selection",
  });
  editor.onDidChangeModelContent(() => {
    emit("update:modelValue", editor?.getValue() ?? "");
  });
  // Ctrl+S → 保存（与 RunBar 保存同函数）
  editor.addCommand(m.KeyMod.CtrlCmd | m.KeyCode.KeyS, () => emit("save"));
});

watch(
  () => props.modelValue,
  (value) => {
    if (!editor || !monaco) return;
    if (editor.getValue() !== value) {
      // pushEditOperations 保留 undo 栈（「丢弃」后 Ctrl+Z 可撤销对照）
      const model = editor.getModel();
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

watch(
  () => props.paramsSchema,
  (schema) => {
    paramsSchemaRef = schema ?? [];
  },
  { immediate: true, deep: true },
);

watch(
  () => appearance.value?.resolved,
  (scheme) => {
    monaco?.editor.setTheme(scheme === "dark" ? "only-dark" : "only-light");
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
