import { useEffect, useState } from "react";
import { deleteProviderSecret, getSettings, listLanguages, saveProviderConfig, saveSettings, setProviderSecret,
  type AppErrorDto, type LanguageOptionDto, type ProviderConfigInput, type SettingsDto } from "../../lib/commands";
import { safeAppError } from "../translate/translate-state";

const blankProvider: ProviderConfigInput = { id: "openai-compatible", displayName: "OpenAI 兼容服务", endpoint: "https://api.openai.com/v1", model: "gpt-4o-mini", isLocal: false, enabled: true };

export function SettingsView() {
  const [settings, setSettings] = useState<SettingsDto | null>(null);
  const [languages, setLanguages] = useState<LanguageOptionDto[]>([]);
  const [config, setConfig] = useState<ProviderConfigInput>(blankProvider);
  const [secret, setSecret] = useState("");
  const [error, setError] = useState<AppErrorDto | null>(null);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    void Promise.all([getSettings(), listLanguages()]).then(([s, ls]) => {
      if (active) { setSettings(s); setLanguages(ls); setConfig(s.providers[0] ?? blankProvider); setError(s.shortcutError); }
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
    setConfig(settings?.providers.find(p => p.id === id) ?? { ...blankProvider }); setSecret(""); setStatus("");
  }
  return <section className="settings-view" aria-label="设置">
    <h2>设置</h2>
    {error && <p role="alert" className="inline-error">{error.message}</p>}
    {status && <p role="status">{status}</p>}
    {!settings ? <p>正在加载设置…</p> : <>
      <fieldset disabled={busy}><legend>翻译偏好</legend><div className="settings-grid">
        <label>默认源语言<select value={settings.source} onChange={e => setSettings({...settings,source:e.target.value})}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select></label>
        <label>默认目标语言<select value={settings.target} onChange={e => setSettings({...settings,target:e.target.value})}>{languages.map(l => <option key={l.code} value={l.code}>{l.label}</option>)}</select></label>
        <label>隐私模式<select value={settings.privacyMode} onChange={e => {
          const privacyMode = e.target.value as SettingsDto["privacyMode"];
          const allowed = settings.providers.filter(p => p.enabled && (privacyMode !== "LocalOnly" || p.isLocal));
          setSettings({...settings,privacyMode,providerId:allowed.find(p => p.id === settings.providerId)?.id ?? allowed[0]?.id ?? ""});
        }}><option value="OnlinePreferred">在线优先</option><option value="LocalOnly">敏感模式 · 仅本地</option></select></label>
        <label>默认翻译服务<select value={settings.providerId} onChange={e => setSettings({...settings,providerId:e.target.value})}>{!visible.length && <option value="">请先配置本地服务</option>}{visible.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}</select></label>
        <label>截图快捷键<input value={settings.shortcut} onChange={e => setSettings({...settings,shortcut:e.target.value})} placeholder="Ctrl+Shift+T" /></label>
      </div>
      <label className="check-label"><input type="checkbox" checked={settings.fallbackEnabled} onChange={e => setSettings({...settings,fallbackEnabled:e.target.checked})} />允许服务回退</label>
      <label className="check-label"><input type="checkbox" checked={settings.historyEnabled} onChange={e => setSettings({...settings,historyEnabled:e.target.checked})} />保存翻译历史</label>
      <p className="provider-notice">关闭历史记录后，新翻译不再保存；已有记录可在历史中手动清除。敏感模式只使用本地服务。</p>
      <button type="button" onClick={() => void action(async () => {
        const {source,target,providerId,privacyMode,fallbackEnabled,historyEnabled,shortcut} = settings;
        await saveSettings({source,target,providerId,privacyMode,fallbackEnabled,historyEnabled,shortcut});
      })}>保存设置</button></fieldset>
      <fieldset disabled={busy}><legend>翻译服务与凭据</legend>
        <label>配置服务<select value={settings.providers.some(p => p.id === config.id) ? config.id : "new"} onChange={e => chooseProvider(e.target.value)}>{settings.providers.map(p => <option key={p.id} value={p.id}>{p.displayName}</option>)}<option value="new">添加服务…</option></select></label>
        <div className="settings-grid">
          <label>服务 ID<input value={config.id} onChange={e => {setConfig({...config,id:e.target.value});setSecret("");}} /></label>
          <label>服务名称<input value={config.displayName} onChange={e => setConfig({...config,displayName:e.target.value})} /></label>
          <label>服务地址<input value={config.endpoint ?? ""} onChange={e => setConfig({...config,endpoint:e.target.value || null})} placeholder="http://localhost:11434/v1" /></label>
          <label>模型<input value={config.model ?? ""} onChange={e => setConfig({...config,model:e.target.value || null})} /></label>
        </div>
        <label className="check-label"><input type="checkbox" checked={config.isLocal} onChange={e => setConfig({...config,isLocal:e.target.checked})} />本地服务（仅限回环地址）</label>
        <label className="check-label"><input type="checkbox" checked={config.enabled} onChange={e => setConfig({...config,enabled:e.target.checked})} />启用服务</label>
        <p className="provider-notice">MyMemory 使用 ID mymemory，DeepL 使用 deepl；其他服务使用 OpenAI 兼容协议，需填写模型与服务地址。在线翻译会将原文发送给所选服务。</p>
        <button type="button" onClick={() => void action(async () => {
          const {id,displayName,endpoint,model,isLocal,enabled} = config;
          await saveProviderConfig({id,displayName,endpoint,model,isLocal,enabled});
          const loaded = await getSettings(); setSettings({...loaded, source:settings.source,target:settings.target,privacyMode:settings.privacyMode,historyEnabled:settings.historyEnabled,fallbackEnabled:settings.fallbackEnabled,shortcut:settings.shortcut});
        })}>保存服务</button>
        <p aria-label="密钥状态">{config.id === "mymemory" ? "无需密钥" : configured ? "已配置" : "未配置"}</p>
        <label>API 密钥<input type="password" disabled={config.id === "mymemory"} autoComplete="new-password" value={secret} onChange={e => setSecret(e.target.value)} /></label>
        <p className="provider-notice">密钥保存在系统凭据管理器中；保存后输入框清空。请先保存服务配置。</p>
        <div className="settings-actions"><button type="button" disabled={!secret.trim()} onClick={() => void action(async () => {
          const value = secret; setSecret(""); await setProviderSecret(config.id, value);
          setSettings({...settings,providers:settings.providers.map(p => p.id === config.id ? {...p,configured:true} : p)});
        })}>保存密钥</button><button type="button" disabled={config.id === "mymemory"} onClick={() => void action(async () => {
          setSecret(""); await deleteProviderSecret(config.id);
          setSettings({...settings,providers:settings.providers.map(p => p.id === config.id ? {...p,configured:false} : p)});
        })}>删除密钥</button></div>
      </fieldset>
    </>}
  </section>;
}
