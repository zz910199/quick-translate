pub mod deepl;
pub mod mymemory;
pub mod openai_compatible;

use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use url::{Host, Url};

use crate::{
    domain::{
        ProviderConfig, ProviderError, ProviderId, TranslationProvider, TranslationRequest,
        TranslationResult,
    },
    platform::secure_store::SecureStore,
};

pub const CREDENTIAL_SERVICE: &str = "quick-translate";

/// MyMemory rejects anything longer than this many UTF-8 bytes per request.
pub const MYMEMORY_LIMIT_BYTES: usize = 500;

#[derive(Clone, Copy)]
pub struct HttpTimeouts {
    pub connect: Duration,
    pub total: Duration,
}

impl Default for HttpTimeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(5),
            total: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProviderMetadata {
    pub id: ProviderId,
    pub display_name: String,
    pub is_local: bool,
    pub attribution: Option<String>,
    pub limits_notice: Option<String>,
    /// Per-request input limit in UTF-8 bytes, when the service has a hard one.
    pub limit_bytes: Option<usize>,
}

struct Entry {
    adapter: Arc<dyn TranslationProvider>,
    metadata: ProviderMetadata,
}

#[derive(Default)]
pub struct ProviderRegistry {
    entries: BTreeMap<String, Entry>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_configs(
        configs: &[ProviderConfig],
        secure_store: Arc<dyn SecureStore>,
    ) -> Result<Self, ProviderError> {
        Self::from_configs_with_timeouts(configs, secure_store, HttpTimeouts::default())
    }

    /// Exact built-in IDs choose their adapters. Custom IDs require a model and
    /// use the OpenAI-compatible protocol; configuration has no separate kind field.
    pub fn from_configs_with_timeouts(
        configs: &[ProviderConfig],
        secure_store: Arc<dyn SecureStore>,
        timeouts: HttpTimeouts,
    ) -> Result<Self, ProviderError> {
        let mut registry = Self::new();
        for config in configs.iter().filter(|config| config.enabled()) {
            let adapter: Arc<dyn TranslationProvider> = match config.id().as_str() {
                "mymemory" => Arc::new(mymemory::MyMemory::new(config.clone(), timeouts)?),
                "deepl" => Arc::new(deepl::DeepL::new(
                    config.clone(),
                    secure_store.clone(),
                    timeouts,
                )?),
                "openai-compatible" => Arc::new(openai_compatible::OpenAiCompatible::new(
                    config.clone(),
                    secure_store.clone(),
                    timeouts,
                )?),
                _ if config.model().is_some() => {
                    Arc::new(openai_compatible::OpenAiCompatible::new(
                        config.clone(),
                        secure_store.clone(),
                        timeouts,
                    )?)
                }
                _ => return Err(ProviderError::Configuration),
            };
            registry.register(config.clone(), adapter)?;
        }
        Ok(registry)
    }

    pub fn register(
        &mut self,
        config: ProviderConfig,
        adapter: Arc<dyn TranslationProvider>,
    ) -> Result<(), ProviderError> {
        if !config.enabled() {
            return Ok(());
        }
        validate_local(&config)?;
        if self.entries.contains_key(config.id().as_str()) {
            return Err(ProviderError::Configuration);
        }
        let (attribution, limits_notice, limit_bytes) = if adapter.id() == "mymemory" {
            (Some("Translation provided by MyMemory (Translated). https://mymemory.translated.net/".into()),
             Some(format!("Online service: text is sent to MyMemory. Maximum {MYMEMORY_LIMIT_BYTES} UTF-8 bytes per request; anonymous use is limited to 5,000 characters per day. Quotas and availability are set by the service and may change.")),
             Some(MYMEMORY_LIMIT_BYTES))
        } else {
            (None, None, None)
        };
        self.entries.insert(
            config.id().as_str().into(),
            Entry {
                adapter,
                metadata: ProviderMetadata {
                    id: config.id().clone(),
                    display_name: config.display_name().into(),
                    is_local: config.is_local(),
                    attribution,
                    limits_notice,
                    limit_bytes,
                },
            },
        );
        Ok(())
    }

    pub fn metadata(&self, id: &ProviderId) -> Option<&ProviderMetadata> {
        self.entries.get(id.as_str()).map(|entry| &entry.metadata)
    }

    /// Stable ordering by configured ID, independent of insertion order.
    pub fn local_provider(&self, excluding: Option<&ProviderId>) -> Option<ProviderId> {
        self.entries
            .values()
            .find(|entry| entry.metadata.is_local && Some(&entry.metadata.id) != excluding)
            .map(|entry| entry.metadata.id.clone())
    }

    pub async fn translate(
        &self,
        provider_id: &ProviderId,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, ProviderError> {
        let entry = self
            .entries
            .get(provider_id.as_str())
            .ok_or(ProviderError::Configuration)?;
        let mut result = entry.adapter.translate(request).await?;
        result.provider_id = provider_id.as_str().into();
        Ok(result)
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

fn validate_local(config: &ProviderConfig) -> Result<(), ProviderError> {
    if config.is_local() && !config.endpoint().is_some_and(is_loopback) {
        return Err(ProviderError::Configuration);
    }
    Ok(())
}

struct HttpTransport {
    config: ProviderConfig,
    client: Client,
    endpoint: Url,
}

impl HttpTransport {
    fn new(
        config: ProviderConfig,
        default_endpoint: Option<&str>,
        timeouts: HttpTimeouts,
    ) -> Result<Self, ProviderError> {
        validate_local(&config)?;
        if timeouts.connect.is_zero() || timeouts.total.is_zero() {
            return Err(ProviderError::Configuration);
        }
        let endpoint = config
            .endpoint()
            .cloned()
            .or_else(|| default_endpoint.and_then(|value| Url::parse(value).ok()))
            .ok_or(ProviderError::Configuration)?;
        let mut builder = Client::builder()
            .connect_timeout(timeouts.connect)
            .timeout(timeouts.total)
            .redirect(reqwest::redirect::Policy::none());
        if config.is_local() {
            builder = builder.no_proxy();
            // Pin localhost to the loopback interface rather than trusting DNS.
            if endpoint.host_str() == Some("localhost") {
                builder = builder.resolve(
                    "localhost",
                    SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::LOCALHOST),
                        endpoint.port_or_known_default().unwrap_or(80),
                    ),
                );
            }
        }
        let client = builder.build().map_err(|_| ProviderError::Configuration)?;
        Ok(Self {
            config,
            client,
            endpoint,
        })
    }

    fn secret(
        &self,
        store: &dyn SecureStore,
        required: bool,
    ) -> Result<Option<String>, ProviderError> {
        match self.config.secret_account() {
            Some(account) => {
                let key = store
                    .get(CREDENTIAL_SERVICE, account)
                    .map_err(|_| ProviderError::Authentication)?
                    .filter(|key| !key.trim().is_empty());
                if key.is_none() && required {
                    Err(ProviderError::Authentication)
                } else {
                    Ok(key)
                }
            }
            None if required => Err(ProviderError::Authentication),
            None => Ok(None),
        }
    }

    async fn send<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, ProviderError> {
        let response = request.send().await.map_err(transport_error)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(status_error(
                status,
                response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse().ok()),
            ));
        }
        response.json().await.map_err(|error| {
            if error.is_timeout() {
                ProviderError::Timeout
            } else if error.is_decode() {
                ProviderError::InvalidResponse
            } else {
                transport_error(error)
            }
        })
    }

    fn result(
        &self,
        text: String,
        request: &TranslationRequest,
        start: Instant,
    ) -> Result<TranslationResult, ProviderError> {
        if text.trim().is_empty() {
            return Err(ProviderError::InvalidResponse);
        }
        Ok(TranslationResult {
            translated_text: text,
            provider_id: self.config.id().as_str().into(),
            elapsed_ms: start.elapsed().as_millis().min(u64::MAX as u128) as u64,
            character_count: request.text().chars().count(),
        })
    }
}

fn transport_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout
    } else if error.is_builder() {
        ProviderError::Configuration
    } else {
        ProviderError::Network
    }
}

fn status_error(status: u16, retry_after_seconds: Option<u64>) -> ProviderError {
    match status {
        401 | 403 => ProviderError::Authentication,
        429 => ProviderError::RateLimited {
            retry_after_seconds,
        },
        _ => ProviderError::Service {
            status: Some(status),
        },
    }
}
