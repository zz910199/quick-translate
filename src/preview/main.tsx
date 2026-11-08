/**
 * Browser preview harness. Serves the real React tree with stubbed Tauri IPC so the
 * layout can be inspected without a desktop shell: `npm run dev` then /preview.html.
 * Add `?view=history|settings` and `?demo=loading|error` to reach other states.
 */
import { createRoot } from "react-dom/client";
import { App } from "../App";
import "../styles.css";

const params = new URLSearchParams(window.location.search);
const demo = params.get("demo");

const languages = [
  { code: "en", label: "英语" },
  { code: "zh-CN", label: "中文（简体）" },
  { code: "ja", label: "日语" },
  { code: "de", label: "德语" },
];

const providers = [
  {
    id: "mymemory",
    displayName: "MyMemory",
    isLocal: false,
    configured: true,
    isDefault: true,
    limitBytes: 500,
    notice:
      "在线服务：翻译时原文将发送至所选服务提供方。 Translation provided by MyMemory (Translated). 每次最多 500 UTF-8 字节，匿名额度每天 5,000 字符。",
  },
  { id: "deepl", displayName: "DeepL", isLocal: false, configured: false, isDefault: false },
  { id: "local", displayName: "本地模型", isLocal: true, configured: true, isDefault: false },
];

const history = [
  { id: 1, sourceText: "The quick brown fox jumps over the lazy dog.", translatedText: "敏捷的棕色狐狸跳过了懒狗。", source: "en", target: "zh-CN", providerId: "MyMemory", timestamp: "2026-11-08T14:22:00Z" },
  { id: 2, sourceText: "Ship it on Friday, nobody will notice.", translatedText: "周五发布吧，反正没人会注意。", source: "en", target: "zh-CN", providerId: "DeepL", timestamp: "2026-11-08T11:08:00Z" },
  { id: 3, sourceText: "请求超时，请稍后重试。", translatedText: "Request timed out, please try again later.", source: "zh-CN", target: "en", providerId: "MyMemory", timestamp: "2026-11-07T21:40:00Z" },
  { id: 4, sourceText: "The build failed because the lockfile was stale.", translatedText: "构建失败，因为锁文件已过期。", source: "en", target: "zh-CN", providerId: "MyMemory", timestamp: "2026-11-07T09:15:00Z" },
  { id: 5, sourceText: "不要在生产环境里直接改数据库。", translatedText: "Do not edit the database directly in production.", source: "zh-CN", target: "en", providerId: "DeepL", timestamp: "2026-11-06T18:02:00Z" },
];

const settings = {
  source: "en",
  target: "zh-CN",
  providerId: "mymemory",
  privacyMode: "OnlinePreferred",
  fallbackEnabled: true,
  historyEnabled: true,
  shortcut: "Ctrl+Shift+T",
  shortcutError: null,
  providers: [
    { id: "mymemory", displayName: "MyMemory", endpoint: "https://api.mymemory.translated.net/get", model: null, isLocal: false, enabled: true, configured: true },
    { id: "deepl", displayName: "DeepL", endpoint: "https://api-free.deepl.com/v2", model: null, isLocal: false, enabled: true, configured: false },
    { id: "local", displayName: "本地模型", endpoint: "http://localhost:11434/v1", model: "qwen2.5", isLocal: true, enabled: true, configured: true },
  ],
};

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

async function invoke(command: string, _args?: unknown): Promise<unknown> {
  if (command.startsWith("plugin:event|")) return 1;
  if (demo === "loading" && command === "translate_text") await sleep(60_000);
  switch (command) {
    case "list_languages": return languages;
    case "list_providers": return providers;
    case "get_translation_selection": return { source: "en", target: "zh-CN", providerId: "mymemory", privacyMode: "OnlinePreferred" };
    case "get_settings": return settings;
    case "list_history": return history;
    case "translate_text":
      if (demo === "error") throw { code: "network", message: "无法连接翻译服务，请检查网络后重试。", retryable: true };
      return { translatedText: "敏捷的棕色狐狸跳过了懒狗，随后停下来思考这一举动是否值得。", providerId: "mymemory", elapsedMs: 412, characterCount: 103 };
    case "save_translation_selection":
    case "write_clipboard":
    case "save_settings":
      return null;
    case "begin_screenshot_translation": return [];
    default: return null;
  }
}

(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  invoke,
  transformCallback: () => 0,
  metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
};

createRoot(document.getElementById("root")!).render(<App />);

// ?debug=1 overlays the live viewport size, for verifying responsive breakpoints headlessly.
if (params.get("debug")) {
  const badge = document.createElement("div");
  badge.style.cssText = "position:fixed;right:4px;top:4px;z-index:99;background:#000c;color:#fff;font:600 11px monospace;padding:3px 6px;border-radius:4px";
  const update = () => { badge.textContent = `${window.innerWidth}×${window.innerHeight}`; };
  update();
  window.addEventListener("resize", update);
  document.body.appendChild(badge);
}
