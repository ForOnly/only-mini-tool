<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppInput from "@/components/common/AppInput.vue";
import type { ScriptDto, ScriptParamDef, ScriptParamType } from "@/api/types";

const props = defineProps<{
  draft: ScriptDto;
}>();

const emit = defineEmits<{
  "update:draft": [value: ScriptDto];
}>();

const { t } = useI18n();

/** 本地行数组：允许空 key 草稿行；同步回 draft.env 时再过滤。 */
const envRows = ref<{ key: string; value: string }[]>([]);

watch(
  () => props.draft.env,
  (env) => {
    const next = Object.entries(env ?? {}).map(([key, value]) => ({
      key,
      value: value ?? "",
    }));
    // 保留正在编辑的空 key 行，避免外部 patch 冲掉 Add
    const blanks = envRows.value.filter((r) => !r.key.trim());
    envRows.value = [...next, ...blanks];
  },
  { immediate: true, deep: true },
);

function patch(partial: Partial<ScriptDto>) {
  emit("update:draft", { ...props.draft, ...partial });
}

function syncEnvToDraft() {
  const env: { [key in string]?: string } = {};
  for (const row of envRows.value) {
    const k = row.key.trim();
    if (!k) continue;
    env[k] = row.value;
  }
  patch({ env });
}

function updateEnvKey(index: number, key: string) {
  envRows.value = envRows.value.map((r, i) => (i === index ? { ...r, key } : r));
  syncEnvToDraft();
}

function updateEnvValue(index: number, value: string) {
  envRows.value = envRows.value.map((r, i) => (i === index ? { ...r, value } : r));
  syncEnvToDraft();
}

function addEnvRow() {
  envRows.value = [...envRows.value, { key: "", value: "" }];
}

function removeEnvRow(index: number) {
  envRows.value = envRows.value.filter((_, i) => i !== index);
  syncEnvToDraft();
}

async function pickWorkspace() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    patch({ workspacePath: selected });
  }
}

function addParam() {
  const schema = [...(props.draft.paramsSchema ?? [])];
  const key = `param${schema.length + 1}`;
  schema.push({
    key,
    label: key,
    type: "string",
    required: false,
    options: [],
    passAs: "env",
  });
  patch({ paramsSchema: schema });
}

function updateParam(index: number, partial: Partial<ScriptParamDef>) {
  const schema = props.draft.paramsSchema.map((p, i) =>
    i === index ? { ...p, ...partial } : p,
  );
  patch({ paramsSchema: schema });
}

function removeParam(index: number) {
  patch({
    paramsSchema: props.draft.paramsSchema.filter((_, i) => i !== index),
  });
}

function onTypeChange(index: number, type: ScriptParamType) {
  updateParam(index, {
    type,
    options: type === "select" ? props.draft.paramsSchema[index]?.options ?? [""] : [],
    pathMode: type === "path" ? "file" : undefined,
  });
}
</script>

<template>
  <div class="meta">
    <label class="field">
      <span>{{ t("scripts.name") }}</span>
      <AppInput
        :model-value="draft.name"
        @update:model-value="patch({ name: $event })"
      />
    </label>
    <label class="field">
      <span>{{ t("scripts.description") }}</span>
      <AppInput
        :model-value="draft.description"
        @update:model-value="patch({ description: $event })"
      />
    </label>
    <label class="field">
      <span>{{ t("scripts.workspace") }}</span>
      <div class="row">
        <AppInput
          :model-value="draft.workspacePath ?? ''"
          :placeholder="t('scripts.workspacePlaceholder')"
          @update:model-value="patch({ workspacePath: $event || undefined })"
        />
        <AppButton variant="ghost" type="button" @click="pickWorkspace">
          {{ t("scripts.browse") }}
        </AppButton>
      </div>
    </label>
    <label class="field">
      <span>{{ t("scripts.interpreter") }}</span>
      <AppInput
        :model-value="draft.interpreterPath ?? ''"
        :placeholder="t('scripts.interpreterPlaceholder')"
        @update:model-value="patch({ interpreterPath: $event || undefined })"
      />
    </label>

    <div class="block">
      <div class="block-head">
        <h3>{{ t("scripts.scriptEnv") }}</h3>
        <AppButton variant="ghost" type="button" @click="addEnvRow">
          {{ t("scripts.addEnv") }}
        </AppButton>
      </div>
      <div v-for="(row, index) in envRows" :key="index" class="row">
        <AppInput
          :model-value="row.key"
          :placeholder="t('scripts.envKey')"
          @update:model-value="updateEnvKey(index, $event)"
        />
        <AppInput
          :model-value="row.value"
          :placeholder="t('scripts.envValue')"
          @update:model-value="updateEnvValue(index, $event)"
        />
        <AppButton variant="ghost" type="button" @click="removeEnvRow(index)">
          {{ t("scripts.remove") }}
        </AppButton>
      </div>
    </div>

    <div class="block">
      <div class="block-head">
        <h3>{{ t("scripts.paramSchema") }}</h3>
        <AppButton variant="ghost" type="button" @click="addParam">
          {{ t("scripts.addParam") }}
        </AppButton>
      </div>
      <div
        v-for="(param, index) in draft.paramsSchema"
        :key="index"
        class="param-card"
      >
        <div class="row">
          <AppInput
            :model-value="param.key"
            :placeholder="t('scripts.paramKey')"
            @update:model-value="updateParam(index, { key: $event })"
          />
          <AppInput
            :model-value="param.label"
            :placeholder="t('scripts.paramLabel')"
            @update:model-value="updateParam(index, { label: $event })"
          />
        </div>
        <div class="row">
          <select
            :value="param.type"
            @change="onTypeChange(index, ($event.target as HTMLSelectElement).value as ScriptParamType)"
          >
            <option value="string">string</option>
            <option value="number">number</option>
            <option value="boolean">boolean</option>
            <option value="select">select</option>
            <option value="path">path</option>
          </select>
          <select
            :value="param.passAs"
            @change="
              updateParam(index, {
                passAs: ($event.target as HTMLSelectElement).value as 'env' | 'arg' | 'stdin',
              })
            "
          >
            <option value="env">env</option>
            <option value="arg">arg</option>
            <option value="stdin">stdin</option>
          </select>
          <label class="check">
            <input
              type="checkbox"
              :checked="param.required"
              @change="
                updateParam(index, {
                  required: ($event.target as HTMLInputElement).checked,
                })
              "
            />
            {{ t("scripts.required") }}
          </label>
          <AppButton variant="ghost" type="button" @click="removeParam(index)">
            {{ t("scripts.remove") }}
          </AppButton>
        </div>
        <!-- default：boolean 勾选、select 下拉（有选项时）、其余文本 -->
        <label v-if="param.type === 'boolean'" class="check">
          <input
            type="checkbox"
            :checked="param.default === 'true'"
            @change="
              updateParam(index, {
                default: ($event.target as HTMLInputElement).checked ? 'true' : undefined,
              })
            "
          />
          {{ t("scripts.paramDefault") }}
        </label>
        <select
          v-else-if="param.type === 'select' && (param.options ?? []).length"
          :value="param.default ?? ''"
          @change="
            updateParam(index, {
              default: ($event.target as HTMLSelectElement).value || undefined,
            })
          "
        >
          <option value="">—</option>
          <option v-for="opt in param.options" :key="opt" :value="opt">{{ opt }}</option>
        </select>
        <AppInput
          v-else
          :model-value="param.default ?? ''"
          :placeholder="t('scripts.paramDefault')"
          @update:model-value="updateParam(index, { default: $event || undefined })"
        />
        <AppInput
          v-if="param.type === 'select'"
          :model-value="(param.options ?? []).join(',')"
          :placeholder="t('scripts.selectOptions')"
          @update:model-value="
            updateParam(index, {
              options: $event
                .split(',')
                .map((s) => s.trim())
                .filter(Boolean),
            })
          "
        />
        <select
          v-if="param.type === 'path'"
          :value="param.pathMode ?? 'file'"
          @change="
            updateParam(index, {
              pathMode: ($event.target as HTMLSelectElement).value as 'file' | 'dir',
            })
          "
        >
          <option value="file">{{ t("scripts.pathFile") }}</option>
          <option value="dir">{{ t("scripts.pathDir") }}</option>
        </select>
      </div>
    </div>
  </div>
</template>

<style scoped>
.meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
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

.row > :deep(.app-input),
.row > select {
  flex: 1;
  min-width: 0;
}

.block {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-top: var(--space-2);
  border-top: 1px solid var(--border);
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

.param-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.check {
  display: flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
  color: var(--text);
  font-size: var(--text-sm);
}

select {
  min-height: 36px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface);
  color: var(--text);
  padding: 0 var(--space-2);
}
</style>
