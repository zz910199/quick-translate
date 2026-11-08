import { useEffect, useState } from "react";
import { clearHistory, deleteHistoryEntry, listHistory, retranslateHistoryEntry, writeClipboard, type AppErrorDto, type HistoryEntryDto } from "../../lib/commands";
import { AlertIcon, CopyIcon, OpenIcon, RetranslateIcon, SearchIcon, TrashIcon } from "../../components/icons";
import { safeAppError } from "../translate/translate-state";

export function HistoryView() {
  const [entries, setEntries] = useState<HistoryEntryDto[]>([]);
  const [opened, setOpened] = useState<HistoryEntryDto | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [error, setError] = useState<AppErrorDto | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);
  const [notice, setNotice] = useState("");
  const [query, setQuery] = useState("");
  useEffect(() => {
    let active = true;
    void listHistory().then(rows => {if(active)setEntries(rows);}).catch(e => {if(active)setError(safeAppError(e));}).finally(() => {if(active)setLoading(false);});
    return () => {active=false;};
  }, []);
  async function action(work: () => Promise<void>) {
    if(busy)return; setBusy(true);setError(null);setCopied(false);setNotice("");
    try {await work();} catch(e){setError(safeAppError(e));}finally{setBusy(false);}
  }
  /** Rewrites the stored row in place; retranslating never appends a second entry. */
  async function retranslate(entry: HistoryEntryDto, sourceText?: string) {
    const updated = await retranslateHistoryEntry(entry.id, sourceText);
    setEntries(current => current.map(item => item.id === updated.id ? updated : item));
    setOpened(current => current && current.id === updated.id ? updated : current);
    setNotice(updated.providerId === entry.providerId
      ? "已更新这条记录"
      : `原服务不可用，已改用 ${updated.providerId} 并更新这条记录`);
  }
  function open(entry: HistoryEntryDto) {
    setOpened(entry); setCopied(false); setNotice("");
  }
  const needle = query.trim().toLowerCase();
  const shown = needle
    ? entries.filter(entry => `${entry.sourceText} ${entry.translatedText}`.toLowerCase().includes(needle))
    : entries;

  return <section className="history-view" aria-label="历史记录">
    <div className="toolrow">
      <label className="search">
        <SearchIcon />
        <input aria-label="搜索历史" placeholder="搜索原文或译文" value={query} onChange={event => setQuery(event.target.value)} />
      </label>
      <span className="count" aria-label="记录条数">{shown.length} 条</span>
      <span className="spacer" />
      <button type="button" className="btn-ghost danger" disabled={busy || !entries.length} onClick={() => setConfirmClear(true)}>清空历史</button>
    </div>

    {error && <div role="alert" className="alert"><AlertIcon /><span className="msg">{error.message}</span></div>}
    {notice && <p role="status" className="notice">{notice}</p>}

    {confirmClear && (
      <div role="alertdialog" aria-label="清空历史确认" className="confirm">
        <p>确认清空全部历史记录？此操作无法撤销。</p>
        <button type="button" className="btn-ghost danger" disabled={busy}
          onClick={() => void action(async () => {await clearHistory();setEntries([]);setOpened(null);setConfirmClear(false);})}>确认清空</button>
        <button type="button" className="btn-ghost" disabled={busy} onClick={() => setConfirmClear(false)}>取消</button>
      </div>
    )}

    {opened && (
      <section className="detail" aria-label="历史详情">
        <label>历史原文
          <textarea value={opened.sourceText} disabled={busy} onChange={e => setOpened({...opened,sourceText:e.target.value})} />
        </label>
        <div aria-label="历史译文" className="output-inline">{opened.translatedText}</div>
        <p className="meta">{opened.source} → {opened.target} · {opened.providerId} · {new Date(opened.timestamp).toLocaleString()}</p>
        <div className="actions">
          <button type="button" className="btn-ghost" disabled={busy}
            onClick={() => void action(async () => {await writeClipboard(opened.translatedText);setCopied(true);})}>复制译文</button>
          <button type="button" className="btn-ghost" disabled={busy}
            onClick={() => void action(() => retranslate(opened, opened.sourceText))}>重新翻译</button>
          <button type="button" className="btn-ghost" disabled={busy} onClick={() => setOpened(null)}>关闭详情</button>
          {copied && <span role="status" className="saved">已复制</span>}
        </div>
      </section>
    )}

    {loading
      ? <p className="empty">正在加载历史…</p>
      : !shown.length
        ? <p className="empty">{needle ? "没有匹配的记录" : "暂无历史记录"}</p>
        : <ol className="hist-list">{shown.map(entry => <li key={entry.id}>
            <article className="hist-card" onClick={() => open(entry)}>
              <p className="hist-src">{entry.sourceText}</p>
              <p className="hist-dst">{entry.translatedText}</p>
              <div className="hist-meta">
                <span className="meta">{entry.providerId}</span>
                <span className="meta">·</span>
                <span className="meta">{new Date(entry.timestamp).toLocaleString()}</span>
                <div className="ops" onClick={event => event.stopPropagation()}>
                  <button type="button" className="mini" aria-label="重新打开" title="打开详情" disabled={busy} onClick={() => open(entry)}>
                    <OpenIcon />
                  </button>
                  <button type="button" className="mini" aria-label="重新翻译此条" title="重新翻译" disabled={busy}
                    onClick={() => void action(() => retranslate(entry))}>
                    <RetranslateIcon />
                  </button>
                  <button type="button" className="mini" aria-label="复制" title="复制译文" disabled={busy}
                    onClick={() => void action(async () => {await writeClipboard(entry.translatedText);setCopied(true);})}>
                    <CopyIcon />
                  </button>
                  <button type="button" className="mini" aria-label="删除" title="删除这条记录" disabled={busy}
                    onClick={() => void action(async () => {
                      await deleteHistoryEntry(entry.id);
                      setEntries(current => current.filter(item => item.id !== entry.id));
                      if (opened?.id === entry.id) setOpened(null);
                    })}>
                    <TrashIcon />
                  </button>
                </div>
              </div>
            </article>
          </li>)}</ol>}
  </section>;
}
