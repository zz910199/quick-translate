import { invoke } from "@tauri-apps/api/core";

export interface TranslationResultDto { translatedText: string; providerId: string; elapsedMs: number; characterCount: number }
export interface LanguageOptionDto { code: string; label: string }
export interface ProviderOptionDto { id: string; displayName: string; isLocal: boolean; configured: boolean; isDefault: boolean; notice?: string }
export interface AppErrorDto { code: string; message: string; retryable: boolean }
export interface TranslationSelectionDto { source: string; target: string; providerId: string; privacyMode: "OnlinePreferred" | "LocalOnly" }
export interface ScreenshotRect { x: number; y: number; width: number; height: number }
export interface CapturedDisplayDto { id: string; originX: number; originY: number; scaleFactor: number; previewDataUrl: string; width: number; height: number }
export interface OcrBlockDto { text: string; confidence: number | null; bounds: ScreenshotRect }
export interface OcrResultDto { text: string; confidence: number | null; blocks: OcrBlockDto[] }

export const translateText = (text: string, source: string, target: string, providerId: string) =>
  invoke<TranslationResultDto>("translate_text", { text, source, target, providerId });
export const listLanguages = () => invoke<LanguageOptionDto[]>("list_languages");
export const listProviders = () => invoke<ProviderOptionDto[]>("list_providers");
export const writeClipboard = (text: string) => invoke<void>("write_clipboard", { text });
export const getTranslationSelection = () => invoke<TranslationSelectionDto>("get_translation_selection");
export const saveTranslationSelection = (source: string, target: string, providerId: string) =>
  invoke<void>("save_translation_selection", { source, target, providerId });
export const beginScreenshotTranslation = () => invoke<CapturedDisplayDto[]>("begin_screenshot_translation");
export const getScreenshotDisplay = (displayId: string) => invoke<CapturedDisplayDto>("get_screenshot_display", { displayId });
export const recognizeScreenshotRegion = (displayId: string, rect: ScreenshotRect) =>
  invoke<OcrResultDto>("recognize_screenshot_region", { displayId, rect });
export const cancelScreenshotTranslation = (displayId: string) => invoke<void>("cancel_screenshot_translation", { displayId });
