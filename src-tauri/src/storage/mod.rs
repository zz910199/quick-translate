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

/// Two repositories over the same file may be opened concurrently. On Windows, a second
/// connection that races the first one's file creation gets `SQLITE_READONLY`, so opening and
/// migrating is serialized inside the process.
static OPEN_LOCK: Mutex<()> = Mutex::new(());

impl Database {
    /// The caller chooses the location and creates its containing directory.
    fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let _guard = OPEN_LOCK
            .lock()
            .map_err(|_| StorageError::LockUnavailable)?;
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        // A second process may still be applying the migration; retry instead of failing startup.
        let migration = include_str!("../../migrations/001_initial.sql");
        let mut attempt = 0u32;
        loop {
            match connection.execute_batch(migration) {
                Ok(()) => break,
                Err(error) if is_retryable(&error) && attempt < 8 => {
                    let _ = connection.execute_batch("ROLLBACK;");
                    attempt += 1;
                    std::thread::sleep(Duration::from_millis(20 * u64::from(attempt)));
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(Self(Mutex::new(connection)))
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.0.lock().map_err(|_| StorageError::LockUnavailable)
    }
}

fn is_retryable(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if matches!(
                code.code,
                rusqlite::ErrorCode::DatabaseBusy
                    | rusqlite::ErrorCode::DatabaseLocked
                    | rusqlite::ErrorCode::ReadOnly
            )
    )
}
