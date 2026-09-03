<script setup lang="ts">
defineProps<{
  left: number;
  top: number;
  width: number;
  height: number;
  active?: boolean;
}>();

defineEmits<{
  enter: [];
  leave: [];
  dblclick: [];
}>();
</script>

<template>
  <button
    type="button"
    class="box"
    :class="{ active }"
    :style="{
      left: `${left}px`,
      top: `${top}px`,
      width: `${width}px`,
      height: `${height}px`,
    }"
    @mouseenter="$emit('enter')"
    @mouseleave="$emit('leave')"
    @dblclick="$emit('dblclick')"
  />
</template>

<style scoped>
.box {
  position: absolute;
  box-sizing: border-box;
  margin: 0;
  padding: 0;
  border: 2px solid var(--ocr-box);
  background: transparent;
  cursor: pointer;
  pointer-events: auto;
}

.box:hover,
.box.active {
  border-color: var(--ocr-box-hover);
  background: color-mix(in srgb, var(--ocr-box-hover) 18%, transparent);
}
</style>
