import { useCallback, useEffect, useRef, useState } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { beginScreenshotTranslation, cancelScreenshotTranslation, getScreenshotDisplay, getSettings, getTranslationSelection, listLanguages,
  recognizeScreenshotRegion, saveTranslationSelection, type AppErrorDto, type CapturedDisplayDto,
  type LanguageOptionDto, type OcrResultDto, type ScreenshotRect, type TranslationSelectionDto } from "./lib/commands";
import { CameraIcon } from "./components/icons";
import { ScreenshotOverlay } from "./features/screenshot/ScreenshotOverlay";
import { ScreenshotResult } from "./features/screenshot/ScreenshotResult";
import { TranslateView } from "./features/translate/TranslateView";
import { SettingsView } from "./features/settings/SettingsView";
import { HistoryView } from "./features/history/HistoryView";

const views = [
  { id: "Translate", label: "翻译" },
  { id: "History", label: "历史" },
  { id: "Settings", label: "设置" },
] as const;
type View = (typeof views)[number]["id"];
type OcrResultEvent = OcrResultDto & { displayId: string };
type OcrErrorEvent = AppErrorDto & { displayId: string };
type OcrEvent = { kind: "result"; payload: OcrResultEvent } | { kind: "error"; payload: OcrErrorEvent };

function captureError(error: unknown): AppErrorDto {
  if (error && typeof error === "object" && "code" in error && "message" in error && "retryable" in error) {
    return error as AppErrorDto;
  }
  return { code: "capture_unavailable", message: "无法获取屏幕截图，请重试。", retryable: true };
}

function OverlayWindow({ displayId }: { displayId: string }) {
  const [display, setDisplay] = useState<CapturedDisplayDto | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    void getScreenshotDisplay(displayId).then(value => { if (active) setDisplay(value); }).catch(async error => {
      if (!active) return;
      try {
        await emitTo("main", "screenshot-ocr-error", { displayId, ...captureError(error) } satisfies OcrErrorEvent);
      } finally {
        await cancelScreenshotTranslation(displayId);
      }
    });
    return () => { active = false; };
  }, [displayId]);
  const cancel = useCallback(() => { void cancelScreenshotTranslation(displayId); }, [displayId]);
  async function select(id: string, rect: ScreenshotRect) {
    if (busy) return;
    setBusy(true);
    try { await recognizeScreenshotRegion(id, rect); }
    catch (error) {
      const typed = captureError(error);
      if (typed.code !== "cancelled") await emitTo("main", "screenshot-ocr-error", { displayId: id, ...typed } satisfies OcrErrorEvent);
      await cancelScreenshotTranslation(id);
    }
  }
  return display ? <ScreenshotOverlay display={display} onSelect={(id, rect) => void select(id, rect)} onCancel={cancel} /> : null;
}

function MainApp() {
  const requested = new URLSearchParams(window.location.search).get("view");
  const startView = views.find(view => view.id === requested)?.id ?? "Translate";
  const [activeView, setActiveView] = useState<View>(startView);
  const [draftText, setDraftText] = useState<string>();
  const [ocrResult, setOcrResult] = useState<OcrResultDto | null>(null);
  const [languages, setLanguages] = useState<LanguageOptionDto[]>([]);
  const [selection, setSelection] = useState<TranslationSelectionDto | null>(null);
  const [screenshotError, setScreenshotError] = useState<AppErrorDto | null>(null);
  const [capturing, setCapturing] = useState(false);
  const [desktopError, setDesktopError] = useState<AppErrorDto | null>(null);
  const captureStarting = useRef(false);
  const captureIds = useRef(new Set<string>());
  const pendingCaptureEvents = useRef<OcrEvent[] | null>(null);

  const receiveOcrEvent = useCallback((event: OcrEvent) => {
    if (!captureIds.current.has(event.payload.displayId)) return;
    if (event.kind === "error") {
      captureIds.current.clear(); setScreenshotError(event.payload); setCapturing(false); setActiveView("Translate");
      return;
    }
    const result = event.payload;
    void Promise.all([listLanguages(), getTranslationSelection()]).then(([options, saved]) => {
      if (captureIds.current.has(result.displayId)) {
        captureIds.current.clear(); setLanguages(options); setSelection(saved); setOcrResult(result); setScreenshotError(null); setActiveView("Translate");
      }
    }).catch(error => { if (captureIds.current.has(result.displayId)) setScreenshotError(captureError(error)); });
  }, []);

  useEffect(() => {
    let active = true;
    function receive(event: OcrEvent) {
      if (!active) return;
      if (pendingCaptureEvents.current) pendingCaptureEvents.current.push(event);
      else receiveOcrEvent(event);
    }
    const success = listen<OcrResultEvent>("screenshot-ocr-result", event => {
      receive({ kind: "result", payload: event.payload });
    });
    const failure = listen<OcrErrorEvent>("screenshot-ocr-error", event => {
      receive({ kind: "error", payload: event.payload });
    });
    return () => { active = false; captureIds.current.clear(); void success.then(unlisten => unlisten()); void failure.then(unlisten => unlisten()); };
  }, [receiveOcrEvent]);

  const startScreenshot = useCallback(async () => {
    if (captureStarting.current) return;
    captureStarting.current = true;
    setActiveView("Translate");
    captureIds.current.clear();
    pendingCaptureEvents.current = [];
    setScreenshotError(null); setOcrResult(null); setCapturing(true);
    try {
      const displays = await beginScreenshotTranslation();
      captureIds.current = new Set(displays.map(display => display.id));
      const event = pendingCaptureEvents.current?.find(value => captureIds.current.has(value.payload.displayId));
      if (event) receiveOcrEvent(event);
    }
    catch (error) { setScreenshotError(error as AppErrorDto); }
    finally { pendingCaptureEvents.current = null; setCapturing(false); captureStarting.current = false; }
  }, [receiveOcrEvent]);
  useEffect(() => {
    let active = true;
    const requested = listen("screenshot-requested", () => { if (active) void startScreenshot(); });
    void getSettings().then(settings => {if(active)setDesktopError(settings.shortcutError);}).catch(() => undefined);
    return () => {active=false;void requested.then(unlisten => unlisten());};
  }, [startScreenshot]);
  async function confirm(text: string, source: string, target: string) {
    try {
      if (selection && source && target) await saveTranslationSelection(source, target, selection.providerId);
      setDraftText(text); setOcrResult(null); setScreenshotError(null); setActiveView("Translate");
    } catch (error) { setScreenshotError(error as AppErrorDto); }
  }

  /** Arrow keys move between tabs, matching the tablist pattern. */
  function moveTab(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") return;
    event.preventDefault();
    const index = views.findIndex(view => view.id === activeView);
    const next = event.key === "ArrowRight" ? (index + 1) % views.length : (index - 1 + views.length) % views.length;
    setActiveView(views[next].id);
    event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
  }

  const activeLabel = views.find(view => view.id === activeView)?.label ?? "";

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">译</span>
          <h1 className="brand-name">Quick Translate</h1>
        </div>
        <nav className="seg" role="tablist" aria-label="主导航" onKeyDown={moveTab}>
          {views.map((view) => (
            <button
              key={view.id}
              type="button"
              role="tab"
              aria-selected={activeView === view.id}
              tabIndex={activeView === view.id ? 0 : -1}
              onClick={() => setActiveView(view.id)}
            >
              {view.label}
            </button>
          ))}
        </nav>
        <div className="topbar-actions">
          <button
            type="button"
            className="icon-btn"
            aria-label="截图翻译"
            title="截图翻译 · Ctrl+Shift+S"
            disabled={capturing}
            onClick={() => void startScreenshot()}
          >
            <CameraIcon />
          </button>
        </div>
      </header>

      <main className="main-panel" aria-label={activeLabel}>
        {desktopError && <div role="alert" className="alert"><span className="msg">{desktopError.message}</span><button onClick={() => {setActiveView("Settings");setDesktopError(null);}}>打开设置</button></div>}
        {activeView === "Settings" && <SettingsView />}
        {activeView === "History" && <HistoryView />}
        {activeView === "Translate" && <>
          {ocrResult ? <ScreenshotResult result={ocrResult} languages={languages} source={selection?.source} target={selection?.target}
            onConfirm={(text, source, target) => void confirm(text, source, target)} onRetry={() => void startScreenshot()} />
            : <TranslateView
                draftText={draftText}
                externalError={screenshotError ? { error: screenshotError, actionLabel: "重新截图", onAction: () => void startScreenshot() } : null}
              />}
        </>}
      </main>
    </div>
  );
}

export function App() {
  const displayId = new URLSearchParams(window.location.search).get("screenshot");
  return displayId ? <OverlayWindow displayId={displayId} /> : <MainApp />;
}
