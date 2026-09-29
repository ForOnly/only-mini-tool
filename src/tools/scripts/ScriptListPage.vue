<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";
import AppConfirm from "@/components/common/AppConfirm.vue";
import AppInput from "@/components/common/AppInput.vue";
import { useMessage } from "@/composables/useMessage";
import { useWorkbench } from "@/composables/useWorkbench";
import { useScripts, type ScriptTemplate } from "@/tools/scripts/useScripts";
import { formatAppError } from "@/utils/error";

const { t } = useI18n();
const { success, error } = useMessage();
const { mainView } = useWorkbench();
const { list, listLoaded, scriptIdNum, refreshList, createNew, rename, remove, openEditor } =
  useScripts();

const createOpen = ref(false);
const createName = ref("");
const createTemplate = ref<ScriptTemplate>("argparse");
const renameId = ref<number | null>(null);
const renameName = ref("");
const deleteId = ref<number | null>(null);
const deleteName = ref("");

const TEMPLATE_OPTIONS: { value: ScriptTemplate; labelKey: string }[] = [
  { value: "argparse", labelKey: "scripts.templateArgparse" },
  { value: "env", labelKey: "scripts.templateEnv" },
  { value: "stdin", labelKey: "scripts.templateStdin" },
  { value: "blank", labelKey: "scripts.templateBlank" },
];

function report(err: unknown) {
  error(formatAppError(err, (key) => t(key)));
}

// 弹窗 Teleport 到 body：离开工具视图时关闭，避免残留在桌面页上方
watch(mainView, (view) => {
  if (view !== "tool") {
    createOpen.value = false;
    renameId.value = null;
    deleteId.value = null;
  }
});

onMounted(() => {
  void refreshList().catch(report);
});

function startCreate() {
  createName.value = "";
  createOpen.value = true;
}

async function confirmCreate() {
  const name = createName.value.trim();
  createOpen.value = false;
  if (!name) return;
  try {
    await createNew(name, createTemplate.value);
    success(t("scripts.created"));
  } catch (err) {
    report(err);
  }
}

function startRename(id: number, name: string) {
  renameId.value = id;
  renameName.value = name;
}

async function confirmRename() {
  const id = renameId.value;
  const name = renameName.value.trim();
  renameId.value = null;
  if (id == null || !name) return;
  try {
    await rename(id, name);
    success(t("scripts.renamed"));
  } catch (err) {
    report(err);
  }
}

function startDelete(id: number, name: string) {
  deleteId.value = id;
  deleteName.value = name;
}

async function confirmDelete() {
  const id = deleteId.value;
  deleteId.value = null;
  if (id == null) return;
  try {
    await remove(id);
    success(t("scripts.deleted"));
  } catch (err) {
    report(err);
  }
}

async function openItem(id: number) {
  try {
    await openEditor(id);
  } catch (err) {
    report(err);
  }
}
</script>

<template>
  <div class="list-page">
    <header class="head">
      <h1>{{ t("scripts.listTitle") }}</h1>
      <AppButton variant="primary" @click="startCreate">{{ t("scripts.new") }}</AppButton>
    </header>

    <p v-if="!list.length && listLoaded" class="empty">
      {{ t("scripts.empty") }}
    </p>

    <ul v-else class="items">
      <li v-for="item in list" :key="String(item.id)" class="item">
        <button type="button" class="main" @click="openItem(scriptIdNum(item.id))">
          <span class="name">{{ item.name }}</span>
          <span v-if="item.description" class="desc">{{ item.description }}</span>
          <span class="meta">{{ item.updatedAt }}</span>
        </button>
        <div class="actions">
          <AppButton @click="startRename(scriptIdNum(item.id), item.name)">
            {{ t("scripts.rename") }}
          </AppButton>
          <AppButton
            variant="ghost"
            @click="startDelete(scriptIdNum(item.id), item.name)"
          >
            {{ t("scripts.delete") }}
          </AppButton>
        </div>
      </li>
    </ul>

    <AppConfirm
      :open="createOpen"
      :title="t('scripts.new')"
      :confirm-label="t('common.confirm')"
      :cancel-label="t('common.cancel')"
      @confirm="confirmCreate"
      @cancel="createOpen = false"
    >
      <AppInput v-model="createName" :placeholder="t('scripts.namePlaceholder')" />
      <div class="template-row">
        <span class="template-label">{{ t("scripts.template") }}</span>
        <select v-model="createTemplate">
          <option v-for="opt in TEMPLATE_OPTIONS" :key="opt.value" :value="opt.value">
            {{ t(opt.labelKey) }}
          </option>
        </select>
      </div>
    </AppConfirm>

    <AppConfirm
      :open="renameId != null"
      :title="t('scripts.rename')"
      :confirm-label="t('common.confirm')"
      :cancel-label="t('common.cancel')"
      @confirm="confirmRename"
      @cancel="renameId = null"
    >
      <AppInput v-model="renameName" :placeholder="t('scripts.namePlaceholder')" />
    </AppConfirm>

    <AppConfirm
      :open="deleteId != null"
      :title="t('scripts.deleteConfirmTitle')"
      :message="t('scripts.deleteConfirmMessage', { name: deleteName })"
      :confirm-label="t('scripts.delete')"
      :cancel-label="t('common.cancel')"
      danger
      @confirm="confirmDelete"
      @cancel="deleteId = null"
    />
  </div>
</template>

<style scoped>
.list-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: var(--space-4);
  gap: var(--space-3);
  overflow: auto;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.head h1 {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: var(--font-weight-title);
}

.empty {
  margin: var(--space-6) 0;
  color: var(--text-muted);
  text-align: center;
}

.items {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.item {
  display: flex;
  align-items: stretch;
  gap: var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface);
  overflow: hidden;
}

.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: var(--space-3);
  border: 0;
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.main:hover {
  background: color-mix(in srgb, var(--surface) 88%, var(--accent));
}

.name {
  font-weight: 600;
}

.desc {
  font-size: var(--text-sm);
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}

.meta {
  font-size: var(--text-xs, 12px);
  color: var(--text-muted);
}

.actions {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-2);
}

.template-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.template-label {
  font-size: var(--text-sm);
  color: var(--text-muted);
  white-space: nowrap;
}

.template-row select {
  flex: 1;
  min-width: 0;
  min-height: 36px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--surface);
  color: var(--text);
  padding: 0 var(--space-2);
}
</style>
