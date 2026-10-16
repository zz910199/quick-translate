use quick_translate_lib::{
    commands::{
        settings::{ProviderConfigInput, SettingsInput},
        translation::AppServices,
    },
    domain::PrivacyMode,
    platform::{
        clipboard::{Clipboard, ClipboardError},
        secure_store::{SecureStore, SecureStoreError},
        shortcut::{GlobalShortcut, ShortcutController},
    },
    storage::{SettingsRepository, SqliteHistoryRepository, SqliteSettingsRepository},
};
use std::sync::{Arc, Mutex};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
#[derive(Default)]
struct Secrets(Mutex<Option<String>>);
impl SecureStore for Secrets {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecureStoreError> {
        assert_eq!(service, "quick-translate");
        assert_eq!(account, "provider:local");
        *self.0.lock().unwrap() = Some(secret.into());
        Ok(())
    }
    fn get(&self, _: &str, _: &str) -> Result<Option<String>, SecureStoreError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), SecureStoreError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}
struct Clip;
impl Clipboard for Clip {
    fn write_text(&self, _: &str) -> Result<(), ClipboardError> {
        Ok(())
    }
}
fn fixture() -> (
    tempfile::TempDir,
    AppServices,
    Arc<SqliteSettingsRepository>,
    Arc<Secrets>,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.sqlite");
    let settings = Arc::new(SqliteSettingsRepository::open(&path).unwrap());
    let secrets = Arc::new(Secrets::default());
    let services = AppServices::new(
        settings.clone(),
        Arc::new(SqliteHistoryRepository::open(&path).unwrap()),
        secrets.clone(),
        Arc::new(Clip),
    );
    (dir, services, settings, secrets)
}
fn local(endpoint: &str) -> ProviderConfigInput {
    ProviderConfigInput {
        id: "local".into(),
        display_name: "Local".into(),
        endpoint: Some(endpoint.into()),
        model: Some("model".into()),
        is_local: true,
        enabled: true,
    }
}
#[tokio::test]
async fn shortcut_secret_command_tests_saved_local_service_translates_without_key() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    loop {
                        let mut chunk = [0; 4096];
                        let count = stream.read(&mut chunk).unwrap();
                        bytes.extend_from_slice(&chunk[..count]);
                        if count == 0 {
                            break;
                        }
                        if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                            let length = headers
                                .lines()
                                .find_map(|line| {
                                    line.strip_prefix("content-length:")
                                        .and_then(|n| n.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + length {
                                break;
                            }
                        }
                    }
                    let body = r#"{"choices":[{"message":{"content":"本地翻译"}}]}"#;
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                    return Some(String::from_utf8(bytes).unwrap());
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(5))
                }
                _ => return None,
            }
        }
    });
    let (_dir, service, _, _) = fixture();
    service.save_provider_config(local(&endpoint)).unwrap();
    let credential_present = service
        .get_settings()
        .unwrap()
        .providers
        .into_iter()
        .find(|p| p.config.id == "local")
        .unwrap()
        .configured;
    assert!(
        !credential_present,
        "settings reports credential presence only"
    );
    let usable = service
        .list_providers()
        .unwrap()
        .into_iter()
        .find(|p| p.id == "local")
        .unwrap()
        .configured;
    let result = service
        .translate_text("Hello".into(), "en".into(), "zh-CN".into(), "local".into())
        .await;
    let received = handle.join().unwrap();
    assert!(usable, "a local service is usable without a credential");
    assert_eq!(result.unwrap().translated_text, "本地翻译");
    assert!(!received.unwrap().to_lowercase().contains("authorization:"));
}
#[test]
fn shortcut_secret_command_tests_secret_roundtrip_returns_only_configured_status() {
    let (_dir, service, settings, secrets) = fixture();
    service
        .save_provider_config(local("http://localhost:11434/v1"))
        .unwrap();
    service
        .set_provider_secret("local".into(), "private-test-key".into())
        .unwrap();
    assert_eq!(
        secrets.0.lock().unwrap().as_deref(),
        Some("private-test-key")
    );
    let dto = serde_json::to_value(service.get_settings().unwrap()).unwrap();
    assert_eq!(dto["providers"][1]["configured"], true);
    assert!(!dto.to_string().contains("private-test-key"));
    assert!(!serde_json::to_string(&settings.load().unwrap())
        .unwrap()
        .contains("private-test-key"));
    service.delete_provider_secret("local".into()).unwrap();
    assert!(secrets.0.lock().unwrap().is_none());
    let dto = serde_json::to_value(service.get_settings().unwrap()).unwrap();
    assert_eq!(dto["providers"][1]["configured"], false);
}
#[test]
fn shortcut_secret_command_tests_reject_remote_local_endpoints_and_accept_loopback() {
    let (_dir, service, _, _) = fixture();
    for endpoint in ["https://example.com/v1", "https://localhost.evil.test/v1"] {
        assert_eq!(
            service
                .save_provider_config(local(endpoint))
                .unwrap_err()
                .code,
            "invalid_local_endpoint"
        );
    }
    for endpoint in [
        "http://localhost:11434/v1",
        "http://127.0.0.1:1234/v1",
        "http://[::1]:1234/v1",
    ] {
        service.save_provider_config(local(endpoint)).unwrap();
    }
}
#[derive(Default)]
struct Shortcuts(Mutex<Vec<String>>);
impl GlobalShortcut for Shortcuts {
    fn register(
        &self,
        accelerator: &str,
        _callback: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(), quick_translate_lib::commands::translation::AppErrorDto> {
        if accelerator == "Ctrl+Alt+Q" {
            return Err(
                quick_translate_lib::commands::translation::AppErrorDto::new(
                    "shortcut_conflict",
                    "快捷键已被占用，请更换组合。",
                    false,
                ),
            );
        }
        self.0.lock().unwrap().push(accelerator.into());
        Ok(())
    }
    fn unregister(
        &self,
        accelerator: &str,
    ) -> Result<(), quick_translate_lib::commands::translation::AppErrorDto> {
        self.0.lock().unwrap().retain(|v| v != accelerator);
        Ok(())
    }
}
#[test]
fn shortcut_secret_command_tests_conflict_retains_previous_registration_and_settings() {
    let shortcuts = Arc::new(Shortcuts::default());
    let controller = ShortcutController::new(shortcuts.clone(), Arc::new(|| {}));
    controller.apply("Ctrl+Shift+T", || Ok(())).unwrap();
    let mut persisted = false;
    let error = controller
        .apply("Ctrl+Alt+Q", || {
            persisted = true;
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code, "shortcut_conflict");
    assert!(error.message.contains("占用"));
    assert!(!persisted);
    assert_eq!(*shortcuts.0.lock().unwrap(), vec!["Ctrl+Shift+T"]);
    let result = controller.apply("Ctrl+Shift+Y", || {
        Err(quick_translate_lib::commands::translation::AppErrorDto::new("storage", "error", true))
    });
    assert!(result.is_err());
    assert_eq!(*shortcuts.0.lock().unwrap(), vec!["Ctrl+Shift+T"]);
}
#[test]
fn shortcut_secret_command_tests_sensitive_mode_selects_local_and_preserves_history() {
    let (_dir, service, settings, _) = fixture();
    service
        .save_provider_config(local("http://localhost:11434/v1"))
        .unwrap();
    service
        .save_settings(SettingsInput {
            source: "ja".into(),
            target: "en".into(),
            provider_id: "mymemory".into(),
            privacy_mode: PrivacyMode::LocalOnly,
            fallback_enabled: false,
            history_enabled: false,
            shortcut: "Ctrl+Shift+T".into(),
        })
        .unwrap();
    let saved = settings.load().unwrap();
    assert_eq!(saved.active_provider.as_str(), "local");
    assert!(!saved.history_enabled);
    assert!(!saved.fallback_enabled);
    assert_eq!(saved.source_language.as_str(), "ja");
}
#[test]
fn shortcut_secret_command_tests_history_limits_and_mutations_preserve_disabled_records() {
    use quick_translate_lib::{domain::ProviderId, storage::HistoryEntry};
    let (_dir, service, _, _) = fixture();
    for i in 0..105 {
        service
            .history
            .insert(&HistoryEntry {
                id: None,
                source_text: format!("row-{i}"),
                translated_text: "译文".into(),
                source_language: "en".parse().unwrap(),
                target_language: "zh-CN".parse().unwrap(),
                provider_id: ProviderId::new("mymemory"),
                timestamp: chrono::Utc::now(),
            })
            .unwrap();
    }
    service
        .save_settings(SettingsInput {
            source: "en".into(),
            target: "zh-CN".into(),
            provider_id: "mymemory".into(),
            privacy_mode: PrivacyMode::OnlinePreferred,
            fallback_enabled: true,
            history_enabled: false,
            shortcut: "Ctrl+Shift+T".into(),
        })
        .unwrap();
    let rows = service.list_history().unwrap();
    assert_eq!(rows.len(), 100);
    assert_eq!(rows[0].source_text, "row-104");
    assert_eq!(rows[99].source_text, "row-5");
    service.delete_history_entry(rows[0].id).unwrap();
    assert_eq!(service.list_history().unwrap()[0].source_text, "row-103");
    service.clear_history().unwrap();
    assert!(service.list_history().unwrap().is_empty());
}
struct CannotRelease(Shortcuts);
impl GlobalShortcut for CannotRelease {
    fn register(
        &self,
        accelerator: &str,
        callback: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(), quick_translate_lib::commands::translation::AppErrorDto> {
        self.0.register(accelerator, callback)
    }
    fn unregister(
        &self,
        accelerator: &str,
    ) -> Result<(), quick_translate_lib::commands::translation::AppErrorDto> {
        if accelerator == "Ctrl+Shift+T" {
            Err(
                quick_translate_lib::commands::translation::AppErrorDto::new(
                    "shortcut_conflict",
                    "无法注销原快捷键。",
                    false,
                ),
            )
        } else {
            self.0.unregister(accelerator)
        }
    }
}
#[test]
fn shortcut_secret_command_tests_release_failure_does_not_persist_or_leave_two_shortcuts() {
    let shortcuts = Arc::new(CannotRelease(Shortcuts::default()));
    let controller = ShortcutController::new(shortcuts.clone(), Arc::new(|| {}));
    controller.apply("Ctrl+Shift+T", || Ok(())).unwrap();
    let mut persisted = false;
    assert!(controller
        .apply("Ctrl+Shift+Y", || {
            persisted = true;
            Ok(())
        })
        .is_err());
    assert!(!persisted);
    assert_eq!(*shortcuts.0 .0.lock().unwrap(), vec!["Ctrl+Shift+T"]);
}
