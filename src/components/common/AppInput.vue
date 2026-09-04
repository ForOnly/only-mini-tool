<script setup lang="ts">
withDefaults(
  defineProps<{
    modelValue?: string;
    type?: "text" | "password" | "number" | "url";
    placeholder?: string;
    disabled?: boolean;
    autocomplete?: string;
    min?: number | string;
    step?: number | string;
  }>(),
  {
    modelValue: "",
    type: "text",
    disabled: false,
    autocomplete: "off",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
}>();

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}
</script>

<template>
  <input
    class="app-input"
    :type="type"
    :value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    :autocomplete="autocomplete"
    :min="min"
    :step="step"
    @input="onInput"
  />
</template>

<style scoped>
.app-input {
  box-sizing: border-box;
  width: 100%;
  max-width: 420px;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font-size: var(--text-md);
  line-height: 1.25;
}

.app-input:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
}

.app-input:focus-visible {
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
}

.app-input:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.app-input::placeholder {
  color: var(--text-muted);
}
</style>
