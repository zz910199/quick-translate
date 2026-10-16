use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use super::{ProviderError, TranslationRequest, TranslationResult};

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: ProviderId,
    pub display_name: String,
    pub endpoint: Option<Url>,
    pub model: Option<String>,
    pub secret_account: Option<String>,
    pub is_local: bool,
    pub enabled: bool,
}

#[async_trait]
pub trait TranslationProvider: Send + Sync {
    fn id(&self) -> &'static str;

    async fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, ProviderError>;
}
