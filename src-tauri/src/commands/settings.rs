use serde::{Deserialize, Serialize};
use tauri::State;
use url::{Host, Url};

use super::translation::{list_languages, AppErrorDto, AppServices};
use crate::{
    domain::{PrivacyMode, ProviderConfig, ProviderId},
    platform::shortcut::ShortcutController,
    providers::{ProviderRegistry, CREDENTIAL_SERVICE},
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub source: String,
    pub target: String,
    pub provider_id: String,
    pub privacy_mode: PrivacyMode,
    pub fallback_enabled: bool,
    pub history_enabled: bool,
    pub shortcut: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigInput {
    pub id: String,
    pub display_name: String,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    pub is_local: bool,
    pub enabled: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsDto {
    #[serde(flatten)]
    pub config: ProviderConfigInput,
    pub configured: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    #[serde(flatten)]
    pub settings: SettingsInput,
    pub providers: Vec<ProviderSettingsDto>,
    pub shortcut_error: Option<AppErrorDto>,
}
fn configuration() -> AppErrorDto {
    AppErrorDto::new(
        "configuration",
        "服务配置无效，请检查地址、模型与服务 ID。",
        false,
    )
}
fn secure_error() -> AppErrorDto {
    AppErrorDto::new(
        "secure_store",
        "无法访问系统凭据管理器，请检查权限后重试。",
        true,
    )
}

impl AppServices {
    pub fn get_settings(&self) -> Result<SettingsDto, AppErrorDto> {
        let s = self.load_settings()?;
        let providers = s
            .providers
            .iter()
            .map(|p| {
                let has_key = match p.secret_account() {
                    Some(account) => self
                        .secure_store
                        .get(CREDENTIAL_SERVICE, account)
                        .map_err(|_| secure_error())?
                        .is_some_and(|key| !key.trim().is_empty()),
                    None => false,
                };
                Ok(ProviderSettingsDto {
                    config: ProviderConfigInput {
                        id: p.id().as_str().into(),
                        display_name: p.display_name().into(),
                        endpoint: p.endpoint().map(|e| e.to_string()),
                        model: p.model().map(str::to_owned),
                        is_local: p.is_local(),
                        enabled: p.enabled(),
                    },
                    configured: has_key,
                })
            })
            .collect::<Result<Vec<_>, AppErrorDto>>()?;
        Ok(SettingsDto {
            settings: SettingsInput {
                source: s.source_language.as_str().into(),
                target: s.target_language.as_str().into(),
                provider_id: s.active_provider.as_str().into(),
                privacy_mode: s.privacy_mode,
                fallback_enabled: s.fallback_enabled,
                history_enabled: s.history_enabled,
                shortcut: s.shortcut,
            },
            providers,
            shortcut_error: None,
        })
    }
    pub fn save_settings(&self, input: SettingsInput) -> Result<(), AppErrorDto> {
        if !list_languages().iter().any(|l| l.code == input.source)
            || !list_languages().iter().any(|l| l.code == input.target)
            || input.source == input.target
        {
            return Err(AppErrorDto::new(
                "invalid_language",
                "请选择不同且受支持的源语言和目标语言。",
                false,
            ));
        }
        self.update_settings(|s| {
            let visible = |p: &&ProviderConfig| {
                p.enabled() && (input.privacy_mode != PrivacyMode::LocalOnly || p.is_local())
            };
            let selected = s
                .providers
                .iter()
                .filter(visible)
                .find(|p| p.id().as_str() == input.provider_id)
                .or_else(|| s.providers.iter().filter(visible).next());
            // LocalOnly can be enabled before configuring a local model; translation will explain how to configure one.
            s.active_provider = selected
                .map(|p| p.id().clone())
                .unwrap_or_else(|| ProviderId::new(""));
            s.source_language = input.source.parse().map_err(|_| configuration())?;
            s.target_language = input.target.parse().map_err(|_| configuration())?;
            s.privacy_mode = input.privacy_mode;
            s.fallback_enabled = input.fallback_enabled;
            s.history_enabled = input.history_enabled;
            s.shortcut = input.shortcut;
            Ok(())
        })
    }
    pub fn save_provider_config(&self, input: ProviderConfigInput) -> Result<(), AppErrorDto> {
        if input.id.is_empty()
            || input.id.len() > 64
            || !input
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || input.display_name.trim().is_empty()
        {
            return Err(configuration());
        }
        let endpoint = input
            .endpoint
            .as_deref()
            .map(Url::parse)
            .transpose()
            .map_err(|_| configuration())?;
        if input.is_local
            && !endpoint.as_ref().is_some_and(|e| match e.host() {
                Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
                Some(Host::Ipv4(ip)) => ip.is_loopback(),
                Some(Host::Ipv6(ip)) => ip.is_loopback(),
                None => false,
            })
        {
            return Err(AppErrorDto::new(
                "invalid_local_endpoint",
                "本地服务必须使用 localhost、127.0.0.1 或 [::1] 回环地址。",
                false,
            ));
        }
        let config = ProviderConfig::new(
            ProviderId::new(input.id.clone()),
            input.display_name,
            endpoint,
            input.model,
            if input.id == "mymemory" {
                None
            } else {
                Some(format!("provider:{}", input.id))
            },
            input.is_local,
            input.enabled,
        )
        .map_err(|_| configuration())?;
        // Validate adapters even when disabled so malformed configurations cannot poison future registry construction.
        let enabled = ProviderConfig::new(
            config.id().clone(),
            config.display_name().into(),
            config.endpoint().cloned(),
            config.model().map(str::to_owned),
            config.secret_account().map(str::to_owned),
            config.is_local(),
            true,
        )
        .map_err(|_| configuration())?;
        ProviderRegistry::from_configs(&[enabled], self.secure_store.clone())
            .map_err(|_| configuration())?;
        self.update_settings(|s| {
            if let Some(existing) = s.providers.iter_mut().find(|p| p.id() == config.id()) {
                *existing = config;
            } else {
                s.providers.push(config);
            }
            Ok(())
        })
    }
    /// Removes a provider, its stored credential and any selection pointing at it.
    /// The credential goes first so a failed keyring delete leaves the configuration intact.
    pub fn delete_provider_config(&self, provider_id: String) -> Result<(), AppErrorDto> {
        let account = self
            .load_settings()?
            .providers
            .iter()
            .find(|p| p.id().as_str() == provider_id)
            .and_then(|p| p.secret_account().map(str::to_owned));
        if let Some(account) = account {
            self.secure_store
                .delete(CREDENTIAL_SERVICE, &account)
                .map_err(|_| secure_error())?;
        }
        self.update_settings(|s| {
            let before = s.providers.len();
            s.providers.retain(|p| p.id().as_str() != provider_id);
            if s.providers.len() == before {
                return Err(configuration());
            }
            if s.active_provider.as_str() == provider_id {
                // Fall back to the first provider the current privacy mode allows.
                let mode = s.privacy_mode.clone();
                s.active_provider = s
                    .providers
                    .iter()
                    .find(|p| p.enabled() && (mode != PrivacyMode::LocalOnly || p.is_local()))
                    .map(|p| p.id().clone())
                    .unwrap_or_else(|| ProviderId::new(""));
            }
            Ok(())
        })
    }
    fn secret_account(&self, provider_id: &str) -> Result<String, AppErrorDto> {
        self.load_settings()?
            .providers
            .iter()
            .find(|p| p.id().as_str() == provider_id)
            .and_then(|p| p.secret_account().map(str::to_owned))
            .ok_or_else(configuration)
    }
    pub fn set_provider_secret(
        &self,
        provider_id: String,
        secret: String,
    ) -> Result<(), AppErrorDto> {
        if secret.trim().is_empty() {
            return Err(AppErrorDto::new(
                "invalid_secret",
                "请输入 API 密钥。",
                false,
            ));
        }
        let account = self.secret_account(&provider_id)?;
        self.secure_store
            .set(CREDENTIAL_SERVICE, &account, &secret)
            .map_err(|_| secure_error())
    }
    pub fn delete_provider_secret(&self, provider_id: String) -> Result<(), AppErrorDto> {
        let account = self.secret_account(&provider_id)?;
        self.secure_store
            .delete(CREDENTIAL_SERVICE, &account)
            .map_err(|_| secure_error())
    }
}
#[tauri::command]
pub fn get_settings(
    state: State<'_, AppServices>,
    shortcuts: State<'_, ShortcutController>,
) -> Result<SettingsDto, AppErrorDto> {
    let mut dto = state.get_settings()?;
    dto.shortcut_error = shortcuts.error();
    Ok(dto)
}
#[tauri::command]
pub fn save_settings(
    state: State<'_, AppServices>,
    shortcuts: State<'_, ShortcutController>,
    settings: SettingsInput,
) -> Result<(), AppErrorDto> {
    let accelerator = settings.shortcut.clone();
    shortcuts.apply(&accelerator, || state.save_settings(settings))
}
#[tauri::command]
pub fn save_provider_config(
    state: State<'_, AppServices>,
    config: ProviderConfigInput,
) -> Result<(), AppErrorDto> {
    state.save_provider_config(config)
}
#[tauri::command]
pub fn set_provider_secret(
    state: State<'_, AppServices>,
    provider_id: String,
    secret: String,
) -> Result<(), AppErrorDto> {
    state.set_provider_secret(provider_id, secret)
}
#[tauri::command]
pub fn delete_provider_secret(
    state: State<'_, AppServices>,
    provider_id: String,
) -> Result<(), AppErrorDto> {
    state.delete_provider_secret(provider_id)
}
#[tauri::command]
pub fn delete_provider_config(
    state: State<'_, AppServices>,
    provider_id: String,
) -> Result<(), AppErrorDto> {
    state.delete_provider_config(provider_id)
}
