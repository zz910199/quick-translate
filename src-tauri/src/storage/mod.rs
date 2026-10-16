mod history;
mod settings;

use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use rusqlite::Connection;
use thiserror::Error;

pub use history::{HistoryEntry, HistoryRepository, SqliteHistoryRepository};
pub use settings::{SettingsRepository, SqliteSettingsRepository};

/// Deliberately excludes database messages, which can contain stored user text.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum StorageError {
    #[error("local database operation failed")]
    Database,
    #[error("stored data is invalid")]
    InvalidData,
    #[error("local database lock is unavailable")]
    LockUnavailable,
}

impl From<rusqlite::Error> for StorageError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database
    }
}

struct Database(Mutex<Connection>);

impl Database {
    /// The caller chooses the location and creates its containing directory.
    fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(include_str!("../../migrations/001_initial.sql"))?;
        Ok(Self(Mutex::new(connection)))
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.0.lock().map_err(|_| StorageError::LockUnavailable)
    }
}
