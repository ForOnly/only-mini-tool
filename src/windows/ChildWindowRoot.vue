<script setup lang="ts">
/** 子窗口裸根：挂对应窗口 App + 全局 toast 宿主（子窗内错误可见）。 */

import AppMessageHost from "@/components/common/AppMessageHost.vue";
import type { ChildWindowSpec } from "@/windows";

defineProps<{
  spec: ChildWindowSpec;
}>();
</script>

<template>
  <div class="child-window-root">
    <component :is="spec.component" v-bind="spec.props" />
    <AppMessageHost />
  </div>
</template>

<style scoped>
.child-window-root {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}
</style>
