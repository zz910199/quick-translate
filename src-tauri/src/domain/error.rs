use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ValidationError {
    #[error("invalid language code")]
    InvalidLanguageCode,
    #[error("translation text cannot be empty")]
    EmptyText,
    #[error("source and target languages must differ")]
    IdenticalLanguages,
    #[error("invalid provider endpoint")]
    InvalidProviderEndpoint,
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ProviderError {
    #[error("provider authentication failed")]
    Authentication,
    #[error("provider rate limit exceeded")]
    RateLimited { retry_after_seconds: Option<u64> },
    #[error("provider network failure")]
    Network,
    #[error("provider request timed out")]
    Timeout,
    #[error("provider service error")]
    Service { status: Option<u16> },
    #[error("provider returned an invalid response")]
    InvalidResponse,
    #[error("provider does not support this language")]
    UnsupportedLanguage,
    #[error("provider configuration is invalid")]
    Configuration,
}
