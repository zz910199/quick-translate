use std::path::Path;

use rusqlite::OptionalExtension;

use crate::domain::{AppSettings, PrivacyMode, ProviderConfig, ProviderId};

use super::{Database, StorageError};

pub trait SettingsRepository: Send + Sync {
    fn load(&self) -> Result<AppSettings, StorageError>;
    fn save(&self, settings: &AppSettings) -> Result<(), StorageError>;
    /// Re-read only this flag. A translation may finish long after the request started, and the
    /// history policy has to reflect changes made while it was in flight.
    fn history_enabled(&self) -> Result<bool, StorageError> {
        Ok(self.load()?.history_enabled)
    }
}

pub struct SqliteSettingsRepository {
    database: Database,
}

impl SqliteSettingsRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Ok(Self {
            database: Database::open(path)?,
        })
    }
}

impl SettingsRepository for SqliteSettingsRepository {
    fn load(&self) -> Result<AppSettings, StorageError> {
        let payload: Option<String> = self
            .database
            .connection()?
            .query_row("SELECT payload FROM settings WHERE id = ?1", [1], |row| {
                row.get(0)
            })
            .optional()?;
        match payload {
            Some(payload) => serde_json::from_str(&payload).map_err(|_| StorageError::InvalidData),
            None => Ok(default_settings()),
        }
    }

    fn save(&self, settings: &AppSettings) -> Result<(), StorageError> {
        let payload = serde_json::to_string(settings).map_err(|_| StorageError::InvalidData)?;
        self.database.connection()?.execute(
            "INSERT INTO settings (id, payload) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
            rusqlite::params![1, payload],
        )?;
        Ok(())
    }

    fn history_enabled(&self) -> Result<bool, StorageError> {
        // One column from the stored JSON instead of a full deserialize; absent rows mean the
        // default, which has history on.
        let enabled: Option<bool> = self
            .database
            .connection()?
            .query_row(
                "SELECT json_extract(payload, '$.history_enabled') FROM settings WHERE id = ?1",
                [1],
                |row| row.get(0),
            )
            .optional()?;
        Ok(enabled.unwrap_or(true))
    }
}

fn default_settings() -> AppSettings {
    AppSettings {
        source_language: "en".parse().expect("valid default language"),
        target_language: "zh-CN".parse().expect("valid default language"),
        active_provider: ProviderId::new("mymemory"),
        privacy_mode: PrivacyMode::OnlinePreferred,
        fallback_enabled: true,
        history_enabled: true,
        shortcut: "Ctrl+Shift+T".into(),
        providers: vec![ProviderConfig::new(
            ProviderId::new("mymemory"),
            "MyMemory".into(),
            Some(
                "https://api.mymemory.translated.net/get"
                    .parse()
                    .expect("valid default URL"),
            ),
            None,
            None,
            false,
            true,
        )
        .expect("valid default provider")],
    }
}
