import { useCallback, useEffect, useState } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { beginScreenshotTranslation, cancelScreenshotTranslation, getScreenshotDisplay, getTranslationSelection, listLanguages,
  recognizeScreenshotRegion, saveTranslationSelection, type AppErrorDto, type CapturedDisplayDto,
  type LanguageOptionDto, type OcrResultDto, type ScreenshotRect, type TranslationSelectionDto } from "./lib/commands";
import { ScreenshotOverlay } from "./features/screenshot/ScreenshotOverlay";
import { ScreenshotResult } from "./features/screenshot/ScreenshotResult";
import { TranslateView } from "./features/translate/TranslateView";

const views = ["Translate", "History", "Settings"] as const;
type View = (typeof views)[number];

function OverlayWindow({ displayId }: { displayId: string }) {
  const [display, setDisplay] = useState<CapturedDisplayDto | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => { void getScreenshotDisplay(displayId).then(setDisplay).catch(() => void cancelScreenshotTranslation(displayId)); }, [displayId]);
  const cancel = useCallback(() => { void cancelScreenshotTranslation(displayId); }, [displayId]);
  async function select(id: string, rect: ScreenshotRect) {
    if (busy) return;
    setBusy(true);
    try { await recognizeScreenshotRegion(id, rect); }
    catch (error) {
      await emitTo("main", "screenshot-ocr-error", error);
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

  useEffect(() => {
    let active = true;
    const success = listen<OcrResultDto>("screenshot-ocr-result", event => {
      void Promise.all([listLanguages(), getTranslationSelection()]).then(([options, saved]) => {
        if (active) { setLanguages(options); setSelection(saved); setOcrResult(event.payload); setScreenshotError(null); setActiveView("Translate"); }
      }).catch(error => { if (active) setScreenshotError(error as AppErrorDto); });
    });
    const failure = listen<AppErrorDto>("screenshot-ocr-error", event => {
      if (active) { setScreenshotError(event.payload); setCapturing(false); setActiveView("Translate"); }
    });
    return () => { active = false; void success.then(unlisten => unlisten()); void failure.then(unlisten => unlisten()); };
  }, []);

  async function startScreenshot() {
    setScreenshotError(null); setOcrResult(null); setCapturing(true);
    try { await beginScreenshotTranslation(); }
    catch (error) { setScreenshotError(error as AppErrorDto); }
    finally { setCapturing(false); }
  }
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
