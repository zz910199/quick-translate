use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

use super::ValidationError;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct LanguageCode(String);

impl LanguageCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for LanguageCode {
    type Err = ValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut segments = value.split('-');
        let primary = segments.next().unwrap_or_default();
        let valid_primary = (2..=3).contains(&primary.len())
            && primary.bytes().all(|byte| byte.is_ascii_alphabetic());
        let valid_subtags = segments.all(|segment| {
            (2..=8).contains(&segment.len())
                && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });

        if valid_primary && valid_subtags {
            Ok(Self(value.to_owned()))
        } else {
            Err(ValidationError::InvalidLanguageCode)
        }
    }
}

impl<'de> Deserialize<'de> for LanguageCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}
