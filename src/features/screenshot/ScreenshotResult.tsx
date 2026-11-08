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
  if (!result.text.trim()) return <section className="screenshot-result">
    <div className="result-head"><h2>未识别到文字</h2><div className="ops"><button type="button" className="btn-ghost" onClick={onRetry}>重新截图</button></div></div>
  </section>;
  return <section className="screenshot-result" aria-label="确认识别文字">
    <div className="result-head">
      <h2>确认识别文字</h2>
      <div className="ops"><button type="button" className="btn-ghost" onClick={onRetry}>重新截图</button></div>
    </div>
    {languages.length > 0 && <div className="selectors">
      <label className="field"><span>源语言</span>
        <select aria-label="截图源语言" value={selectedSource} onChange={e => setSource(e.target.value)}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select>
      </label>
      <label className="field"><span>目标语言</span>
        <select aria-label="截图目标语言" value={selectedTarget} onChange={e => setTarget(e.target.value)}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select>
      </label>
    </div>}
    <div className="result-body">
      <textarea id="ocr-text" aria-label="识别文字" value={text} spellCheck={false} onChange={e => setText(e.target.value)} />
      <div className="result-foot">
        <span>{Array.from(text).length} 字符</span>
        <button type="button" className="btn btn-primary" disabled={!text.trim() || !!selectedSource && selectedSource === selectedTarget}
          onClick={() => onConfirm(text.trim(), selectedSource, selectedTarget)}>翻译</button>
      </div>
    </div>
  </section>;
}
