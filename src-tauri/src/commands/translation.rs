use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;
use serde::Serialize;
use tauri::State;

use crate::{
    commands::history::HistoryEntryDto,
    domain::{
        AppSettings, PrivacyMode, ProviderConfig, ProviderError, ProviderId, TranslationRequest,
        TranslationResult, ValidationError,
    },
    platform::{clipboard::Clipboard, secure_store::SecureStore},
    providers::{ProviderRegistry, CREDENTIAL_SERVICE},
    services::translation_router::{TranslationError, TranslationRouter},
    storage::{HistoryEntry, HistoryRepository, SettingsRepository},
};

/// One automatic retry after this pause; transient failures are usually DNS hiccups or a local
/// server that is still starting.
const RETRY_DELAY: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}
impl AppErrorDto {
    pub fn new(code: &str, message: &str, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
            retry_after_seconds: None,
        }
    }
    fn with_retry_after(mut self, seconds: Option<u64>) -> Self {
        self.retry_after_seconds = seconds;
        self
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
    pub limit_bytes: Option<usize>,
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

struct CachedRegistry {
    fingerprint: u64,
    registry: Arc<ProviderRegistry>,
}

/// Adapters are immutable once built and read credentials per request, so a registry stays valid
/// until the serialized provider list changes.
fn provider_fingerprint(configs: &[ProviderConfig]) -> u64 {
    let mut hasher = DefaultHasher::new();
    if let Ok(bytes) = serde_json::to_vec(configs) {
        bytes.hash(&mut hasher);
    }
    hasher.finish()
}

/// A translation ready to be stored, together with the request that produced it.
struct PreparedTranslation {
    request: TranslationRequest,
    result: TranslationResult,
}

/// Shared runtime services. Repositories own short synchronous locks; no guard crosses await.
pub struct AppServices {
    pub settings: Arc<dyn SettingsRepository>,
    pub history: Arc<dyn HistoryRepository>,
    pub secure_store: Arc<dyn SecureStore>,
    pub clipboard: Arc<dyn Clipboard>,
    settings_write: Mutex<()>,
    registry_factory: Arc<RegistryFactory>,
    registry_cache: Mutex<Option<CachedRegistry>>,
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
            registry_cache: Mutex::new(None),
        }
    }
    pub fn with_registry_factory(mut self, factory: Arc<RegistryFactory>) -> Self {
        self.registry_factory = factory;
        self
    }
    fn registry(&self, configs: &[ProviderConfig]) -> Result<Arc<ProviderRegistry>, AppErrorDto> {
        let fingerprint = provider_fingerprint(configs);
        if let Ok(cache) = self.registry_cache.lock() {
            if let Some(cached) = cache.as_ref().filter(|c| c.fingerprint == fingerprint) {
                return Ok(Arc::clone(&cached.registry));
            }
        }
        let registry = Arc::new(
            (self.registry_factory)(configs, self.secure_store.clone()).map_err(provider_error)?,
        );
        if let Ok(mut cache) = self.registry_cache.lock() {
            *cache = Some(CachedRegistry {
                fingerprint,
                registry: Arc::clone(&registry),
            });
        }
        Ok(registry)
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
        let registry = self.registry(&s.providers)?;
        Ok(s.providers
            .iter()
            .filter(|p| p.enabled() && (s.privacy_mode != PrivacyMode::LocalOnly || p.is_local()))
            .filter_map(|p| {
                let metadata = registry.metadata(p.id())?;
                let configured = match p.secret_account() {
                    Some(account) => {
                        self.secure_store
                            .get(CREDENTIAL_SERVICE, account)
                            .ok()
                            .flatten()
                            .is_some_and(|key| !key.trim().is_empty())
                            || p.is_local()
                    }
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
                    limit_bytes: metadata.limit_bytes,
                    notice: if notices.is_empty() {
                        None
                    } else {
                        Some(notices.join(" "))
                    },
                })
            })
            .collect())
    }
    /// Runs the privacy policy, provider dispatch, one transient retry and the input limit.
    /// `provider_id` may name a provider that no longer exists (an old history entry); that falls
    /// back to the current default instead of reporting a misleading configuration error.
    async fn translate_with(
        &self,
        text: String,
        source: &str,
        target: &str,
        provider_id: Option<&str>,
    ) -> Result<PreparedTranslation, AppErrorDto> {
        let request =
            TranslationRequest::new(text, parse_language(source)?, parse_language(target)?)
                .map_err(validation_error)?;
        let settings = self.load_settings()?;
        let registry = self.registry(&settings.providers)?;
        let selected = provider_id
            .map(ProviderId::new)
            .filter(|id| registry.metadata(id).is_some())
            .unwrap_or_else(|| settings.active_provider.clone());
        if registry.metadata(&selected).is_none() {
            return Err(provider_error(ProviderError::Configuration));
        }
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
        if let Some(limit) = registry
            .metadata(&effective)
            .and_then(|meta| meta.limit_bytes)
        {
            if request.text().len() > limit {
                return Err(AppErrorDto::new(
                    "input_too_long",
                    &format!(
                        "该服务每次最多支持 {limit} 字节（当前 {} 字节），请缩短原文或更换服务。",
                        request.text().len()
                    ),
                    false,
                ));
            }
        }
        let router = TranslationRouter::new(&registry);
        let mut retried = false;
        let result = loop {
            let attempt = router
                .translate(
                    settings.privacy_mode.clone(),
                    &selected,
                    &request,
                    settings.fallback_enabled,
                )
                .await;
            match attempt {
                // Translation is a read-only operation, so one retry cannot duplicate work on the
                // service side. Authentication and rate limits are terminal and never retried.
                Err(TranslationError::Provider(error)) if !retried && error.is_transient() => {
                    retried = true;
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                settled => break settled,
            }
        };
        let result = result.map_err(|e| match e {
            TranslationError::Provider(p) => provider_error(p),
            TranslationError::NoLocalProvider => no_local_provider(),
        })?;
        Ok(PreparedTranslation { request, result })
    }
    pub async fn translate_text(
        &self,
        text: String,
        source: String,
        target: String,
        provider_id: String,
    ) -> Result<TranslationResultDto, AppErrorDto> {
        let PreparedTranslation { request, result } = self
            .translate_with(text, &source, &target, Some(&provider_id))
            .await?;
        // Read the policy after the round trip so disabling history mid-request takes effect. If
        // the flag cannot be read, prefer not writing: history stays opt-out, never opt-in.
        if self.settings.history_enabled().unwrap_or(false) {
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
    /// Re-runs an existing entry and rewrites it in place, so repeated retranslation does not
    /// duplicate history. The original timestamp and id are preserved.
    pub async fn retranslate_history_entry(
        &self,
        id: i64,
        source_text: Option<String>,
    ) -> Result<HistoryEntryDto, AppErrorDto> {
        let entry = self
            .history
            .get(id)
            .map_err(|_| AppErrorDto::storage())?
            .ok_or_else(|| {
                AppErrorDto::new(
                    "history_missing",
                    "该历史记录已不存在，可能已被删除。",
                    false,
                )
            })?;
        let text = source_text
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| entry.source_text.clone());
        let PreparedTranslation { request, result } = self
            .translate_with(
                text,
                entry.source_language.as_str(),
                entry.target_language.as_str(),
                Some(entry.provider_id.as_str()),
            )
            .await?;
        let updated = HistoryEntry {
            id: Some(id),
            source_text: request.text().into(),
            translated_text: result.translated_text,
            source_language: entry.source_language,
            target_language: entry.target_language,
            provider_id: ProviderId::new(result.provider_id),
            timestamp: entry.timestamp,
        };
        self.history
            .update(&updated)
            .map_err(|_| AppErrorDto::storage())?;
        Ok(crate::commands::history::history_dto(updated))
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
    let retry_after = match &e {
        ProviderError::RateLimited {
            retry_after_seconds,
        } => *retry_after_seconds,
        _ => None,
    };
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
    AppErrorDto::new(code, message, retryable).with_retry_after(retry_after)
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
