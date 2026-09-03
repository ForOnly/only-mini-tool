import { ref } from "vue";

export type MessageType = "success" | "error" | "info" | "warning";

export interface MessageItem {
  id: number;
  type: MessageType;
  text: string;
  closable: boolean;
  /** duration===0 的常驻条，裁剪队列时优先保留 */
  sticky: boolean;
  key?: string;
}

export interface MessageShowOptions {
  type: MessageType;
  text: string;
  duration?: number;
  closable?: boolean;
  key?: string;
}

const messages = ref<MessageItem[]>([]);
const timers = new Map<number, number>();
let seq = 0;

const MAX_VISIBLE = 3;

const DEFAULT_DURATION: Record<MessageType, number> = {
  success: 2800,
  info: 2800,
  warning: 4000,
  error: 0,
};

function clearTimer(id: number) {
  const timer = timers.get(id);
  if (timer !== undefined) {
    clearTimeout(timer);
    timers.delete(id);
  }
}

function trimQueue() {
  while (messages.value.length > MAX_VISIBLE) {
    const drop =
      messages.value.find((m) => !m.sticky && !m.key) ??
      messages.value.find((m) => !m.sticky) ??
      messages.value[0];
    if (!drop) {
      break;
    }
    clearTimer(drop.id);
    messages.value = messages.value.filter((m) => m.id !== drop.id);
  }
}

export function useMessage() {
  function dismiss(id: number) {
    clearTimer(id);
    messages.value = messages.value.filter((m) => m.id !== id);
  }

  function dismissByKey(key: string) {
    const hit = messages.value.filter((m) => m.key === key);
    for (const item of hit) {
      dismiss(item.id);
    }
  }

  function show(options: MessageShowOptions) {
    const { type, text, key } = options;
    if (key) {
      dismissByKey(key);
    }

    const duration = options.duration ?? DEFAULT_DURATION[type];
    const closable = options.closable ?? (duration === 0 || type === "error" || type === "warning");
    const sticky = duration === 0;
    const id = ++seq;
    messages.value = [...messages.value, { id, type, text, closable, sticky, key }];
    trimQueue();

    if (duration > 0) {
      const timer = window.setTimeout(() => {
        dismiss(id);
      }, duration);
      timers.set(id, timer);
    }

    return id;
  }

  function success(text: string, durationMs?: number) {
    return show({ type: "success", text, duration: durationMs });
  }

  function error(text: string, options?: { duration?: number; key?: string }) {
    return show({
      type: "error",
      text,
      duration: options?.duration,
      key: options?.key,
      closable: true,
    });
  }

  function info(text: string, durationMs?: number) {
    return show({ type: "info", text, duration: durationMs });
  }

  function warning(text: string, durationMs?: number) {
    return show({ type: "warning", text, duration: durationMs, closable: true });
  }

  return {
    messages,
    show,
    success,
    error,
    info,
    warning,
    dismiss,
    dismissByKey,
  };
}
