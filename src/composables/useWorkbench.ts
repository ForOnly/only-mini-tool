import { computed, ref } from "vue";

import { useToolSession } from "@/platform/toolSession";
import { tools } from "@/tools/registry";

export type MainView = "home" | "tool" | "settings" | "toolConfig";
export type NavFrom = "home" | "tool";

const mainView = ref<MainView>("home");
const settingsFrom = ref<NavFrom>("home");
const configToolId = ref<string | null>(null);

export function useWorkbench() {
  const session = useToolSession();

  const sortedTools = computed(() => [...tools].sort((a, b) => a.order - b.order));

  const configTool = computed(
    () => tools.find((t) => t.id === configToolId.value) ?? null,
  );

  function openHome() {
    mainView.value = "home";
  }

  async function openTool(id: string) {
    await session.open(id);
    mainView.value = "tool";
  }

  function openSettings(from: NavFrom = mainView.value === "tool" ? "tool" : "home") {
    settingsFrom.value = from;
    mainView.value = "settings";
  }

  function openToolConfig(id: string) {
    const tool = tools.find((t) => t.id === id);
    if (!tool?.settingsSection) {
      return;
    }
    configToolId.value = id;
    mainView.value = "toolConfig";
  }

  async function goBack() {
    if (mainView.value === "settings") {
      mainView.value =
        settingsFrom.value === "tool" && session.activeToolId.value ? "tool" : "home";
      return;
    }
    if (mainView.value === "toolConfig") {
      configToolId.value = null;
      openHome();
      return;
    }
    if (mainView.value === "tool") {
      await session.deactivate();
      openHome();
    }
  }

  async function closeTool(id: string) {
    await session.dispose(id);
    if (mainView.value === "tool" && !session.activeToolId.value) {
      openHome();
    }
  }

  return {
    mainView,
    activeToolId: session.activeToolId,
    activeTool: session.activeTool,
    configToolId,
    configTool,
    tools: sortedTools,
    settingsFrom,
    isToolRunning: session.isRunning,
    isToolDisposing: session.isDisposing,
    toolEpoch: session.epoch,
    openHome,
    openTool,
    openSettings,
    openToolConfig,
    goBack,
    closeTool,
  };
}
