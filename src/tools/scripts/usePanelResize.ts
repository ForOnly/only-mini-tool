import { computed, onBeforeUnmount, onMounted, ref, type Ref } from "vue";

export type PanelResizeAxis = "x" | "y";

export interface UsePanelResizeOptions {
  storageKey: string;
  axis: PanelResizeAxis;
  /** 每次 move/reclamp 调用，窄窗下可动态收缩 */
  getMin: () => number;
  getMax: () => number;
  /** 无存值时的初始值（工厂，便于读容器尺寸） */
  defaultValue: () => number;
}

function readStored(key: string): number | null {
  try {
    const raw = localStorage.getItem(key);
    if (raw == null || raw === "") return null;
    const n = Number(raw);
    return Number.isFinite(n) ? n : null;
  } catch {
    return null;
  }
}

function writeStored(key: string, px: number) {
  try {
    localStorage.setItem(key, String(Math.round(px)));
  } catch {
    /* ignore quota / private mode */
  }
}

function clamp(n: number, min: number, max: number): number {
  if (max < min) return min;
  return Math.min(max, Math.max(min, n));
}

/**
 * 编辑页 panel 尺寸拖拽（日志高度 / 左栏宽度共用）。
 * delta 模式 + pointercancel/lostpointercapture，避免绝对坐标与 flex 压缩脱钩。
 */
export function usePanelResize(options: UsePanelResizeOptions): {
  value: Ref<number>;
  dragging: Ref<boolean>;
  atMax: Ref<boolean>;
  onPointerDown: (event: PointerEvent) => void;
  onKeydown: (event: KeyboardEvent) => void;
  reclamp: () => void;
} {
  const { storageKey, axis, getMin, getMax, defaultValue } = options;

  function applyClamp(n: number): number {
    return clamp(Math.round(n), getMin(), getMax());
  }

  const stored = readStored(storageKey);
  const value = ref(applyClamp(stored ?? defaultValue()));
  const dragging = ref(false);
  /** 窗口缩放时 getMax 非响应式，用 epoch 触发 atMax 重算 */
  const boundsEpoch = ref(0);
  const atMax = computed(() => {
    void boundsEpoch.value;
    return value.value >= getMax();
  });

  let savedUserSelect: string | null = null;
  let activePointerId: number | null = null;
  let activeTarget: HTMLElement | null = null;

  function persist() {
    writeStored(storageKey, value.value);
  }

  function reclamp() {
    value.value = applyClamp(value.value);
    boundsEpoch.value += 1;
  }

  function endDrag(target: HTMLElement | null, pointerId: number | null) {
    if (!dragging.value) return;
    dragging.value = false;
    if (savedUserSelect != null) {
      document.body.style.userSelect = savedUserSelect;
      savedUserSelect = null;
    }
    if (target && pointerId != null) {
      try {
        target.releasePointerCapture(pointerId);
      } catch {
        /* already released */
      }
    }
    activePointerId = null;
    activeTarget = null;
    persist();
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();

    const target = event.currentTarget as HTMLElement;
    const startCoord = axis === "y" ? event.clientY : event.clientX;
    const startValue = value.value;

    dragging.value = true;
    savedUserSelect = document.body.style.userSelect;
    document.body.style.userSelect = "none";
    activePointerId = event.pointerId;
    activeTarget = target;
    target.setPointerCapture(event.pointerId);

    const onMove = (e: PointerEvent) => {
      if (!dragging.value) return;
      const delta =
        axis === "y" ? startCoord - e.clientY : e.clientX - startCoord;
      value.value = applyClamp(startValue + delta);
    };

    const onUp = (e: PointerEvent) => {
      target.removeEventListener("pointermove", onMove);
      target.removeEventListener("pointerup", onUp);
      target.removeEventListener("pointercancel", onUp);
      target.removeEventListener("lostpointercapture", onLost);
      endDrag(target, e.pointerId);
    };

    const onLost = () => {
      target.removeEventListener("pointermove", onMove);
      target.removeEventListener("pointerup", onUp);
      target.removeEventListener("pointercancel", onUp);
      target.removeEventListener("lostpointercapture", onLost);
      endDrag(null, null);
    };

    target.addEventListener("pointermove", onMove);
    target.addEventListener("pointerup", onUp);
    target.addEventListener("pointercancel", onUp);
    target.addEventListener("lostpointercapture", onLost);
  }

  function onKeydown(event: KeyboardEvent) {
    const step = event.shiftKey ? 32 : 8;
    let delta = 0;
    if (axis === "y") {
      if (event.key === "ArrowUp") delta = step;
      else if (event.key === "ArrowDown") delta = -step;
    } else {
      if (event.key === "ArrowRight") delta = step;
      else if (event.key === "ArrowLeft") delta = -step;
    }
    if (delta === 0) return;
    event.preventDefault();
    value.value = applyClamp(value.value + delta);
    persist();
  }

  function onWindowResize() {
    reclamp();
  }

  onMounted(() => {
    window.addEventListener("resize", onWindowResize);
    // 挂载后容器尺寸已就绪，再 clamp 一次（覆盖首屏前 getMax 偏小/偏大）
    reclamp();
  });

  onBeforeUnmount(() => {
    window.removeEventListener("resize", onWindowResize);
    if (dragging.value) {
      endDrag(activeTarget, activePointerId);
    }
  });

  return { value, dragging, atMax, onPointerDown, onKeydown, reclamp };
}
