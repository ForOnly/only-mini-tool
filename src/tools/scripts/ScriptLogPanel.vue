<script setup lang="ts">
/** 运行日志纯内容组件：命令行回显 + 输出区。
 *  面板高度拖拽/标签头由 ScriptBottomPanel 承担（本组件不再自管布局外壳）。 */

import { computed } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import { useMessage } from "@/composables/useMessage";
import type { ScriptRunResult } from "@/api/types";

const props = defineProps<{
  result: ScriptRunResult | null;
  running: boolean;
}>();

const { t } = useI18n();
const { success } = useMessage();

const command = computed(() => props.result?.command ?? "");

const text = computed(() => {
  if (props.running) return t("scripts.runRunning");
  if (!props.result) return t("scripts.logEmpty");
  const parts = [
    props.result.cancelled
      ? t("scripts.runCancelled")
      : t("scripts.exitCode", { code: props.result.exitCode ?? "—" }),
    "",
    "—— stdout ——",
    props.result.stdout || "(empty)",
    "",
    "—— stderr ——",
    props.result.stderr || "(empty)",
  ];
  return parts.join("\n");
});

async function copyCommand() {
  if (!command.value) return;
  try {
    await navigator.clipboard.writeText(command.value);
    success(t("scripts.commandCopied"));
  } catch {
    /* 剪贴板不可用时静默 */
  }
}
</script>

<template>
  <div class="log-body">
    <div v-if="command" class="command-row">
      <span class="command-label">{{ t("scripts.commandLabel") }}</span>
      <code class="command-text">{{ command }}</code>
      <AppButton variant="ghost" type="button" @click="copyCommand">
        {{ t("scripts.copyCommand") }}
      </AppButton>
    </div>
    <pre class="body" data-scrollbar="thin">{{ text }}</pre>
  </div>
</template>

<style scoped>
.log-body {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
}

.command-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  border-bottom: 1px solid var(--border);
  min-width: 0;
}

.command-label {
  font-size: var(--text-xs, 12px);
  color: var(--text-muted);
  white-space: nowrap;
}

.command-text {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.body {
  margin: 0;
  padding: var(--space-3);
  flex: 1;
  min-height: 0;
  overflow: auto;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
