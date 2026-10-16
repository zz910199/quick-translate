use async_trait::async_trait;
use quick_translate_lib::{
    commands::translation::AppServices,
    domain::{
        PrivacyMode, ProviderError, ProviderId, TranslationProvider, TranslationRequest,
        TranslationResult,
    },
    platform::{
        clipboard::{Clipboard, ClipboardError},
        secure_store::{SecureStore, SecureStoreError},
    },
    providers::ProviderRegistry,
    storage::{
        HistoryRepository, SettingsRepository, SqliteHistoryRepository, SqliteSettingsRepository,
    },
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
struct Secrets;
impl SecureStore for Secrets {
    fn get(&self, _: &str, _: &str) -> Result<Option<String>, SecureStoreError> {
        Ok(Some("hidden-secret".into()))
    }
    fn set(&self, _: &str, _: &str, _: &str) -> Result<(), SecureStoreError> {
        Ok(())
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), SecureStoreError> {
        Ok(())
    }
}
#[derive(Default)]
struct MemoryClipboard(Mutex<String>);
impl Clipboard for MemoryClipboard {
    fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        *self.0.lock().unwrap() = text.into();
        Ok(())
    }
}
struct FakeProvider(Arc<AtomicUsize>);
#[async_trait]
impl TranslationProvider for FakeProvider {
    fn id(&self) -> &'static str {
        "mymemory"
    }
    async fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, ProviderError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(TranslationResult {
            translated_text: "你好".into(),
            provider_id: "mymemory".into(),
            elapsed_ms: 3,
            character_count: request.text().chars().count(),
        })
    }
}
fn fixture() -> (
    tempfile::TempDir,
    AppServices,
    Arc<SqliteSettingsRepository>,
    Arc<SqliteHistoryRepository>,
    Arc<AtomicUsize>,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.sqlite");
    let settings = Arc::new(SqliteSettingsRepository::open(&path).unwrap());
    let history = Arc::new(SqliteHistoryRepository::open(&path).unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let services = AppServices::new(
        settings.clone(),
        history.clone(),
        Arc::new(Secrets),
        Arc::new(MemoryClipboard::default()),
    )
    .with_registry_factory(Arc::new(move |configs, _| {
        let mut registry = ProviderRegistry::new();
        for config in configs {
            registry.register(config.clone(), Arc::new(FakeProvider(counter.clone())))?;
        }
        Ok(registry)
    }));
    (dir, services, settings, history, calls)
}
#[tokio::test]
async fn translation_command_tests_history_enabled_then_disabled_and_display_safe_dto() {
    let (_dir, service, settings, history, calls) = fixture();
    let result = service
        .translate_text(
            "Hello".into(),
            "en".into(),
            "zh-CN".into(),
            "mymemory".into(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::json!({"translatedText":"你好","providerId":"mymemory","elapsedMs":3,"characterCount":5})
    );
    let entries = history.list(10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].source_text, "Hello");
    let mut value = settings.load().unwrap();
    value.history_enabled = false;
    settings.save(&value).unwrap();
    service
        .translate_text(
            "Again".into(),
            "en".into(),
            "zh-CN".into(),
            "mymemory".into(),
        )
        .await
        .unwrap();
    assert_eq!(history.list(10).unwrap().len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn translation_command_tests_validate_before_dispatch_and_enforce_local_privacy() {
    let (_dir, service, settings, history, calls) = fixture();
    for (text, source, target, code) in [
        (" ".into(), "en", "zh-CN", "empty_text"),
        ("Hello".into(), "en", "en", "identical_languages"),
        ("你".repeat(167), "en", "zh-CN", "input_too_long"),
    ] {
        assert_eq!(
            service
                .translate_text(text, source.into(), target.into(), "mymemory".into())
                .await
                .unwrap_err()
                .code,
            code
        );
    }
    let mut value = settings.load().unwrap();
    value.privacy_mode = PrivacyMode::LocalOnly;
    settings.save(&value).unwrap();
    assert!(service.list_providers().unwrap().is_empty());
    assert_eq!(
        service
            .translate_text(
                "Hello".into(),
                "en".into(),
                "zh-CN".into(),
                "mymemory".into()
            )
            .await
            .unwrap_err()
            .code,
        "no_local_provider"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(history.list(10).unwrap().is_empty());
}
#[test]
fn translation_command_tests_persist_pair_and_provider_without_secrets() {
    let (_dir, service, settings, _, _) = fixture();
    service
        .save_translation_selection("ja".into(), "en".into(), "mymemory".into())
        .unwrap();
    let selection = service.get_translation_selection().unwrap();
    assert_eq!(selection.source, "ja");
    assert_eq!(selection.target, "en");
    assert_eq!(
        settings.load().unwrap().active_provider,
        ProviderId::new("mymemory")
    );
    let providers = serde_json::to_value(service.list_providers().unwrap()).unwrap();
    assert_eq!(providers[0]["configured"], true);
    assert!(providers[0]["notice"].as_str().unwrap().contains("500"));
    assert!(!providers.to_string().contains("hidden-secret"));
}
