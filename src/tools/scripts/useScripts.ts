import { computed, ref } from "vue";

import type { ScriptDto, ScriptParamDef, ScriptSummary } from "@/api/types";
import {
  createScript,
  deleteScript,
  getScript,
  listScripts,
  renameScript,
  updateScript,
  type ScriptId,
} from "@/api/scripts";

export type ScriptsPage = "list" | "editor";

const PARAMS_STORAGE_PREFIX = "scripts.params.";

const pageRef = ref<ScriptsPage>("list");
const list = ref<ScriptSummary[]>([]);
const current = ref<ScriptDto | null>(null);
const draft = ref<ScriptDto | null>(null);
const paramValues = ref<Record<string, string>>({});
const busy = ref(false);
const listLoaded = ref(false);

function snapshotKey(dto: ScriptDto): string {
  return JSON.stringify({
    name: dto.name,
    description: dto.description,
    body: dto.body,
    workspacePath: dto.workspacePath ?? "",
    interpreterPath: dto.interpreterPath ?? "",
    env: dto.env,
    paramsSchema: dto.paramsSchema,
    argsTemplate: dto.argsTemplate,
  });
}

const dirty = computed(() => {
  if (!current.value || !draft.value) return false;
  return snapshotKey(current.value) !== snapshotKey(draft.value);
});

function scriptIdNum(id: ScriptDto["id"] | ScriptSummary["id"]): number {
  return typeof id === "bigint" ? Number(id) : Number(id);
}

function loadParamValues(id: number) {
  try {
    const raw = localStorage.getItem(PARAMS_STORAGE_PREFIX + id);
    if (!raw) {
      paramValues.value = {};
      return;
    }
    paramValues.value = JSON.parse(raw) as Record<string, string>;
  } catch {
    paramValues.value = {};
  }
}

function persistParamValues(id: number) {
  localStorage.setItem(PARAMS_STORAGE_PREFIX + id, JSON.stringify(paramValues.value));
}

function applyDefaultsFromSchema(schema: ScriptParamDef[]) {
  const next = { ...paramValues.value };
  for (const def of schema) {
    if (next[def.key] === undefined || next[def.key] === "") {
      if (def.default !== undefined && def.default !== null) {
        next[def.key] = String(def.default);
      } else if (def.type === "boolean") {
        next[def.key] = "false";
      } else {
        next[def.key] = next[def.key] ?? "";
      }
    }
  }
  paramValues.value = next;
}

function cloneDto(dto: ScriptDto): ScriptDto {
  return structuredClone(dto);
}

function normalizeEnv(
  env: ScriptDto["env"] | undefined,
): { [key in string]?: string } {
  const out: { [key in string]?: string } = {};
  for (const [k, v] of Object.entries(env ?? {})) {
    if (k) out[k] = v ?? "";
  }
  return out;
}

/** 脚本库业务单例。 */
export function useScripts() {
  async function refreshList() {
    list.value = await listScripts();
    listLoaded.value = true;
  }

  async function openList() {
    pageRef.value = "list";
    current.value = null;
    draft.value = null;
    await refreshList();
  }

  async function openEditor(id: ScriptId) {
    busy.value = true;
    try {
      const dto = await getScript(id);
      current.value = dto;
      draft.value = cloneDto(dto);
      loadParamValues(scriptIdNum(dto.id));
      applyDefaultsFromSchema(dto.paramsSchema);
      pageRef.value = "editor";
    } finally {
      busy.value = false;
    }
  }

  async function createNew(name: string) {
    const dto = await createScript({
      name,
      description: "",
      body: 'print("hello from only-mini-tool")\n',
    });
    await refreshList();
    await openEditor(scriptIdNum(dto.id));
  }

  async function rename(id: ScriptId, name: string) {
    await renameScript(id, name);
    await refreshList();
    if (draft.value && scriptIdNum(draft.value.id) === id) {
      draft.value = { ...draft.value, name };
      if (current.value) current.value = { ...current.value, name };
    }
  }

  async function remove(id: ScriptId) {
    await deleteScript(id);
    localStorage.removeItem(PARAMS_STORAGE_PREFIX + id);
    if (draft.value && scriptIdNum(draft.value.id) === id) {
      await openList();
    } else {
      await refreshList();
    }
  }

  async function save(): Promise<ScriptDto> {
    if (!draft.value) {
      throw new Error("no draft");
    }
    const d = draft.value;
    const updated = await updateScript(scriptIdNum(d.id), {
      name: d.name,
      description: d.description,
      body: d.body,
      workspacePath: d.workspacePath || undefined,
      interpreterPath: d.interpreterPath || undefined,
      env: normalizeEnv(d.env),
      paramsSchema: d.paramsSchema,
      argsTemplate: d.argsTemplate ?? { before: [], after: [] },
    });
    current.value = updated;
    draft.value = cloneDto(updated);
    persistParamValues(scriptIdNum(updated.id));
    await refreshList();
    return updated;
  }

  function discardDraft() {
    if (current.value) {
      draft.value = cloneDto(current.value);
    }
  }

  function setParamValue(key: string, value: string) {
    paramValues.value = { ...paramValues.value, [key]: value };
    if (draft.value) {
      persistParamValues(scriptIdNum(draft.value.id));
    }
  }

  async function resetSession() {
    pageRef.value = "list";
    list.value = [];
    current.value = null;
    draft.value = null;
    paramValues.value = {};
    listLoaded.value = false;
    busy.value = false;
  }

  return {
    page: pageRef,
    list,
    current,
    draft,
    paramValues,
    busy,
    listLoaded,
    dirty,
    scriptIdNum,
    refreshList,
    openList,
    openEditor,
    createNew,
    rename,
    remove,
    save,
    discardDraft,
    setParamValue,
    persistParamValues,
    resetSession,
  };
}
