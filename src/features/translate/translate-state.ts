import type { AppErrorDto } from "../../lib/commands";

/** UTF-8 length, which is what the services actually limit (not JavaScript characters). */
export function utf8Bytes(text: string): number {
  return new TextEncoder().encode(text).length;
}

export function validateTranslation(text: string, source: string, target: string, providerId: string, limitBytes?: number): AppErrorDto | null {
  if (!text.trim()) return { code: "empty_text", message: "请输入要翻译的文本。", retryable: false };
  if (source === target) return { code: "identical_languages", message: "请选择不同的源语言和目标语言。", retryable: false };
  const limit = limitBytes ?? (providerId === "mymemory" ? 500 : undefined);
  const bytes = utf8Bytes(text);
  if (limit !== undefined && bytes > limit)
    return { code: "input_too_long", message: `该服务每次最多支持 ${limit} 字节（当前 ${bytes} 字节），请缩短原文或更换服务。`, retryable: false };
  if (!providerId) return { code: "configuration", message: "请先在设置中配置可用的翻译服务。", retryable: false };
  return null;
}

export function safeAppError(error: unknown): AppErrorDto {
  if (typeof error === "object" && error !== null && "code" in error && "message" in error && "retryable" in error
    && typeof error.code === "string" && typeof error.message === "string" && typeof error.retryable === "boolean") {
    const retryAfterSeconds = "retryAfterSeconds" in error && typeof error.retryAfterSeconds === "number" ? error.retryAfterSeconds : undefined;
    return { code: error.code, message: error.message, retryable: error.retryable, ...(retryAfterSeconds === undefined ? {} : { retryAfterSeconds }) };
  }
  return { code: "unavailable", message: "操作暂不可用，请重试。", retryable: true };
}
