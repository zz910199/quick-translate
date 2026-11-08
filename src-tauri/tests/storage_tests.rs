use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use quick_translate_lib::{
    domain::{PrivacyMode, ProviderConfig, ProviderId},
    platform::secure_store::{
        CredentialBackend, KeyringSecureStore, SecureStore, SecureStoreError,
    },
    storage::{
        HistoryEntry, HistoryRepository, SettingsRepository, SqliteHistoryRepository,
        SqliteSettingsRepository, StorageError,
    },
};

fn entry(text: &str, timestamp: &str) -> HistoryEntry {
    HistoryEntry {
        id: None,
        source_text: text.into(),
        translated_text: format!("译文 {text}"),
        source_language: "en".parse().unwrap(),
        target_language: "zh-CN".parse().unwrap(),
        provider_id: ProviderId::new("mymemory"),
        timestamp: timestamp.parse::<DateTime<Utc>>().unwrap(),
    }
}

#[test]
fn storage_tests_settings_defaults_and_round_trip_survive_reopening() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.sqlite");
    let repo = SqliteSettingsRepository::open(&path).unwrap();
    let mut settings = repo.load().unwrap();
    assert_eq!(settings.source_language.as_str(), "en");
    assert_eq!(settings.target_language.as_str(), "zh-CN");
    assert_eq!(settings.active_provider.as_str(), "mymemory");
    assert_eq!(settings.privacy_mode, PrivacyMode::OnlinePreferred);
    assert!(settings.history_enabled);
    assert!(settings.fallback_enabled);
    assert_eq!(settings.shortcut, "Ctrl+Shift+T");
    assert_eq!(settings.providers[0].id().as_str(), "mymemory");
    settings.source_language = "ja".parse().unwrap();
    settings.target_language = "de".parse().unwrap();
    settings.active_provider = ProviderId::new("local");
    settings.privacy_mode = PrivacyMode::LocalOnly;
    settings.history_enabled = false;
    settings.fallback_enabled = false;
    settings.shortcut = "Alt+T".into();
    settings.providers = vec![ProviderConfig::new(
        ProviderId::new("local"),
        "Local".into(),
        Some("http://127.0.0.1:11434/api/generate".parse().unwrap()),
        Some("model".into()),
        Some("account-reference".into()),
        true,
        true,
    )
    .unwrap()];
    repo.save(&settings).unwrap();
    drop(repo);
    assert_eq!(
        SqliteSettingsRepository::open(&path)
            .unwrap()
            .load()
            .unwrap(),
        settings
    );
}

#[test]
fn storage_tests_history_persists_all_fields_orders_by_time_and_limits() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite");
    let repo = SqliteHistoryRepository::open(&path).unwrap();
    let newer = entry(
        "quotes '; DROP TABLE history; -- 世界",
        "2026-10-17T03:00:00Z",
    );
    let newer_id = repo.insert(&newer).unwrap();
    repo.insert(&entry("older", "2026-10-16T04:00:00Z"))
        .unwrap();
    let tie_id = repo.insert(&entry("tie", "2026-10-17T03:00:00Z")).unwrap();
    drop(repo);
    let repo = SqliteHistoryRepository::open(&path).unwrap();
    let rows = repo.list(2).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, Some(tie_id));
    assert_eq!(
        rows[1],
        HistoryEntry {
            id: Some(newer_id),
            ..newer
        }
    );
    assert!(repo.list(0).unwrap().is_empty());
    assert_eq!(repo.list(100).unwrap().len(), 3);
}

#[test]
fn storage_tests_history_deletes_one_entry_and_clears_all() {
    let temp = tempfile::tempdir().unwrap();
    let repo = SqliteHistoryRepository::open(temp.path().join("data.sqlite")).unwrap();
    let first = repo
        .insert(&entry("first", "2026-10-17T03:00:00Z"))
        .unwrap();
    let second = repo
        .insert(&entry("second", "2026-10-17T04:00:00Z"))
        .unwrap();
    repo.delete(first).unwrap();
    repo.delete(first).unwrap();
    assert_eq!(repo.list(10).unwrap()[0].id, Some(second));
    assert_eq!(repo.list(10).unwrap().len(), 1);
    repo.clear().unwrap();
    assert!(repo.list(10).unwrap().is_empty());
}

#[test]
fn storage_tests_history_disabled_flag_is_available_without_deleting_saved_history() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("data.sqlite");
    let history = SqliteHistoryRepository::open(&path).unwrap();
    history
        .insert(&entry("saved", "2026-10-17T03:00:00Z"))
        .unwrap();
    let settings = SqliteSettingsRepository::open(&path).unwrap();
    let mut values = settings.load().unwrap();
    values.history_enabled = false;
    settings.save(&values).unwrap();
    assert!(!settings.load().unwrap().history_enabled);
    assert_eq!(history.list(10).unwrap().len(), 1);
}

#[test]
fn storage_tests_history_updates_in_place_and_reports_missing_rows() {
    let temp = tempfile::tempdir().unwrap();
    let repo = SqliteHistoryRepository::open(temp.path().join("data.sqlite")).unwrap();
    let id = repo
        .insert(&entry("first", "2026-10-17T03:00:00Z"))
        .unwrap();
    let stored = repo.get(id).unwrap().expect("inserted row is readable");
    assert_eq!(stored.source_text, "first");
    let updated = HistoryEntry {
        id: Some(id),
        source_text: "edited".into(),
        translated_text: "改过的译文".into(),
        provider_id: ProviderId::new("deepl"),
        ..stored
    };
    repo.update(&updated).unwrap();
    let rows = repo.list(10).unwrap();
    assert_eq!(rows.len(), 1, "an update must not append a row");
    assert_eq!(rows[0].source_text, "edited");
    assert_eq!(rows[0].translated_text, "改过的译文");
    assert_eq!(rows[0].provider_id.as_str(), "deepl");
    // The timestamp is preserved, so the entry keeps its position in the list.
    assert_eq!(rows[0].timestamp, updated.timestamp);
    assert!(repo.get(999).unwrap().is_none());
}

#[test]
fn storage_tests_history_enabled_flag_is_read_without_full_settings_load() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("data.sqlite");
    let settings = SqliteSettingsRepository::open(&path).unwrap();
    assert!(settings.history_enabled().unwrap());
    let mut values = settings.load().unwrap();
    values.history_enabled = false;
    settings.save(&values).unwrap();
    assert!(!settings.history_enabled().unwrap());
}

#[test]
fn storage_tests_repositories_support_shared_runtime_ownership_and_concurrent_open() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SqliteSettingsRepository>();
    assert_send_sync::<SqliteHistoryRepository>();
    assert_send_sync::<KeyringSecureStore>();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("data.sqlite");
    std::thread::scope(|scope| {
        let one = scope.spawn(|| {
            SqliteSettingsRepository::open(&path)
                .unwrap()
                .load()
                .unwrap()
        });
        let two = scope.spawn(|| {
            SqliteHistoryRepository::open(&path)
                .unwrap()
                .list(1)
                .unwrap()
        });
        assert!(one.join().unwrap().history_enabled);
        assert!(two.join().unwrap().is_empty());
    });
}

#[test]
fn storage_tests_corrupt_settings_return_a_typed_error() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("data.sqlite");
    let repo = SqliteSettingsRepository::open(&path).unwrap();
    let raw = rusqlite::Connection::open(&path).unwrap();
    raw.execute(
        "INSERT INTO settings (id, payload) VALUES (1, ?1)",
        ["{invalid}"],
    )
    .unwrap();
    assert!(matches!(repo.load(), Err(StorageError::InvalidData)));
}

#[derive(Default)]
struct FakeCredentials(Mutex<HashMap<(String, String), String>>);

impl CredentialBackend for FakeCredentials {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), keyring::Error> {
        self.0
            .lock()
            .unwrap()
            .insert((service.into(), account.into()), secret.into());
        Ok(())
    }
    fn get(&self, service: &str, account: &str) -> Result<String, keyring::Error> {
        self.0
            .lock()
            .unwrap()
            .get(&(service.into(), account.into()))
            .cloned()
            .ok_or(keyring::Error::NoEntry)
    }
    fn delete(&self, service: &str, account: &str) -> Result<(), keyring::Error> {
        self.0
            .lock()
            .unwrap()
            .remove(&(service.into(), account.into()))
            .map(|_| ())
            .ok_or(keyring::Error::NoEntry)
    }
}

#[test]
fn storage_tests_secure_store_handles_missing_credentials_and_namespaces_accounts() {
    let store: Box<dyn SecureStore> = Box::new(KeyringSecureStore::with_backend(Arc::new(
        FakeCredentials::default(),
    )));
    assert_eq!(store.get("app", "one").unwrap(), None);
    store.set("app", "one", "first-secret").unwrap();
    store.set("app", "two", "other-secret").unwrap();
    store.set("other-app", "one", "third-secret").unwrap();
    store.set("app", "one", "updated-secret").unwrap();
    assert_eq!(
        store.get("app", "one").unwrap().as_deref(),
        Some("updated-secret")
    );
    assert_eq!(
        store.get("app", "two").unwrap().as_deref(),
        Some("other-secret")
    );
    assert_eq!(
        store.get("other-app", "one").unwrap().as_deref(),
        Some("third-secret")
    );
    store.delete("app", "one").unwrap();
    store.delete("app", "one").unwrap();
    assert_eq!(store.get("app", "one").unwrap(), None);
}

struct FailingCredentials;
impl CredentialBackend for FailingCredentials {
    fn set(&self, _: &str, _: &str, secret: &str) -> Result<(), keyring::Error> {
        Err(keyring::Error::PlatformFailure(Box::new(
            std::io::Error::other(secret.to_string()),
        )))
    }
    fn get(&self, _: &str, _: &str) -> Result<String, keyring::Error> {
        Err(keyring::Error::NoStorageAccess(Box::new(
            std::io::Error::other("raw-platform-data"),
        )))
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), keyring::Error> {
        Err(keyring::Error::PlatformFailure(Box::new(
            std::io::Error::other("raw-platform-data"),
        )))
    }
}

#[test]
fn storage_tests_secure_store_redacts_platform_errors_and_supplied_secrets() {
    let store = KeyringSecureStore::with_backend(Arc::new(FailingCredentials));
    let err = store.set("app", "account", "supplied-secret").unwrap_err();
    assert_eq!(err, SecureStoreError::Unavailable);
    assert!(!format!("{err:?} {err}").contains("supplied-secret"));
    let err = store.get("app", "account").unwrap_err();
    assert_eq!(err, SecureStoreError::AccessDenied);
    assert!(!format!("{err:?} {err}").contains("raw-platform-data"));
    assert_eq!(
        store.delete("app", "account").unwrap_err(),
        SecureStoreError::Unavailable
    );
}
