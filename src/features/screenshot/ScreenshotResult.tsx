import { useEffect, useState } from "react";
import type { LanguageOptionDto, OcrResultDto } from "../../lib/commands";

export function ScreenshotResult({ result, languages = [], source = "", target = "", onConfirm, onRetry }: {
  result: OcrResultDto;
  languages?: LanguageOptionDto[];
  source?: string;
  target?: string;
  onConfirm: (text: string, source: string, target: string) => void;
  onRetry: () => void;
}) {
  const [text, setText] = useState(result.text);
  const [selectedSource, setSource] = useState(source);
  const [selectedTarget, setTarget] = useState(target);
  useEffect(() => { setText(result.text); }, [result]);
  if (!result.text.trim()) return <section className="screenshot-result"><p role="alert">未识别到文字</p><button type="button" onClick={onRetry}>重新截图</button></section>;
  return <section className="screenshot-result" aria-label="确认识别文字">
    <div className="translate-heading"><h2>确认识别文字</h2><button type="button" onClick={onRetry}>重新截图</button></div>
    {languages.length > 0 && <div className="translation-selectors">
      <label>源语言<select aria-label="截图源语言" value={selectedSource} onChange={e => setSource(e.target.value)}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select></label>
      <label>目标语言<select aria-label="截图目标语言" value={selectedTarget} onChange={e => setTarget(e.target.value)}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select></label>
    </div>}
    <label htmlFor="ocr-text">识别文字</label>
    <textarea id="ocr-text" value={text} onChange={e => setText(e.target.value)} />
    <div className="card-footer"><span>{Array.from(text).length} 字符</span><button type="button" className="translate-button" disabled={!text.trim() || !!selectedSource && selectedSource === selectedTarget} onClick={() => onConfirm(text.trim(), selectedSource, selectedTarget)}>翻译</button></div>
  </section>;
}
