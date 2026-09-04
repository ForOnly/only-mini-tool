import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** 无边框窗口：最小化 / 最大化 / 关闭与最大化态。 */
export function useWindowControls() {
  const isMaximized = ref(false);
  let unlisten: (() => void) | null = null;

  async function refreshMaximized() {
    try {
      isMaximized.value = await getCurrentWindow().isMaximized();
    } catch {
      /* web / 非 Tauri */
    }
  }

  async function minimize() {
    try {
      await getCurrentWindow().minimize();
    } catch {
      /* web / 非 Tauri / 权限不足 */
    }
  }

  async function toggleMaximize() {
    try {
      await getCurrentWindow().toggleMaximize();
      await refreshMaximized();
    } catch {
      /* web / 非 Tauri / 权限不足 */
    }
  }

  async function close() {
    try {
      await getCurrentWindow().close();
    } catch {
      /* web / 非 Tauri / 权限不足 */
    }
  }

  onMounted(async () => {
    await refreshMaximized();
    try {
      unlisten = await getCurrentWindow().onResized(() => {
        void refreshMaximized();
      });
    } catch {
      /* ignore */
    }
  });

  onUnmounted(() => {
    unlisten?.();
  });

  return {
    isMaximized,
    minimize,
    toggleMaximize,
    close,
  };
}
