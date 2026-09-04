<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from "vue";
import { useI18n } from "vue-i18n";

import AppButton from "@/components/common/AppButton.vue";

const props = withDefaults(
  defineProps<{
    open: boolean;
    title: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    danger?: boolean;
  }>(),
  {
    message: "",
    danger: false,
  },
);

const emit = defineEmits<{
  confirm: [];
  cancel: [];
}>();

const { t } = useI18n();

const resolvedConfirmLabel = computed(
  () => props.confirmLabel ?? t("common.confirm"),
);
const resolvedCancelLabel = computed(
  () => props.cancelLabel ?? t("common.cancel"),
);

function onKeydown(event: KeyboardEvent) {
  if (!props.open) {
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    emit("cancel");
  }
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      document.addEventListener("keydown", onKeydown);
    } else {
      document.removeEventListener("keydown", onKeydown);
    }
  },
);

onMounted(() => {
  if (props.open) {
    document.addEventListener("keydown", onKeydown);
  }
});

onUnmounted(() => {
  document.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="backdrop"
      role="presentation"
      @click.self="emit('cancel')"
    >
      <div
        class="dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        :aria-describedby="message ? 'confirm-desc' : undefined"
      >
        <h2 id="confirm-title" class="title">{{ title }}</h2>
        <p v-if="message" id="confirm-desc" class="message">{{ message }}</p>
        <div class="actions">
          <AppButton variant="ghost" @click="emit('cancel')">
            {{ resolvedCancelLabel }}
          </AppButton>
          <AppButton
            variant="primary"
            :class="{ danger }"
            @click="emit('confirm')"
          >
            {{ resolvedConfirmLabel }}
          </AppButton>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 9500;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-5);
  background: color-mix(in srgb, #000 40%, transparent);
}

.dialog {
  width: min(420px, 100%);
  padding: var(--space-5);
  border-radius: calc(var(--radius) + 2px);
  border: 1px solid var(--border);
  background: var(--surface);
  box-shadow: var(--shadow-panel);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.title {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: var(--font-weight-title);
  color: var(--text);
}

.message {
  margin: 0;
  font-size: var(--text-md);
  line-height: 1.45;
  color: var(--text-muted);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.actions :deep(.btn.danger) {
  background: var(--danger);
  border-color: var(--danger);
  color: #fff;
}

.actions :deep(.btn.danger:hover) {
  background: color-mix(in srgb, var(--danger) 88%, #000);
  border-color: color-mix(in srgb, var(--danger) 88%, #000);
}
</style>
