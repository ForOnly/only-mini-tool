export interface AppErrorPayload {
  code?: string;
  message?: string;
}

export function getErrorCode(error: unknown): string | null {
  if (error && typeof error === "object" && "code" in error) {
    const code = (error as AppErrorPayload).code;
    return typeof code === "string" ? code : null;
  }
  return null;
}

/** IPC code `ocr.not_configured` → i18n key `errors.ocr.not_configured` */
export function errorI18nKey(code: string): string {
  return `errors.${code}`;
}

export function formatAppError(
  error: unknown,
  translate?: (key: string) => string,
): string {
  const code = getErrorCode(error);
  if (code && translate) {
    const key = errorI18nKey(code);
    const localized = translate(key);
    if (localized && localized !== key) {
      return localized;
    }
  }

  if (error && typeof error === "object" && "message" in error) {
    const message = (error as AppErrorPayload).message;
    if (typeof message === "string" && message.length > 0) {
      return message;
    }
  }
  if (typeof error === "string") {
    return error;
  }
  return translate?.("errors.unknown") ?? "Unknown error";
}
