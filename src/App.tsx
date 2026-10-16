import { useCallback, useEffect, useRef, useState } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { beginScreenshotTranslation, cancelScreenshotTranslation, getScreenshotDisplay, getSettings, getTranslationSelection, listLanguages,
  recognizeScreenshotRegion, saveTranslationSelection, type AppErrorDto, type CapturedDisplayDto,
  type LanguageOptionDto, type OcrResultDto, type ScreenshotRect, type TranslationSelectionDto } from "./lib/commands";
import { ScreenshotOverlay } from "./features/screenshot/ScreenshotOverlay";
import { ScreenshotResult } from "./features/screenshot/ScreenshotResult";
import { TranslateView } from "./features/translate/TranslateView";
import { SettingsView } from "./features/settings/SettingsView";
import { HistoryView } from "./features/history/HistoryView";

const views = ["Translate", "History", "Settings"] as const;
type View = (typeof views)[number];
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
  const [activeView, setActiveView] = useState<View>("Translate");
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

  return (
    <div className="app-shell">
      <header className="title-bar">
        <h1>Quick Translate</h1>
      </header>
      <nav className="primary-nav" aria-label="Main navigation">
        {views.map((view) => (
          <button
            key={view}
            type="button"
            className={`nav-button${activeView === view ? " active" : ""}`}
            aria-current={activeView === view ? "page" : undefined}
            onClick={() => setActiveView(view)}
          >
            {view}
          </button>
        ))}
      </nav>
      <main className="main-panel" aria-label={activeView}>
        {desktopError && <div role="alert" className="inline-error"><span>{desktopError.message}</span><button onClick={() => {setActiveView("Settings");setDesktopError(null);}}>打开设置</button></div>}
        {activeView === "Settings" && <SettingsView />}
        {activeView === "History" && <HistoryView />}
        {activeView === "Translate" && <>
          <div className="capture-toolbar"><button type="button" disabled={capturing} onClick={() => void startScreenshot()}>截图翻译</button></div>
          {screenshotError && <div className="inline-error screenshot-error" role="alert"><span>{screenshotError.message || "截图失败，请重试。"}</span><button type="button" onClick={() => void startScreenshot()}>重新截图</button></div>}
          {ocrResult ? <ScreenshotResult result={ocrResult} languages={languages} source={selection?.source} target={selection?.target}
            onConfirm={(text, source, target) => void confirm(text, source, target)} onRetry={() => void startScreenshot()} />
            : <TranslateView draftText={draftText} />}
        </>}
      </main>
    </div>
  );
}

export function App() {
  const displayId = new URLSearchParams(window.location.search).get("screenshot");
  return displayId ? <OverlayWindow displayId={displayId} /> : <MainApp />;
}
