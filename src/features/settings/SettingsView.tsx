import { useEffect, useState } from "react";
import { deleteProviderConfig, deleteProviderSecret, getSettings, listLanguages, saveProviderConfig, saveSettings, setProviderSecret,
  type AppErrorDto, type LanguageOptionDto, type ProviderConfigInput, type SettingsDto } from "../../lib/commands";
import { AlertIcon } from "../../components/icons";
import { safeAppError } from "../translate/translate-state";

const blankProvider = (providers: SettingsDto["providers"] = []): ProviderConfigInput => {
  const used = new Set(providers.map(provider => provider.id));
  let suffix = 1;
  let id = "openai-compatible";
  while (used.has(id)) id = `openai-compatible-${++suffix}`;
  return { id, displayName: "OpenAI 兼容服务", endpoint: "https://api.openai.com/v1", model: "gpt-4o-mini", isLocal: false, enabled: true };
};

export function SettingsView() {
  const [settings, setSettings] = useState<SettingsDto | null>(null);
  const [languages, setLanguages] = useState<LanguageOptionDto[]>([]);
  const [config, setConfig] = useState<ProviderConfigInput>(blankProvider);
  const [secret, setSecret] = useState("");
  const [error, setError] = useState<AppErrorDto | null>(null);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [addingProvider, setAddingProvider] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  useEffect(() => {
    let active = true;
    void Promise.all([getSettings(), listLanguages()]).then(([s, ls]) => {
      if (active) { setSettings(s); setLanguages(ls); setConfig(s.providers[0] ?? blankProvider(s.providers)); setError(s.shortcutError); }
    }).catch(e => { if (active) setError(safeAppError(e)); });
    return () => { active = false; };
  }, []);
  async function action(work: () => Promise<void>) {
    if (busy) return;
    setBusy(true); setError(null); setStatus("");
    try { await work(); setStatus("已保存"); }
    catch (e) { setError(safeAppError(e)); }
    finally { setBusy(false); }
  }
  const visible = settings?.providers.filter(p => p.enabled && (settings.privacyMode !== "LocalOnly" || p.isLocal)) ?? [];
  const configured = settings?.providers.find(p => p.id === config.id)?.configured ?? false;
  function chooseProvider(id: string) {
    if (id === "new") { setAddingProvider(true); setConfig(blankProvider(settings?.providers)); }
    else { setAddingProvider(false); setConfig(settings?.providers.find(p => p.id === id) ?? blankProvider(settings?.providers)); }
    setSecret(""); setStatus(""); setError(null); setConfirmDelete(false);
  }

  return <section className="settings-view" aria-label="设置">
    <div className="settings-scroll">
      {error && <div role="alert" className="alert"><AlertIcon /><span className="msg">{error.message}</span></div>}
      {status && <p role="status" className="notice">{status}</p>}
      {!settings ? <p className="empty">正在加载设置…</p> : <>
        <section className="group">
          <div className="group-head"><h2>翻译偏好</h2><span className="desc">新翻译的默认行为</span></div>
          <div className="grid-2">
            <label className="field"><span>默认源语言</span>
              <select value={settings.source} disabled={busy} onChange={e => setSettings({...settings,source:e.target.value})}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select>
            </label>
            <label className="field"><span>默认目标语言</span>
              <select value={settings.target} disabled={busy} onChange={e => setSettings({...settings,target:e.target.value})}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select>
            </label>
            <label className="field"><span>隐私模式</span>
              <select value={settings.privacyMode} disabled={busy} onChange={e => {
                const privacyMode = e.target.value as SettingsDto["privacyMode"];
                const allowed = settings.providers.filter(p => p.enabled && (privacyMode !== "LocalOnly" || p.isLocal));
                setSettings({...settings,privacyMode,providerId:allowed.find(p => p.id === settings.providerId)?.id ?? allowed[0]?.id ?? ""});
              }}><option value="OnlinePreferred">在线优先</option><option value="LocalOnly">敏感模式 · 仅本地</option></select>
            </label>
            <label className="field"><span>默认翻译服务</span>
              <select value={settings.providerId} disabled={busy} onChange={e => setSettings({...settings,providerId:e.target.value})}>{!visible.length && <option value="">请先配置本地服务</option>}{visible.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}</select>
            </label>
          </div>
          <div className="row">
            <label className="field grow"><span>截图快捷键</span>
              <input value={settings.shortcut} disabled={busy} onChange={e => setSettings({...settings,shortcut:e.target.value})} placeholder="Ctrl+Shift+T" />
            </label>
            <div className="kbd-row" aria-hidden="true"><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>T</kbd></div>
          </div>
          <label className="switch-row">
            <span className="grow"><span className="label">允许服务回退</span><span className="sub">当前服务不可用时自动尝试其他已配置服务</span></span>
            <input type="checkbox" className="switch" aria-label="允许服务回退" checked={settings.fallbackEnabled} disabled={busy}
              onChange={e => setSettings({...settings,fallbackEnabled:e.target.checked})} />
          </label>
          <label className="switch-row">
            <span className="grow"><span className="label">保存翻译历史</span><span className="sub">关闭后不再新增记录，已有记录保留</span></span>
            <input type="checkbox" className="switch" aria-label="保存翻译历史" checked={settings.historyEnabled} disabled={busy}
              onChange={e => setSettings({...settings,historyEnabled:e.target.checked})} />
          </label>
          <p className="note-line">敏感模式只使用本地服务；历史保存在本机 SQLite。</p>
          <div className="settings-actions">
            <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void action(async () => {
              const {source,target,providerId,privacyMode,fallbackEnabled,historyEnabled,shortcut} = settings;
              await saveSettings({source,target,providerId,privacyMode,fallbackEnabled,historyEnabled,shortcut});
            })}>保存设置</button>
          </div>
        </section>

        <section className="group">
          <div className="group-head"><h2>翻译服务与凭据</h2><span className="desc">密钥存于系统凭据管理器</span></div>
          <div className="row">
            <label className="field grow"><span>配置服务</span>
              <select value={addingProvider ? "new" : config.id} disabled={busy} onChange={e => chooseProvider(e.target.value)}>{settings.providers.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}<option value="new">添加服务…</option></select>
            </label>
            <span className="badge">{config.id === "mymemory" ? "内置" : configured ? "已配置" : "待配置"}</span>
          </div>
          <div className="grid-2">
            <label className="field"><span>服务 ID</span>
              <input value={config.id} disabled={busy} onChange={e => {setConfig({...config,id:e.target.value});setSecret("");}} />
            </label>
            <label className="field"><span>服务名称</span>
              <input value={config.displayName} disabled={busy} onChange={e => setConfig({...config,displayName:e.target.value})} />
            </label>
            <label className="field"><span>服务地址</span>
              <input value={config.endpoint ?? ""} disabled={busy} onChange={e => setConfig({...config,endpoint:e.target.value || null})} placeholder="http://localhost:11434/v1" />
            </label>
            <label className="field"><span>模型</span>
              <input value={config.model ?? ""} disabled={busy} onChange={e => setConfig({...config,model:e.target.value || null})} />
            </label>
          </div>
          <label className="switch-row">
            <span className="grow"><span className="label">本地服务</span><span className="sub">仅限回环地址，文本不离开本机</span></span>
            <input type="checkbox" className="switch" aria-label="本地服务" checked={config.isLocal} disabled={busy}
              onChange={e => setConfig({...config,isLocal:e.target.checked})} />
          </label>
          <label className="switch-row">
            <span className="grow"><span className="label">启用服务</span></span>
            <input type="checkbox" className="switch" aria-label="启用服务" checked={config.enabled} disabled={busy}
              onChange={e => setConfig({...config,enabled:e.target.checked})} />
          </label>
          <p className="note-line">MyMemory 使用 ID mymemory，DeepL 使用 deepl；其他服务使用 OpenAI 兼容协议，需填写模型与服务地址。在线翻译会将原文发送给所选服务。</p>
          <div className="settings-actions">
            <button type="button" className="btn-ghost" disabled={busy} onClick={() => void action(async () => {
              const {id,displayName,endpoint,model,isLocal,enabled} = config;
              if (addingProvider && settings.providers.some(provider => provider.id === id)) throw {code:"configuration",message:"服务 ID 已存在，请使用新的 ID。",retryable:false};
              await saveProviderConfig({id,displayName,endpoint,model,isLocal,enabled});
              const loaded = await getSettings(); setSettings({...loaded, source:settings.source,target:settings.target,privacyMode:settings.privacyMode,historyEnabled:settings.historyEnabled,fallbackEnabled:settings.fallbackEnabled,shortcut:settings.shortcut});
              setAddingProvider(false); setConfig(loaded.providers.find(provider => provider.id === id) ?? config);
            })}>保存服务</button>
            {confirmDelete ? <div className="settings-actions" role="alertdialog" aria-label="删除服务确认">
              <span>删除 {config.displayName || config.id}？服务配置与已保存的密钥都会被移除。</span>
              <button type="button" className="btn-ghost danger" disabled={busy} onClick={() => void action(async () => {
                await deleteProviderConfig(config.id);
                const loaded = await getSettings();
                setSettings({...loaded, source:settings.source,target:settings.target,privacyMode:settings.privacyMode,historyEnabled:settings.historyEnabled,fallbackEnabled:settings.fallbackEnabled,shortcut:settings.shortcut});
                setAddingProvider(false); setConfirmDelete(false); setSecret("");
                setConfig(loaded.providers[0] ?? blankProvider(loaded.providers));
              })}>确认删除</button>
              <button type="button" className="btn-ghost" disabled={busy} onClick={() => setConfirmDelete(false)}>取消</button>
            </div> : <button type="button" className="btn-ghost danger" disabled={busy || !settings.providers.some(provider => provider.id === config.id)} onClick={() => setConfirmDelete(true)}>删除服务</button>}
          </div>
          <div className="row">
            <label className="field grow"><span>API 密钥</span>
              <input type="password" disabled={busy || config.id === "mymemory"} autoComplete="new-password" value={secret} onChange={e => setSecret(e.target.value)} />
            </label>
            <div className="ops">
              <button type="button" className="btn-ghost" disabled={busy || !secret.trim()} onClick={() => void action(async () => {
                const value = secret; setSecret(""); await setProviderSecret(config.id, value);
                setSettings({...settings,providers:settings.providers.map(p => p.id === config.id ? {...p,configured:true} : p)});
              })}>保存密钥</button>
              <button type="button" className="btn-ghost danger" disabled={busy || config.id === "mymemory"} onClick={() => void action(async () => {
                setSecret(""); await deleteProviderSecret(config.id);
                setSettings({...settings,providers:settings.providers.map(p => p.id === config.id ? {...p,configured:false} : p)});
              })}>删除密钥</button>
            </div>
          </div>
          <p className="status-text" aria-label="密钥状态">{config.id === "mymemory" ? "无需密钥" : configured ? "已配置" : "未配置"}</p>
          <p className="note-line">密钥保存在系统凭据管理器中；保存后输入框清空。请先保存服务配置。</p>
        </section>
      </>}
    </div>
  </section>;
}
