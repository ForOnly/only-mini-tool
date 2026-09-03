import { useOcr } from "@/tools/ocr/useOcr";

/** OCR 工具会话：关闭时清空业务单例与临时文件。 */
export const ocrSession = {
  async dispose(): Promise<void> {
    await useOcr().resetSession();
  },
};
