import type { AppErrorDto } from "../../lib/commands";

export function validateTranslation(text: string, source: string, target: string, providerId: string): AppErrorDto | null {
  if (!text.trim()) return { code: "empty_text", message: "请输入要翻译的文本。", retryable: false };
  if (source === target) return { code: "identical_languages", message: "请选择不同的源语言和目标语言。", retryable: false };
  if (providerId === "mymemory" && new TextEncoder().encode(text).length > 500)
    return { code: "input_too_long", message: "MyMemory 每次最多支持 500 UTF-8 字节，请缩短原文或更换服务。", retryable: false };
  if (!providerId) return { code: "configuration", message: "请先在设置中配置可用的翻译服务。", retryable: false };
  return null;
}

export function safeAppError(error: unknown): AppErrorDto {
  if (typeof error === "object" && error !== null && "code" in error && "message" in error && "retryable" in error
    && typeof error.code === "string" && typeof error.message === "string" && typeof error.retryable === "boolean")
    return { code: error.code, message: error.message, retryable: error.retryable };
  return { code: "unavailable", message: "操作暂不可用，请重试。", retryable: true };
}
