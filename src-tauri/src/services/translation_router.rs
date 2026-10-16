use thiserror::Error;

use crate::{
    domain::{PrivacyMode, ProviderError, ProviderId, TranslationRequest, TranslationResult},
    providers::ProviderRegistry,
};

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum TranslationError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("no enabled local translation provider is configured")]
    NoLocalProvider,
}

pub struct TranslationRouter {
    registry: ProviderRegistry,
}

impl TranslationRouter {
    pub fn new(registry: ProviderRegistry) -> Self {
        Self { registry }
    }

    pub async fn translate(
        &self,
        mode: PrivacyMode,
        selected_provider: &ProviderId,
        request: &TranslationRequest,
        fallback_enabled: bool,
    ) -> Result<TranslationResult, TranslationError> {
        match mode {
            PrivacyMode::LocalOnly => {
                let provider = if self
                    .registry
                    .metadata(selected_provider)
                    .is_some_and(|metadata| metadata.is_local)
                {
                    selected_provider.clone()
                } else {
                    self.registry
                        .local_provider(None)
                        .ok_or(TranslationError::NoLocalProvider)?
                };
                // Only one local dispatch. A local failure can never trigger a remote request.
                Ok(self.registry.translate(&provider, request).await?)
            }
            PrivacyMode::OnlinePreferred => {
                match self.registry.translate(selected_provider, request).await {
                    Ok(result) => Ok(result),
                    Err(error) if fallback_enabled => {
                        match self.registry.local_provider(Some(selected_provider)) {
                            Some(local) => Ok(self.registry.translate(&local, request).await?),
                            None => Err(error.into()),
                        }
                    }
                    Err(error) => Err(error.into()),
                }
            }
        }
    }
}
