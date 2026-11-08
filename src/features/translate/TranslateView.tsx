import { useEffect, useRef, useState } from "react";
import { getTranslationSelection, listLanguages, listProviders, saveTranslationSelection, translateText, writeClipboard,
  type AppErrorDto, type LanguageOptionDto, type ProviderOptionDto, type TranslationResultDto } from "../../lib/commands";
import { safeAppError, utf8Bytes, validateTranslation } from "./translate-state";

export interface TranslateViewProps { draftText?: string }

export function TranslateView({ draftText }: TranslateViewProps = {}) {
  const [languages, setLanguages] = useState<LanguageOptionDto[]>([]);
  const [providers, setProviders] = useState<ProviderOptionDto[]>([]);
  const [source, setSource] = useState("en");
  const [target, setTarget] = useState("zh-CN");
  const [providerId, setProviderId] = useState("");
  const [localOnly, setLocalOnly] = useState(false);
  const [text, setText] = useState(draftText ?? "");
  const [result, setResult] = useState<TranslationResultDto | null>(null);
  const [error, setError] = useState<AppErrorDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState(false);
  const [copied, setCopied] = useState(false);
  const [retryAction, setRetryAction] = useState<"translate" | "copy">("translate");
  const saves = useRef<Promise<void>>(Promise.resolve());
  const submitting = useRef(false);

  useEffect(() => {
    if (draftText !== undefined) { setText(draftText); setResult(null); setCopied(false); setError(null); }
  }, [draftText]);

  useEffect(() => {
    let active = true;
    Promise.all([listLanguages(), listProviders(), getTranslationSelection()]).then(([ls, ps, selection]) => {
      if (!active) return;
      const visible = ps.filter(p => selection.privacyMode !== "LocalOnly" || p.isLocal);
      setLanguages(ls); setProviders(visible); setSource(selection.source); setTarget(selection.target);
      setProviderId(visible.find(p => p.id === selection.providerId)?.id ?? visible.find(p => p.isDefault)?.id ?? visible[0]?.id ?? "");
      setLocalOnly(selection.privacyMode === "LocalOnly");
    }).catch(e => { if (active) setError(safeAppError(e)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);

  const provider = providers.find(p => p.id === providerId);
  const bytes = utf8Bytes(text);
  const overLimit = provider?.limitBytes !== undefined && bytes > provider.limitBytes;
  function persist(nextSource: string, nextTarget: string, nextProvider: string) {
    if (!nextProvider) return;
    // Serialize changes so a slow earlier write cannot overwrite the final selection.
    saves.current = saves.current.catch(() => undefined).then(() => saveTranslationSelection(nextSource, nextTarget, nextProvider));
    void saves.current.catch(e => setError(safeAppError(e)));
  }
  async function translate() {
    if (submitting.current) return;
    const invalid = validateTranslation(text, source, target, providerId, provider?.limitBytes);
    if (invalid) { setError(invalid); return; }
    submitting.current = true; setPending(true); setError(null); setCopied(false); setResult(null); setRetryAction("translate");
    try {
      await saves.current.catch(() => undefined);
      await saveTranslationSelection(source, target, providerId);
      setResult(await translateText(text, source, target, providerId));
    } catch (e) { setError(safeAppError(e)); }
    finally { submitting.current = false; setPending(false); }
  }
  async function copy() {
    if (!result) return;
    setRetryAction("copy");
    try { await writeClipboard(result.translatedText); setCopied(true); setError(null); }
    catch (e) { setError(safeAppError(e)); }
  }
  const disabled = loading || pending;
  return (
    <section className="translate-view" aria-label="文本翻译">
      <div className="translate-heading"><h2>文本翻译</h2><span className="mode-badge">{localOnly ? "仅本地 · 文本不发送至在线服务" : "在线优先"}</span></div>
      <div className="translation-selectors">
        <label>源语言<select aria-label="源语言" value={source} disabled={disabled} onChange={e => { setSource(e.target.value); persist(e.target.value, target, providerId); }}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select></label>
        <span className="direction" aria-hidden="true">→</span>
        <label>目标语言<select aria-label="目标语言" value={target} disabled={disabled} onChange={e => { setTarget(e.target.value); persist(source, e.target.value, providerId); }}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select></label>
        <label className="provider-selector">翻译服务<select aria-label="翻译服务" value={providerId} disabled={disabled || !providers.length} onChange={e => { setProviderId(e.target.value); persist(source, target, e.target.value); }}>{!providers.length && <option value="">暂无可用服务</option>}{providers.map(p => <option key={p.id} value={p.id}>{p.displayName}{!p.configured ? "（待配置）" : ""}</option>)}</select></label>
      </div>
      {provider?.notice && <p className="provider-notice">{provider.notice}</p>}
      {!loading && !providers.length && <p className="provider-notice">{localOnly ? "请在设置中启用本地翻译服务。" : "请在设置中配置翻译服务。"}</p>}
      <div className="translation-workspace">
        <div className="text-card"><label htmlFor="source-text">原文</label><textarea id="source-text" value={text} disabled={pending} placeholder="输入或粘贴要翻译的文本…" onChange={e => { setText(e.target.value); setCopied(false); }} /><div className="card-footer"><span className={overLimit ? "char-counter over-limit" : "char-counter"} aria-label="原文长度">{provider?.limitBytes !== undefined ? `${bytes} / ${provider.limitBytes} 字节` : `${Array.from(text).length} 字符`}</span><button className="translate-button" disabled={disabled || !provider?.configured || overLimit} onClick={() => void translate()}>{pending ? "翻译中…" : "翻译"}</button></div></div>
        <div className="text-card result-card"><div className="result-heading"><h3>译文</h3><button className="copy-button" disabled={!result || pending} onClick={() => void copy()}>复制译文</button></div><div className={`translation-output${result ? "" : " empty-output"}`} aria-label="译文" aria-live="polite">{result?.translatedText ?? (pending ? "正在翻译，请稍候…" : loading ? "正在加载翻译配置…" : "译文将显示在这里")}</div><div className="card-footer"><span>{result ? `${providers.find(p => p.id === result.providerId)?.displayName ?? result.providerId} · ${result.elapsedMs} ms` : ""}</span>{copied && <span role="status">已复制</span>}</div></div>
      </div>
      {error && <div className="inline-error" role="alert"><span>{error.message}{error.retryAfterSeconds !== undefined ? `（约 ${error.retryAfterSeconds} 秒后可重试）` : ""}</span>{error.retryable && <button type="button" disabled={pending} onClick={() => void (retryAction === "copy" ? copy() : translate())}>重试</button>}</div>}
    </section>
  );
}
