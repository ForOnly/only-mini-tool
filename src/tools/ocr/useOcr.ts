import { convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { computed, ref } from "vue";

import {
  cancelRecognize,
  clearOcrTemp,
  recognizeImage,
  rotateImageOrientation,
  saveClipboardImage,
  stageImageBytes,
  stageImageFile,
} from "@/api/ocr";
import type { OcrResult } from "@/api/types";
import { useMessage } from "@/composables/useMessage";
import { i18n } from "@/i18n";
import { copyTextToClipboard, extFromImageBlob } from "@/tools/ocr/copyText";
import { formatAppError, getErrorCode } from "@/utils/error";

export type CopyLogEntry = {
  id: string;
  text: string;
  at: number;
};

const imagePath = ref<string | null>(null);
const imageUrl = ref<string | null>(null);
const result = ref<OcrResult | null>(null);
const busy = ref(false);
const boxesVisible = ref(true);
const highlightIndex = ref<number | null>(null);
const zoom = ref(1);
const panX = ref(0);
const panY = ref(0);
const lastError = ref<string | null>(null);
const imageNaturalWidth = ref(0);
const imageNaturalHeight = ref(0);
/** 画布双击等主动定位列表项（与 hover 高亮分离）。 */
const revealIndex = ref<number | null>(null);
const revealSeq = ref(0);
/** 本会话复制历史（换图 / 清除时清空）。 */
const copyLog = ref<CopyLogEntry[]>([]);

/** 关闭/清除/取消时递增，丢弃进行中的识别写回。 */
let sessionGen = 0;

const ZOOM_MIN = 0.1;
const ZOOM_MAX = 5;
/** 粘贴/字节入库上限（与后端 stage_image_bytes 一致）。 */
const MAX_STAGE_BYTES = 8 * 1024 * 1024;
const COPY_LOG_MAX = 50;

function tr(key: string): string {
  return String(i18n.global.t(key));
}

function errMsg(error: unknown): string {
  return formatAppError(error, tr);
}

function stale(gen: number): boolean {
  return gen !== sessionGen;
}

function warnBusyBlocked() {
  useMessage().warning(tr("ocr.busyBlocked"));
}

function revokeDisplayUrl(url: string | null) {
  if (url && url.startsWith("blob:")) {
    URL.revokeObjectURL(url);
  }
}

function setDisplayUrl(next: string) {
  revokeDisplayUrl(imageUrl.value);
  imageUrl.value = next;
}

function clearCopyLog() {
  copyLog.value = [];
}

function appendCopyLog(text: string) {
  const entry: CopyLogEntry = {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    text,
    at: Date.now(),
  };
  copyLog.value = [entry, ...copyLog.value].slice(0, COPY_LOG_MAX);
}

export function useOcr() {
  const hasImage = computed(() => !!imagePath.value);
  const words = computed(() => result.value?.words ?? []);
  const fullText = computed(() => result.value?.text ?? "");

  function setHighlight(index: number | null) {
    highlightIndex.value = index;
  }

  /** 高亮并请求结果列表滚动到该项。 */
  function revealWord(index: number) {
    highlightIndex.value = index;
    revealIndex.value = index;
    revealSeq.value += 1;
  }

  function setZoom(next: number) {
    zoom.value = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, next));
  }

  /** 以视口坐标为锚点缩放 */
  function zoomAt(nextZoom: number, anchorX: number, anchorY: number) {
    const clamped = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, nextZoom));
    const prev = zoom.value;
    if (prev === 0) {
      zoom.value = clamped;
      return;
    }
    const scale = clamped / prev;
    panX.value = anchorX - (anchorX - panX.value) * scale;
    panY.value = anchorY - (anchorY - panY.value) * scale;
    zoom.value = clamped;
  }

  function panBy(dx: number, dy: number) {
    panX.value += dx;
    panY.value += dy;
  }

  /** 按视口尺寸适配图片并居中；一律经 setZoom 钳制。 */
  function fitToView(
    viewportW: number,
    viewportH: number,
    naturalW = imageNaturalWidth.value,
    naturalH = imageNaturalHeight.value,
  ) {
    if (naturalW <= 0 || naturalH <= 0 || viewportW <= 0 || viewportH <= 0) {
      return;
    }
    const pad = 16;
    const vw = Math.max(1, viewportW - pad * 2);
    const vh = Math.max(1, viewportH - pad * 2);
    const nextZoom = Math.min(1, vw / naturalW, vh / naturalH);
    setZoom(nextZoom);
    panX.value = (viewportW - naturalW * zoom.value) / 2;
    panY.value = (viewportH - naturalH * zoom.value) / 2;
  }

  function zoomIn() {
    setZoom(zoom.value * 1.25);
  }

  function zoomOut() {
    setZoom(zoom.value / 1.25);
  }

  function resetView() {
    zoom.value = 1;
    panX.value = 0;
    panY.value = 0;
  }

  function setImageSize(width: number, height: number) {
    imageNaturalWidth.value = width;
    imageNaturalHeight.value = height;
  }

  async function loadPath(
    path: string,
    options: { autoRecognize?: boolean; displayUrl?: string } = {},
  ) {
    const { autoRecognize = true, displayUrl } = options;
    imagePath.value = path;
    setDisplayUrl(displayUrl ?? convertFileSrc(path));
    result.value = null;
    highlightIndex.value = null;
    lastError.value = null;
    clearCopyLog();
    resetView();
    if (autoRecognize) {
      await runRecognize();
    }
  }

  async function runRecognize() {
    if (!imagePath.value || busy.value) {
      return;
    }
    const gen = sessionGen;
    busy.value = true;
    lastError.value = null;
    try {
      const ocr = await recognizeImage(imagePath.value);
      if (stale(gen)) {
        return;
      }
      result.value = ocr;
    } catch (error) {
      if (stale(gen)) {
        return;
      }
      if (getErrorCode(error) === "ocr.cancelled") {
        return;
      }
      lastError.value = errMsg(error);
      result.value = null;
    } finally {
      if (!stale(gen)) {
        busy.value = false;
      }
    }
  }

  /** 硬取消后端识别并丢弃写回。 */
  async function cancelInFlight() {
    if (!busy.value) {
      return;
    }
    sessionGen += 1;
    busy.value = false;
    lastError.value = null;
    try {
      await cancelRecognize();
    } catch {
      /* ignore */
    }
  }

  async function openFile() {
    if (busy.value) {
      warnBusyBlocked();
      return;
    }
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "bmp", "gif", "webp"],
        },
      ],
    });
    if (!selected || Array.isArray(selected)) {
      return;
    }
    try {
      const staged = await stageImageFile(selected);
      await loadPath(staged);
    } catch (error) {
      lastError.value = errMsg(error);
    }
  }

  /** Web / Clipboard API：Blob 预览 + bytes 入库识别。 */
  async function pasteFromBlob(blob: Blob) {
    if (busy.value) {
      warnBusyBlocked();
      return;
    }
    try {
      if (blob.size > MAX_STAGE_BYTES) {
        lastError.value = tr("errors.ocr.image_too_large");
        return;
      }
      const buffer = await blob.arrayBuffer();
      if (buffer.byteLength > MAX_STAGE_BYTES) {
        lastError.value = tr("errors.ocr.image_too_large");
        return;
      }
      const bytes = Array.from(new Uint8Array(buffer));
      const staged = await stageImageBytes(bytes, extFromImageBlob(blob));
      const previewUrl = URL.createObjectURL(blob);
      await loadPath(staged, { displayUrl: previewUrl });
    } catch (error) {
      lastError.value = errMsg(error);
    }
  }

  /** 按钮/菜单：仅 Rust 读系统剪贴板（无 WebView 权限弹窗）。 */
  async function pasteImage() {
    if (busy.value) {
      warnBusyBlocked();
      return;
    }
    try {
      const staged = await saveClipboardImage();
      await loadPath(staged);
    } catch (error) {
      lastError.value = errMsg(error);
    }
  }

  async function openDroppedPaths(paths: string[]) {
    if (paths.length === 0) {
      return;
    }
    if (busy.value) {
      warnBusyBlocked();
      return;
    }
    const first = paths.find((p) => /\.(png|jpe?g|bmp|gif|webp)$/i.test(p));
    if (!first) {
      lastError.value = tr("errors.ocr.bad_image");
      return;
    }
    try {
      const staged = await stageImageFile(first);
      await loadPath(staged);
    } catch (error) {
      lastError.value = errMsg(error);
    }
  }

  async function rotateClockwise() {
    if (!imagePath.value || busy.value) {
      return;
    }
    const gen = sessionGen;
    busy.value = true;
    lastError.value = null;
    try {
      const next = await rotateImageOrientation(imagePath.value, 90);
      if (stale(gen)) {
        return;
      }
      imagePath.value = next;
      setDisplayUrl(convertFileSrc(next));
      result.value = null;
      highlightIndex.value = null;
      clearCopyLog();
      resetView();
      busy.value = false;
      await runRecognize();
    } catch (error) {
      if (stale(gen)) {
        return;
      }
      lastError.value = errMsg(error);
      busy.value = false;
    }
  }

  function toggleBoxes() {
    boxesVisible.value = !boxesVisible.value;
  }

  async function copyText(text: string): Promise<boolean> {
    if (!text) {
      return false;
    }
    await copyTextToClipboard(text);
    appendCopyLog(text);
    return true;
  }

  async function resetSession() {
    sessionGen += 1;
    busy.value = false;
    imagePath.value = null;
    revokeDisplayUrl(imageUrl.value);
    imageUrl.value = null;
    result.value = null;
    highlightIndex.value = null;
    lastError.value = null;
    clearCopyLog();
    resetView();
    imageNaturalWidth.value = 0;
    imageNaturalHeight.value = 0;
    useMessage().dismissByKey("ocr.lastError");
    try {
      await cancelRecognize();
    } catch {
      /* ignore */
    }
    try {
      await clearOcrTemp();
    } catch {
      /* ignore cleanup errors */
    }
  }

  async function clear() {
    await resetSession();
  }

  return {
    imagePath,
    imageUrl,
    result,
    busy,
    boxesVisible,
    highlightIndex,
    zoom,
    panX,
    panY,
    lastError,
    imageNaturalWidth,
    imageNaturalHeight,
    revealIndex,
    revealSeq,
    copyLog,
    hasImage,
    words,
    fullText,
    setHighlight,
    revealWord,
    setZoom,
    zoomAt,
    zoomIn,
    zoomOut,
    fitToView,
    panBy,
    setImageSize,
    resetView,
    openFile,
    pasteImage,
    pasteFromBlob,
    openDroppedPaths,
    rotateClockwise,
    toggleBoxes,
    clear,
    resetSession,
    runRecognize,
    cancelInFlight,
    copyText,
  };
}
