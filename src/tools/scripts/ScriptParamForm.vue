<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppInput from "@/components/common/AppInput.vue";
import type { ScriptParamDef } from "@/api/types";

defineProps<{
  schema: ScriptParamDef[];
  values: Record<string, string>;
}>();

const emit = defineEmits<{
  change: [key: string, value: string];
}>();

const { t } = useI18n();

async function pickPath(def: ScriptParamDef) {
  const selected = await open({
    directory: def.pathMode === "dir",
    multiple: false,
  });
  if (typeof selected === "string") {
    emit("change", def.key, selected);
  }
}
</script>

<template>
  <div class="params">
    <h3>{{ t("scripts.paramValues") }}</h3>
    <p v-if="!schema.length" class="hint">{{ t("scripts.noParams") }}</p>
    <label v-for="def in schema" :key="def.key" class="field">
      <span>
        {{ def.label || def.key }}
        <template v-if="def.required"> *</template>
      </span>
      <div v-if="def.type === 'boolean'" class="row">
        <input
          type="checkbox"
          :checked="values[def.key] === 'true' || values[def.key] === '1'"
          @change="
            emit(
              'change',
              def.key,
              ($event.target as HTMLInputElement).checked ? 'true' : 'false',
            )
          "
        />
      </div>
      <select
        v-else-if="def.type === 'select'"
        :value="values[def.key] ?? ''"
        @change="emit('change', def.key, ($event.target as HTMLSelectElement).value)"
      >
        <option value="">—</option>
        <option v-for="opt in def.options" :key="opt" :value="opt">{{ opt }}</option>
      </select>
      <div v-else-if="def.type === 'path'" class="row">
        <AppInput
          :model-value="values[def.key] ?? ''"
          @update:model-value="emit('change', def.key, $event)"
        />
        <AppButton variant="ghost" type="button" @click="pickPath(def)">
          {{ t("scripts.browse") }}
        </AppButton>
      </div>
      <AppInput
        v-else
        :type="def.type === 'number' ? 'number' : 'text'"
        :model-value="values[def.key] ?? ''"
        @update:model-value="emit('change', def.key, $event)"
      />
    </label>
  </div>
</template>

<style scoped>
.params {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-top: var(--space-2);
  border-top: 1px solid var(--border);
}

.params h3 {
  margin: 0;
  font-size: var(--text-md);
}

.hint {
  margin: 0;
  color: var(--text-muted);
  font-size: var(--text-sm);
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

.row > :deep(.app-input) {
  flex: 1;
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
