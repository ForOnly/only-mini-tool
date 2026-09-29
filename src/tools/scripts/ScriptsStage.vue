<script setup lang="ts">
import { onMounted } from "vue";

import ScriptEditorPage from "@/tools/scripts/ScriptEditorPage.vue";
import ScriptListPage from "@/tools/scripts/ScriptListPage.vue";
import { useScripts } from "@/tools/scripts/useScripts";

defineOptions({ name: "ScriptsStage" });

const { page, listLoaded, refreshList } = useScripts();

onMounted(() => {
  if (!listLoaded.value) {
    void refreshList().catch(() => {
      /* list page also loads */
    });
  }
});
</script>

<template>
  <div class="scripts-stage">
    <ScriptListPage v-if="page === 'list'" />
    <ScriptEditorPage v-else />
  </div>
</template>

<style scoped>
.scripts-stage {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
</style>
