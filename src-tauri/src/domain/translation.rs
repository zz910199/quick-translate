use serde::{Deserialize, Deserializer, Serialize};

use super::{LanguageCode, ValidationError};

/// A validated request with immutable read access.
///
/// ```compile_fail
/// use quick_translate_lib::domain::TranslationRequest;
/// let request = TranslationRequest {
///     text: "   ".into(),
///     source: "en".parse().unwrap(),
///     target: "en".parse().unwrap(),
/// };
/// ```
///
/// ```compile_fail
/// use quick_translate_lib::domain::TranslationRequest;
/// let mut request = TranslationRequest::new(
///     "hello".into(), "en".parse().unwrap(), "ja".parse().unwrap()
/// ).unwrap();
/// request.text = "   ".into();
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TranslationRequest {
    text: String,
    source: LanguageCode,
    target: LanguageCode,
}

impl TranslationRequest {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn source(&self) -> &LanguageCode {
        &self.source
    }

    pub fn target(&self) -> &LanguageCode {
        &self.target
    }

    pub fn new(
        text: String,
        source: LanguageCode,
        target: LanguageCode,
    ) -> Result<Self, ValidationError> {
        if text.trim().is_empty() {
            return Err(ValidationError::EmptyText);
        }
        if source == target {
            return Err(ValidationError::IdenticalLanguages);
        }

        Ok(Self {
            text,
            source,
            target,
        })
    }
}

impl<'de> Deserialize<'de> for TranslationRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Fields {
            text: String,
            source: LanguageCode,
            target: LanguageCode,
        }

        let fields = Fields::deserialize(deserializer)?;
        Self::new(fields.text, fields.source, fields.target).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TranslationResult {
    pub translated_text: String,
    pub provider_id: String,
    pub elapsed_ms: u64,
    pub character_count: usize,
}
