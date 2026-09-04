import { invoke } from "@tauri-apps/api/core";

import type { OcrResult, OcrSettingsBundle, OcrSettingsSave } from "./types";

export function recognizeImage(path: string): Promise<OcrResult> {
  return invoke<OcrResult>("recognize_image", { path });
}

export function cancelRecognize(): Promise<void> {
  return invoke("cancel_recognize");
}

export function stageImageFile(path: string): Promise<string> {
  return invoke<string>("stage_image_file", { path });
}

export function stageImageBytes(bytes: number[] | Uint8Array, ext: string): Promise<string> {
  const payload = bytes instanceof Uint8Array ? Array.from(bytes) : bytes;
  return invoke<string>("stage_image_bytes", { bytes: payload, ext });
}

export function saveClipboardImage(): Promise<string> {
  return invoke<string>("save_clipboard_image");
}

export function rotateImageOrientation(path: string, degrees: number): Promise<string> {
  return invoke<string>("rotate_image_orientation", { path, degrees });
}

export function clearOcrTemp(): Promise<void> {
  return invoke("clear_ocr_temp");
}

export function getOcrSettings(): Promise<OcrSettingsBundle> {
  return invoke<OcrSettingsBundle>("get_ocr_settings");
}

export function saveOcrSettings(payload: OcrSettingsSave): Promise<void> {
  return invoke("save_ocr_settings", { payload });
}
