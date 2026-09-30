<script setup lang="ts">
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { useAppearance } from "@/composables/useAppearance";
import { useMessage } from "@/composables/useMessage";
import type { ScriptParamDef } from "@/api/types";
import { formatScriptCode } from "@/api/scripts";
import { formatAppError } from "@/utils/error";

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

/** 首次动态加载后注册 providers（幂等）：关键字/片段 + params 字段 + black 格式化。
 * onError：setup 注入的错误回调（格式化失败可见化——模块级函数取不到 useMessage 上下文）。 */
function registerProviders(m: MonacoModule, onError: (err: unknown) => void) {
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

  // 格式化：后端 black（全局解释器子链）。失败（未装 black 等）经 onError 弹出——
  // 静默 return null 会让用户以为格式化「无效」
  m.languages.registerDocumentFormattingEditProvider("python", {
    async provideDocumentFormattingEdits(model) {
      let formatted: string;
      try {
        formatted = await formatScriptCode(model.getValue());
      } catch (err) {
        onError(err);
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
  console.debug("[monaco] providers registered: params+keywords+format");
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
const loadError = ref<string | null>(null);
let editor: import("monaco-editor/esm/vs/editor/editor.api").editor.IStandaloneCodeEditor | null =
  null;
const { appearance } = useAppearance();
const { error: errorMessage } = useMessage();
const { t } = useI18n();

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
  // 动态链任一环节失败：显式暴露（Vue 吞 async onMounted 错误——不 catch 即静默空白）
  try {
    const m = await loadMonaco();
    registerProviders(m, (err) => errorMessage(formatAppError(err, (k) => t(k))));
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
    // 补全显式触发（防默认漂移）：字母输入弹出 + `params.` 的 `.` 触发
    quickSuggestions: { other: true, comments: false, strings: false },
    suggestOnTriggerCharacters: true,
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
  } catch (err) {
    console.error("[monaco] load/register failed:", err);
    loadError.value = String(err);
  }
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
  <div v-if="loadError" class="editor-error">Monaco {{ t("scripts.editorLoadFailed") }}: {{ loadError }}</div>
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
