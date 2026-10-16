use crate::{
    commands::translation::AppServices,
    platform::{clipboard::SystemClipboard, secure_store::KeyringSecureStore},
    storage::{SqliteHistoryRepository, SqliteSettingsRepository},
};
use std::{path::Path, sync::Arc};

pub const DATA_DIRECTORY: &str = "D:/QuickTranslate/Data";
/// No AppData fallback: all app-owned persistent data stays on the configured D drive.
pub fn persistent_services() -> Result<AppServices, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(DATA_DIRECTORY)?;
    let database = Path::new(DATA_DIRECTORY).join("quick-translate.sqlite3");
    Ok(AppServices::new(
        Arc::new(SqliteSettingsRepository::open(&database)?),
        Arc::new(SqliteHistoryRepository::open(&database)?),
        Arc::new(KeyringSecureStore::default()),
        Arc::new(SystemClipboard),
    ))
}
