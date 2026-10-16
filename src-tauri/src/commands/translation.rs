use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use tauri::State;

use crate::{
    domain::{
        AppSettings, PrivacyMode, ProviderConfig, ProviderError, ProviderId, TranslationRequest,
        ValidationError,
    },
    platform::{clipboard::Clipboard, secure_store::SecureStore},
    providers::{ProviderRegistry, CREDENTIAL_SERVICE},
    services::translation_router::{TranslationError, TranslationRouter},
    storage::{HistoryEntry, HistoryRepository, SettingsRepository},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}
impl AppErrorDto {
    pub fn new(code: &str, message: &str, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
    fn storage() -> Self {
        Self::new("storage", "无法读写本地数据，请重试。", true)
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResultDto {
    pub translated_text: String,
    pub provider_id: String,
    pub elapsed_ms: u64,
    pub character_count: usize,
}
#[derive(Debug, Serialize)]
pub struct LanguageOptionDto {
    pub code: String,
    pub label: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOptionDto {
    pub id: String,
    pub display_name: String,
    pub is_local: bool,
    pub configured: bool,
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSelectionDto {
    pub source: String,
    pub target: String,
    pub provider_id: String,
    pub privacy_mode: PrivacyMode,
}

pub type RegistryFactory = dyn Fn(&[ProviderConfig], Arc<dyn SecureStore>) -> Result<ProviderRegistry, ProviderError>
    + Send
    + Sync;

/// Shared runtime services. Repositories own short synchronous locks; no guard crosses await.
pub struct AppServices {
    pub settings: Arc<dyn SettingsRepository>,
    pub history: Arc<dyn HistoryRepository>,
    pub secure_store: Arc<dyn SecureStore>,
    pub clipboard: Arc<dyn Clipboard>,
    settings_write: Mutex<()>,
    registry_factory: Arc<RegistryFactory>,
}
impl AppServices {
    pub fn new(
        settings: Arc<dyn SettingsRepository>,
        history: Arc<dyn HistoryRepository>,
        secure_store: Arc<dyn SecureStore>,
        clipboard: Arc<dyn Clipboard>,
    ) -> Self {
        Self {
            settings,
            history,
            secure_store,
            clipboard,
            settings_write: Mutex::new(()),
            registry_factory: Arc::new(ProviderRegistry::from_configs),
        }
    }
    pub fn with_registry_factory(mut self, factory: Arc<RegistryFactory>) -> Self {
        self.registry_factory = factory;
        self
    }
    pub fn load_settings(&self) -> Result<AppSettings, AppErrorDto> {
        self.settings.load().map_err(|_| AppErrorDto::storage())
    }
    /// Keep future settings edits and selection updates within this synchronous transaction.
    pub fn update_settings(
        &self,
        edit: impl FnOnce(&mut AppSettings) -> Result<(), AppErrorDto>,
    ) -> Result<(), AppErrorDto> {
        let _guard = self
            .settings_write
            .lock()
            .map_err(|_| AppErrorDto::storage())?;
        let mut settings = self.load_settings()?;
        edit(&mut settings)?;
        self.settings
            .save(&settings)
            .map_err(|_| AppErrorDto::storage())
    }
    pub fn get_translation_selection(&self) -> Result<TranslationSelectionDto, AppErrorDto> {
        let s = self.load_settings()?;
        Ok(TranslationSelectionDto {
            source: s.source_language.as_str().into(),
            target: s.target_language.as_str().into(),
            provider_id: s.active_provider.as_str().into(),
            privacy_mode: s.privacy_mode,
        })
    }
    pub fn save_translation_selection(
        &self,
        source: String,
        target: String,
        provider_id: String,
    ) -> Result<(), AppErrorDto> {
        let source = parse_language(&source)?;
        let target = parse_language(&target)?;
        self.update_settings(|s| {
            if !s.providers.iter().any(|p| {
                p.enabled()
                    && p.id().as_str() == provider_id
                    && (s.privacy_mode != PrivacyMode::LocalOnly || p.is_local())
            }) {
                return Err(provider_error(ProviderError::Configuration));
            }
            s.source_language = source;
            s.target_language = target;
            s.active_provider = ProviderId::new(provider_id);
            Ok(())
        })
    }
    pub fn list_providers(&self) -> Result<Vec<ProviderOptionDto>, AppErrorDto> {
        let s = self.load_settings()?;
        let registry = (self.registry_factory)(&s.providers, self.secure_store.clone())
            .map_err(provider_error)?;
        Ok(s.providers
            .iter()
            .filter(|p| p.enabled() && (s.privacy_mode != PrivacyMode::LocalOnly || p.is_local()))
            .filter_map(|p| {
                let metadata = registry.metadata(p.id())?;
                let configured = match p.secret_account() {
                    Some(account) => self
                        .secure_store
                        .get(CREDENTIAL_SERVICE, account)
                        .ok()
                        .flatten()
                        .is_some_and(|key| !key.trim().is_empty()),
                    None => {
                        p.id().as_str() != "deepl"
                            && (p.is_local() || p.id().as_str() == "mymemory")
                    }
                };
                let mut notices = Vec::new();
                if !p.is_local() {
                    notices.push("在线服务：翻译时原文将发送至所选服务提供方。".to_string());
                }
                if let Some(value) = &metadata.attribution {
                    notices.push(value.clone());
                }
                if let Some(value) = &metadata.limits_notice {
                    notices.push(value.clone());
                }
                Some(ProviderOptionDto {
                    id: p.id().as_str().into(),
                    display_name: p.display_name().into(),
                    is_local: p.is_local(),
                    configured,
                    is_default: p.id() == &s.active_provider,
                    notice: if notices.is_empty() {
                        None
                    } else {
                        Some(notices.join(" "))
                    },
                })
            })
            .collect())
    }
    pub async fn translate_text(
        &self,
        text: String,
        source: String,
        target: String,
        provider_id: String,
    ) -> Result<TranslationResultDto, AppErrorDto> {
        let request =
            TranslationRequest::new(text, parse_language(&source)?, parse_language(&target)?)
                .map_err(validation_error)?;
        let settings = self.load_settings()?;
        let registry = (self.registry_factory)(&settings.providers, self.secure_store.clone())
            .map_err(provider_error)?;
        let selected = ProviderId::new(provider_id);
        // Validate the effective provider: LocalOnly may replace an online selection.
        let effective = if settings.privacy_mode == PrivacyMode::LocalOnly
            && !registry.metadata(&selected).is_some_and(|p| p.is_local)
        {
            registry
                .local_provider(None)
                .ok_or_else(no_local_provider)?
        } else {
            selected.clone()
        };
        if effective.as_str() == "mymemory" && request.text().len() > 500 {
            return Err(AppErrorDto::new(
                "input_too_long",
                "MyMemory 每次最多支持 500 UTF-8 字节，请缩短原文或更换服务。",
                false,
            ));
        }
        let result = TranslationRouter::new(registry)
            .translate(
                settings.privacy_mode,
                &selected,
                &request,
                settings.fallback_enabled,
            )
            .await
            .map_err(|e| match e {
                TranslationError::Provider(p) => provider_error(p),
                TranslationError::NoLocalProvider => no_local_provider(),
            })?;
        // Read again after await so disabling history during a pending request takes effect.
        if settings.history_enabled && self.load_settings()?.history_enabled {
            self.history
                .insert(&HistoryEntry {
                    id: None,
                    source_text: request.text().into(),
                    translated_text: result.translated_text.clone(),
                    source_language: request.source().clone(),
                    target_language: request.target().clone(),
                    provider_id: ProviderId::new(result.provider_id.clone()),
                    timestamp: Utc::now(),
                })
                .map_err(|_| AppErrorDto::storage())?;
        }
        Ok(TranslationResultDto {
            translated_text: result.translated_text,
            provider_id: result.provider_id,
            elapsed_ms: result.elapsed_ms,
            character_count: result.character_count,
        })
    }
}
fn no_local_provider() -> AppErrorDto {
    AppErrorDto::new(
        "no_local_provider",
        "仅本地模式需要先配置并启用本地翻译服务。",
        false,
    )
}
fn parse_language(value: &str) -> Result<crate::domain::LanguageCode, AppErrorDto> {
    if !list_languages().iter().any(|l| l.code == value) {
        return Err(validation_error(ValidationError::InvalidLanguageCode));
    }
    value.parse().map_err(validation_error)
}
fn validation_error(e: ValidationError) -> AppErrorDto {
    match e {
        ValidationError::EmptyText => AppErrorDto::new("empty_text", "请输入要翻译的文本。", false),
        ValidationError::IdenticalLanguages => AppErrorDto::new(
            "identical_languages",
            "请选择不同的源语言和目标语言。",
            false,
        ),
        _ => AppErrorDto::new("invalid_language", "请选择支持的语言。", false),
    }
}
fn provider_error(e: ProviderError) -> AppErrorDto {
    let (code, message, retryable) = match e {
        ProviderError::Authentication => (
            "authentication",
            "凭据不可用，请在设置中检查 API 密钥。",
            false,
        ),
        ProviderError::RateLimited { .. } => (
            "rate_limited",
            "服务请求额度已用尽或过于频繁，请稍后重试。",
            true,
        ),
        ProviderError::Network => ("network", "无法连接翻译服务，请检查网络后重试。", true),
        ProviderError::Timeout => ("timeout", "翻译请求超时，请重试。", true),
        ProviderError::Service { .. } => ("service", "翻译服务暂不可用，请稍后重试。", true),
        ProviderError::InvalidResponse => {
            ("invalid_response", "翻译服务返回了无效结果，请重试。", true)
        }
        ProviderError::UnsupportedLanguage => {
            ("unsupported_language", "此服务不支持所选语言。", false)
        }
        ProviderError::Configuration => (
            "configuration",
            "翻译服务配置不可用，请在设置中检查。",
            false,
        ),
    };
    AppErrorDto::new(code, message, retryable)
}

#[tauri::command]
pub fn list_languages() -> Vec<LanguageOptionDto> {
    [
        ("en", "英语"),
        ("zh-CN", "简体中文"),
        ("zh-TW", "繁体中文"),
        ("ja", "日语"),
        ("ko", "韩语"),
        ("fr", "法语"),
        ("de", "德语"),
        ("es", "西班牙语"),
        ("pt", "葡萄牙语"),
        ("ru", "俄语"),
    ]
    .into_iter()
    .map(|(code, label)| LanguageOptionDto {
        code: code.into(),
        label: label.into(),
    })
    .collect()
}
#[tauri::command]
pub async fn translate_text(
    state: State<'_, AppServices>,
    text: String,
    source: String,
    target: String,
    provider_id: String,
) -> Result<TranslationResultDto, AppErrorDto> {
    state
        .translate_text(text, source, target, provider_id)
        .await
}
#[tauri::command]
pub fn list_providers(
    state: State<'_, AppServices>,
) -> Result<Vec<ProviderOptionDto>, AppErrorDto> {
    state.list_providers()
}
#[tauri::command]
pub fn get_translation_selection(
    state: State<'_, AppServices>,
) -> Result<TranslationSelectionDto, AppErrorDto> {
    state.get_translation_selection()
}
#[tauri::command]
pub fn save_translation_selection(
    state: State<'_, AppServices>,
    source: String,
    target: String,
    provider_id: String,
) -> Result<(), AppErrorDto> {
    state.save_translation_selection(source, target, provider_id)
}
