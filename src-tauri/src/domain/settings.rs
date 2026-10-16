use serde::{Deserialize, Serialize};

use super::{LanguageCode, ProviderConfig, ProviderId};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PrivacyMode {
    OnlinePreferred,
    LocalOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub source_language: LanguageCode,
    pub target_language: LanguageCode,
    pub active_provider: ProviderId,
    pub privacy_mode: PrivacyMode,
    pub fallback_enabled: bool,
    pub history_enabled: bool,
    pub shortcut: String,
    pub providers: Vec<ProviderConfig>,
}
