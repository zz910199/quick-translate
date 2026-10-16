import { useEffect, useState } from "react";
import { clearHistory, deleteHistoryEntry, listHistory, translateText, writeClipboard, type AppErrorDto, type HistoryEntryDto } from "../../lib/commands";
import { safeAppError } from "../translate/translate-state";

export function HistoryView() {
  const [entries, setEntries] = useState<HistoryEntryDto[]>([]);
  const [opened, setOpened] = useState<HistoryEntryDto | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [error, setError] = useState<AppErrorDto | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    let active = true;
    void listHistory().then(rows => {if(active)setEntries(rows);}).catch(e => {if(active)setError(safeAppError(e));}).finally(() => {if(active)setLoading(false);});
    return () => {active=false;};
  }, []);
  async function action(work: () => Promise<void>) {
    if(busy)return; setBusy(true);setError(null);setCopied(false);
    try {await work();} catch(e){setError(safeAppError(e));}finally{setBusy(false);}
  }
  return <section className="history-view" aria-label="历史记录">
    <div className="translate-heading"><h2>历史记录</h2><button disabled={busy || !entries.length} onClick={() => setConfirmClear(true)}>清空历史</button></div>
    <p className="provider-notice">最近 100 条，按时间从新到旧。重新翻译将使用当前隐私设置。</p>
    {error && <p role="alert" className="inline-error">{error.message}</p>}
    {confirmClear && <div role="alertdialog" aria-label="清空历史确认" className="history-confirm"><p>确认清空全部历史记录？此操作无法撤销。</p><button disabled={busy} onClick={() => void action(async () => {await clearHistory();setEntries([]);setOpened(null);setConfirmClear(false);})}>确认清空</button><button disabled={busy} onClick={() => setConfirmClear(false)}>取消</button></div>}
    {opened && <section className="history-detail" aria-label="历史详情">
      <label>历史原文<textarea value={opened.sourceText} disabled={busy} onChange={e => setOpened({...opened,sourceText:e.target.value})} /></label>
      <div aria-label="历史译文" className="translation-output">{opened.translatedText}</div>
      <p>{opened.source} → {opened.target} · {opened.providerId}</p>
      <button disabled={busy} onClick={() => void action(async () => {await writeClipboard(opened.translatedText);setCopied(true);})}>复制译文</button>
      <button disabled={busy} onClick={() => void action(async () => {const result=await translateText(opened.sourceText,opened.source,opened.target,opened.providerId);setOpened({...opened,translatedText:result.translatedText,providerId:result.providerId});setEntries(await listHistory());})}>重新翻译</button>
      <button disabled={busy} onClick={() => setOpened(null)}>关闭详情</button>
      {copied && <span role="status">已复制</span>}
    </section>}
    {loading ? <p>正在加载历史…</p> : !entries.length ? <p>暂无历史记录</p> : <ol className="history-list">{entries.map(entry => <li key={entry.id}>
      <div className="history-text"><p>{entry.sourceText}</p><p>{entry.translatedText}</p><small>{entry.source} → {entry.target} · {entry.providerId} · {new Date(entry.timestamp).toLocaleString()}</small></div>
      <div className="settings-actions"><button disabled={busy} onClick={() => {setOpened(entry);setCopied(false);}}>重新打开</button><button disabled={busy} onClick={() => void action(async () => {const result=await translateText(entry.sourceText,entry.source,entry.target,entry.providerId);setOpened({...entry,translatedText:result.translatedText,providerId:result.providerId});setEntries(await listHistory());})}>重新翻译此条</button><button disabled={busy} onClick={() => void action(async () => {await deleteHistoryEntry(entry.id);setEntries(entries.filter(e => e.id!==entry.id));if(opened?.id===entry.id)setOpened(null);})}>删除</button></div>
    </li>)}</ol>}
  </section>;
}
