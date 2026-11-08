use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::domain::{LanguageCode, ProviderId};

use super::{Database, StorageError};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Option<i64>,
    pub source_text: String,
    pub translated_text: String,
    pub source_language: LanguageCode,
    pub target_language: LanguageCode,
    pub provider_id: ProviderId,
    pub timestamp: DateTime<Utc>,
}

/// Orchestration must check `AppSettings.history_enabled` before inserting.
/// Disabling history preserves previously saved entries until explicitly cleared.
pub trait HistoryRepository: Send + Sync {
    fn insert(&self, entry: &HistoryEntry) -> Result<i64, StorageError>;
    fn list(&self, limit: u32) -> Result<Vec<HistoryEntry>, StorageError>;
    fn get(&self, id: i64) -> Result<Option<HistoryEntry>, StorageError>;
    /// Rewrites an existing row in place, keeping its id and original timestamp.
    fn update(&self, entry: &HistoryEntry) -> Result<(), StorageError>;
    fn delete(&self, id: i64) -> Result<(), StorageError>;
    fn clear(&self) -> Result<(), StorageError>;
}

pub struct SqliteHistoryRepository {
    database: Database,
}

impl SqliteHistoryRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Ok(Self {
            database: Database::open(path)?,
        })
    }
}

impl HistoryRepository for SqliteHistoryRepository {
    fn insert(&self, entry: &HistoryEntry) -> Result<i64, StorageError> {
        let connection = self.database.connection()?;
        connection.execute(
            "INSERT INTO history (source_text, translated_text, source_language, target_language, provider_id, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![entry.source_text, entry.translated_text, entry.source_language.as_str(), entry.target_language.as_str(), entry.provider_id.as_str(), entry.timestamp.to_rfc3339_opts(SecondsFormat::Nanos, true)],
        )?;
        Ok(connection.last_insert_rowid())
    }

    fn list(&self, limit: u32) -> Result<Vec<HistoryEntry>, StorageError> {
        let connection = self.database.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, source_text, translated_text, source_language, target_language, provider_id, timestamp FROM history ORDER BY timestamp DESC, id DESC LIMIT ?1",
        )?;
        let mut rows = statement.query([limit])?;
        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            entries.push(read_entry(row)?);
        }
        Ok(entries)
    }

    fn get(&self, id: i64) -> Result<Option<HistoryEntry>, StorageError> {
        let connection = self.database.connection()?;
        let raw = connection
            .query_row(
                "SELECT id, source_text, translated_text, source_language, target_language, provider_id, timestamp FROM history WHERE id = ?1",
                [id],
                |row| Ok(read_entry(row)),
            )
            .optional()?;
        raw.transpose()
    }

    fn update(&self, entry: &HistoryEntry) -> Result<(), StorageError> {
        let id = entry.id.ok_or(StorageError::InvalidData)?;
        self.database.connection()?.execute(
            "UPDATE history SET source_text = ?2, translated_text = ?3, source_language = ?4, target_language = ?5, provider_id = ?6 WHERE id = ?1",
            rusqlite::params![id, entry.source_text, entry.translated_text, entry.source_language.as_str(), entry.target_language.as_str(), entry.provider_id.as_str()],
        )?;
        Ok(())
    }

    fn delete(&self, id: i64) -> Result<(), StorageError> {
        self.database
            .connection()?
            .execute("DELETE FROM history WHERE id = ?1", [id])?;
        Ok(())
    }

    fn clear(&self) -> Result<(), StorageError> {
        self.database
            .connection()?
            .execute("DELETE FROM history", [])?;
        Ok(())
    }
}

fn read_entry(row: &rusqlite::Row<'_>) -> Result<HistoryEntry, StorageError> {
    Ok(HistoryEntry {
        id: Some(row.get(0)?),
        source_text: row.get(1)?,
        translated_text: row.get(2)?,
        source_language: row
            .get::<_, String>(3)?
            .parse()
            .map_err(|_| StorageError::InvalidData)?,
        target_language: row
            .get::<_, String>(4)?
            .parse()
            .map_err(|_| StorageError::InvalidData)?,
        provider_id: ProviderId::new(row.get::<_, String>(5)?),
        timestamp: row
            .get::<_, String>(6)?
            .parse()
            .map_err(|_| StorageError::InvalidData)?,
    })
}
