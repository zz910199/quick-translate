import { useEffect, useRef, useState } from "react";
import { getTranslationSelection, listLanguages, listProviders, saveTranslationSelection, translateText, writeClipboard,
  type AppErrorDto, type LanguageOptionDto, type ProviderOptionDto, type TranslationResultDto } from "../../lib/commands";
import { AlertIcon, CheckIcon, ClearIcon, CopyIcon, SwapIcon } from "../../components/icons";
import { safeAppError, utf8Bytes, validateTranslation } from "./translate-state";

export interface TranslateViewProps {
  draftText?: string;
  /** Errors raised outside this view (screenshot capture) surface in the same place as local ones. */
  externalError?: { error: AppErrorDto; actionLabel: string; onAction: () => void } | null;
}

export function TranslateView({ draftText, externalError }: TranslateViewProps = {}) {
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
  const input = useRef<HTMLTextAreaElement>(null);

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
  function swap() {
    if (pending || loading) return;
    const nextSource = target;
    const nextTarget = source;
    setSource(nextSource); setTarget(nextTarget);
    // Carry the finished translation back into the source box, the way chat translators do.
    if (result) { setText(result.translatedText); setResult(null); setCopied(false); }
    persist(nextSource, nextTarget, providerId);
  }
  function clear() {
    setText(""); setResult(null); setCopied(false); setError(null);
    input.current?.focus();
  }
  const disabled = loading || pending;
  const shownError = externalError?.error ?? error;
  const retryLabel = externalError ? externalError.actionLabel : "重试";
  function retry() {
    if (externalError) { externalError.onAction(); return; }
    void (retryAction === "copy" ? copy() : translate());
  }
  const status = localOnly
    ? "仅本地 · 文本不离开本机"
    : provider?.notice ?? (providers.length ? "在线服务 · 原文将发送至所选服务提供方" : "请先在设置中配置翻译服务");

  return (
    <section className="translate-view" aria-label="文本翻译"
      onKeyDown={event => {
        if ((event.ctrlKey || event.metaKey) && event.key === "Enter") { event.preventDefault(); void translate(); }
      }}>
      <div className="langbar">
        <div className="lang">
          <select aria-label="源语言" value={source} disabled={disabled}
            onChange={e => { setSource(e.target.value); persist(e.target.value, target, providerId); }}>
            {languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}
          </select>
        </div>
        <button type="button" className="swap" aria-label="互换语言" title="互换语言" disabled={disabled} onClick={swap}>
          <SwapIcon />
        </button>
        <div className="lang">
          <select aria-label="目标语言" value={target} disabled={disabled}
            onChange={e => { setTarget(e.target.value); persist(source, e.target.value, providerId); }}>
            {languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}
          </select>
        </div>
        <div className={`provider-chip${localOnly ? " local" : ""}`}>
          <span className={`dot${localOnly ? " local" : ""}`} aria-hidden="true" />
          <select aria-label="翻译服务" value={providerId} disabled={disabled || !providers.length}
            onChange={e => { setProviderId(e.target.value); persist(source, target, e.target.value); }}>
            {!providers.length && <option value="">暂无可用服务</option>}
            {providers.map(p => <option key={p.id} value={p.id}>{p.displayName}{!p.configured ? "（待配置）" : ""}</option>)}
          </select>
        </div>
      </div>

      <div className="workspace">
        <section className="pane">
          <div className="pane-head">
            <label className="pane-title" htmlFor="source-text">原文</label>
            <div className="pane-tools">
              <button type="button" className="mini" aria-label="清空原文" title="清空" disabled={!text} onClick={clear}>
                <ClearIcon />
              </button>
            </div>
          </div>
          <div className="pane-body">
            <textarea id="source-text" ref={input} value={text} disabled={pending} spellCheck={false}
              placeholder="输入或粘贴文本，Ctrl+Enter 翻译"
              onChange={e => { setText(e.target.value); setCopied(false); }} />
          </div>
          <div className="pane-foot">
            <span className={overLimit ? "over" : ""} aria-label="原文长度">
              {provider?.limitBytes !== undefined ? `${bytes} / ${provider.limitBytes} 字节` : `${Array.from(text).length} 字符`}
            </span>
          </div>
        </section>

        <section className="pane">
          <div className="pane-head">
            <span className="pane-title">译文</span>
            <div className="pane-tools">
              <button type="button" className="mini" aria-label="复制译文" disabled={!result || pending}
                data-state={copied ? "done" : undefined} onClick={() => void copy()}>
                {copied ? <CheckIcon /> : <CopyIcon />}{copied ? "已复制" : "复制"}
              </button>
            </div>
          </div>
          <div className="pane-body">
            {pending
              ? <div className="skeleton" aria-hidden="true"><i style={{ width: "92%" }} /><i style={{ width: "100%" }} /><i style={{ width: "86%" }} /><i style={{ width: "64%" }} /></div>
              : result
                ? <div className="output" aria-label="译文" aria-live="polite">{result.translatedText}</div>
                : <div className="output empty" aria-label="译文" aria-live="polite">
                    <span>{loading ? "正在加载翻译配置…" : "译文将显示在这里"}</span>
                    {!loading && <span className="hint">按 Ctrl + Enter 立即翻译</span>}
                  </div>}
          </div>
          <div className="pane-foot">
            <span>{result ? `${providers.find(p => p.id === result.providerId)?.displayName ?? result.providerId} · ${result.elapsedMs} ms` : "—"}</span>
            <span className="right">{copied && <span className="saved" role="status">已复制</span>}</span>
          </div>
        </section>
      </div>

      {shownError && (
        <div className="alert" role="alert">
          <AlertIcon />
          <span className="msg">{shownError.message}{shownError.retryAfterSeconds !== undefined ? `（约 ${shownError.retryAfterSeconds} 秒后可重试）` : ""}</span>
          {(externalError || shownError.retryable) && <button type="button" disabled={pending} onClick={retry}>{retryLabel}</button>}
        </div>
      )}

      <footer className="actionbar">
        <div className="status" title={status}>
          <span className={`dot${localOnly ? " local" : ""}`} aria-hidden="true" />
          <span>{status}</span>
        </div>
        <div className="actionbar-right">
          <span className="kbd-hint" aria-hidden="true"><kbd>Ctrl</kbd><kbd>Enter</kbd></span>
          <button type="button" className="btn btn-primary" disabled={disabled || !provider?.configured || overLimit}
            onClick={() => void translate()}>
            {pending ? <><span className="spinner" aria-hidden="true" />翻译中…</> : "翻译"}
          </button>
        </div>
      </footer>
    </section>
  );
}
