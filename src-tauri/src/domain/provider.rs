use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize};
use url::{Host, Url};

use super::{ProviderError, TranslationRequest, TranslationResult, ValidationError};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct ProviderConfig {
    id: ProviderId,
    display_name: String,
    endpoint: Option<Url>,
    model: Option<String>,
    secret_account: Option<String>,
    is_local: bool,
    enabled: bool,
}

impl ProviderConfig {
    pub fn id(&self) -> &ProviderId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn endpoint(&self) -> Option<&Url> {
        self.endpoint.as_ref()
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn secret_account(&self) -> Option<&str> {
        self.secret_account.as_deref()
    }

    pub fn is_local(&self) -> bool {
        self.is_local
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn new(
        id: ProviderId,
        display_name: String,
        endpoint: Option<Url>,
        model: Option<String>,
        secret_account: Option<String>,
        is_local: bool,
        enabled: bool,
    ) -> Result<Self, ValidationError> {
        if let Some(url) = &endpoint {
            let allowed_scheme = match url.scheme() {
                "https" => true,
                "http" => match url.host() {
                    Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
                    Some(Host::Ipv4(ip)) => ip.is_loopback(),
                    Some(Host::Ipv6(ip)) => ip.is_loopback(),
                    None => false,
                },
                _ => false,
            };
            if !allowed_scheme
                || url.host().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(ValidationError::InvalidProviderEndpoint);
            }
        }

        Ok(Self {
            id,
            display_name,
            endpoint,
            model,
            secret_account,
            is_local,
            enabled,
        })
    }
}

impl<'de> Deserialize<'de> for ProviderConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Fields {
            id: ProviderId,
            display_name: String,
            endpoint: Option<String>,
            model: Option<String>,
            secret_account: Option<String>,
            is_local: bool,
            enabled: bool,
        }

        let fields = Fields::deserialize(deserializer)?;
        let endpoint = fields
            .endpoint
            .map(|value| Url::parse(&value))
            .transpose()
            .map_err(|_| serde::de::Error::custom(ValidationError::InvalidProviderEndpoint))?;
        Self::new(
            fields.id,
            fields.display_name,
            endpoint,
            fields.model,
            fields.secret_account,
            fields.is_local,
            fields.enabled,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderConfig")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("endpoint", &self.endpoint.as_ref().map(|_| "[redacted]"))
            .field("model", &self.model)
            .field("secret_account", &self.secret_account)
            .field("is_local", &self.is_local)
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[async_trait]
pub trait TranslationProvider: Send + Sync {
    fn id(&self) -> &'static str;

    async fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, ProviderError>;
}
