/** 脚本库领域 Monaco providers（从 ScriptEditor.vue 迁入，作为
 *  registerMonacoProviders 扩展示例）：params 字段补全 + 关键字/内建/片段 +
 *  black 格式化（后端 workspace .venv > 全局链）。 */

import { formatScriptCode } from "@/api/scripts";
import type { ScriptParamDef } from "@/api/types";
import type { MonacoModule } from "@/components/editor/monacoLoader";
import { registerMonacoProviders } from "@/components/editor/providers";
import type { ReportError } from "@/components/editor/providers";

/** 当前编辑器关联的参数 schema（params 补全数据源，由领域组件写入）。 */
let paramsSchemaRef: ScriptParamDef[] = [];

export function setParamsSchema(schema: ScriptParamDef[]) {
  paramsSchemaRef = schema;
}

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

/** 注册 python 领域 providers（幂等）。须在 loadMonaco 之后调用。 */
export function registerPythonProviders(m: MonacoModule, onError: ReportError) {
  registerMonacoProviders(
    "python-defaults",
    (monaco, reportError) => {
      // 让位判定：行尾形如 `params.xxx` 时关键字 provider 返回空
      const inParamsContext = (textUntil: string) => /(?:^|[^.\w])params\.\w*$/.test(textUntil);

      monaco.languages.registerCompletionItemProvider("python", {
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
            kind: monaco.languages.CompletionItemKind.Field,
            detail: `${def.type}${def.required ? " · required" : ""}`,
            documentation: `参数模块字段（onlytool_params.Params.${def.key}）`,
            insertText: def.key,
            range,
          }));
          return { suggestions };
        },
      });

      monaco.languages.registerCompletionItemProvider("python", {
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
              kind: monaco.languages.CompletionItemKind.Keyword,
              insertText: k,
              range,
            })),
            ...PY_BUILTINS.map((b) => ({
              label: b,
              kind: monaco.languages.CompletionItemKind.Function,
              insertText: b,
              range,
            })),
            ...PY_SNIPPETS.map(([label, detail, body]) => ({
              label,
              kind: monaco.languages.CompletionItemKind.Snippet,
              detail,
              insertText: body,
              insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
              range,
            })),
          ];
          return { suggestions };
        },
      });

      // 格式化：后端 black。失败（未装 black 等）经 reportError 弹出——
      // 静默 return null 会让用户以为格式化「无效」
      monaco.languages.registerDocumentFormattingEditProvider("python", {
        async provideDocumentFormattingEdits(model) {
          let formatted: string;
          try {
            formatted = await formatScriptCode(model.getValue());
          } catch (err) {
            reportError(err);
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
      console.info("[monaco] providers registered: params+keywords+format");
    },
    onError,
  );
  void m; // 单例模块经由 providers 框架获取，签名保留 m 便于领域扩展
}
