import { computed, ref } from "vue";

import { tools } from "@/tools/registry";

const running = ref<Record<string, true>>({});
const disposing = ref<Record<string, true>>({});
const epochs = ref<Record<string, number>>({});
const activeToolId = ref<string | null>(null);

const inflight = new Map<string, Promise<void>>();

function findTool(id: string) {
  return tools.find((t) => t.id === id) ?? null;
}

function persistOnDeactivate(id: string): boolean {
  return findTool(id)?.persistOnDeactivate !== false;
}

export function useToolSession() {
  const activeTool = computed(() => {
    const id = activeToolId.value;
    return id ? findTool(id) : null;
  });

  function isRunning(id: string): boolean {
    return !!running.value[id];
  }

  function isDisposing(id: string): boolean {
    return !!disposing.value[id];
  }

  function epoch(id: string): number {
    return epochs.value[id] ?? 0;
  }

  async function waitDispose(id: string): Promise<void> {
    const pending = inflight.get(id);
    if (pending) {
      await pending;
    }
  }

  async function open(id: string): Promise<void> {
    const tool = findTool(id);
    if (!tool) {
      return;
    }
    await waitDispose(id);
    if (!running.value[id]) {
      running.value = { ...running.value, [id]: true };
    }
    activeToolId.value = id;
  }

  /** 回桌面：默认保活；persistOnDeactivate=false 时销毁。 */
  async function deactivate(): Promise<void> {
    const id = activeToolId.value;
    if (id && !persistOnDeactivate(id)) {
      await dispose(id);
    }
  }

  async function dispose(id: string): Promise<void> {
    await waitDispose(id);
    if (!running.value[id]) {
      return;
    }

    const run = (async () => {
      disposing.value = { ...disposing.value, [id]: true };
      try {
        await findTool(id)?.session?.dispose();
      } finally {
        const nextRunning = { ...running.value };
        delete nextRunning[id];
        running.value = nextRunning;
        epochs.value = { ...epochs.value, [id]: (epochs.value[id] ?? 0) + 1 };
        const nextDisposing = { ...disposing.value };
        delete nextDisposing[id];
        disposing.value = nextDisposing;
        if (activeToolId.value === id) {
          activeToolId.value = null;
        }
      }
    })();

    inflight.set(id, run);
    try {
      await run;
    } finally {
      inflight.delete(id);
    }
  }

  return {
    running,
    activeToolId,
    activeTool,
    isRunning,
    isDisposing,
    epoch,
    open,
    deactivate,
    dispose,
  };
}
