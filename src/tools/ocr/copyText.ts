/** 将文本写入系统剪贴板。 */
export async function copyTextToClipboard(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

/** 从 PasteEvent 的 clipboardData 提取第一张图片。 */
export function extractClipboardImageBlob(data: DataTransfer | null): Blob | null {
  if (!data) {
    return null;
  }
  for (const item of Array.from(data.items)) {
    if (item.kind === "file" && item.type.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) {
        return file;
      }
    }
  }
  for (const file of Array.from(data.files)) {
    if (file.type.startsWith("image/")) {
      return file;
    }
  }
  return null;
}

export function extFromImageBlob(blob: Blob): string {
  const mime = blob.type.toLowerCase();
  if (mime.includes("jpeg") || mime.includes("jpg")) {
    return "jpg";
  }
  if (mime.includes("webp")) {
    return "webp";
  }
  if (mime.includes("gif")) {
    return "gif";
  }
  if (mime.includes("bmp")) {
    return "bmp";
  }
  if (blob instanceof File) {
    const match = /\.([a-z0-9]+)$/i.exec(blob.name);
    if (match) {
      return match[1].toLowerCase();
    }
  }
  return "png";
}
