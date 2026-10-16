use quick_translate_lib::{
    commands::translation::AppServices,
    domain::{PrivacyMode, ProviderConfig, ProviderId},
    platform::{
        clipboard::{Clipboard, ClipboardError},
        screen_capture::{
            CaptureError, CapturedDisplay, ImageRegion, ScreenCapture, ScreenshotRect,
        },
        secure_store::{SecureStore, SecureStoreError},
    },
    services::ocr::{OcrEngine, OcrResult, OcrService},
    storage::{
        HistoryRepository, SettingsRepository, SqliteHistoryRepository, SqliteSettingsRepository,
    },
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

struct FakeSecrets;
impl SecureStore for FakeSecrets {
    fn set(&self, _: &str, _: &str, _: &str) -> Result<(), SecureStoreError> {
        Ok(())
    }
    fn get(&self, _: &str, _: &str) -> Result<Option<String>, SecureStoreError> {
        Ok(None)
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), SecureStoreError> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeClipboard(Mutex<String>);
impl Clipboard for FakeClipboard {
    fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
        *self.0.lock().unwrap() = text.to_owned();
        Ok(())
    }
}

// A real loopback HTTP boundary: the production adapter serializes and parses each request.
fn mock_server(
    count: usize,
) -> (
    String,
    mpsc::Receiver<serde_json::Value>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        for _ in 0..count {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let body_start = loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let len = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + len {
                        break end + 4;
                    }
                }
            };
            let body: serde_json::Value = serde_json::from_slice(&bytes[body_start..]).unwrap();
            let translated = format!(
                "translated: {}",
                body["messages"][1]["content"].as_str().unwrap()
            );
            tx.send(body).unwrap();
            let response =
                serde_json::json!({"choices": [{"message": {"content": translated}}]}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        }
    });
    (endpoint, rx, worker)
}

struct FakeCapture;
impl ScreenCapture for FakeCapture {
    fn capture_all_displays(&self) -> Result<Vec<CapturedDisplay>, CaptureError> {
        Ok(vec![CapturedDisplay {
            id: "display-0".into(),
            origin_x: 0,
            origin_y: 0,
            scale_factor: 1.0,
            preview_data_url: "data:image/jpeg;base64,AA==".into(),
            width: 2,
            height: 2,
            rgba: vec![255; 16],
        }])
    }
}

struct FakeOcr;
impl OcrEngine for FakeOcr {
    fn recognize(
        &self,
        image: ImageRegion,
        _: Option<&quick_translate_lib::domain::LanguageCode>,
    ) -> Result<OcrResult, quick_translate_lib::services::ocr::OcrError> {
        assert_eq!((image.width, image.height), (1, 1));
        Ok(OcrResult {
            text: "截图文字".into(),
            confidence: Some(0.99),
            blocks: vec![],
        })
    }
}

fn fixture(endpoint: &str) -> (tempfile::TempDir, AppServices, Arc<SqliteHistoryRepository>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("integration.sqlite3");
    let settings = Arc::new(SqliteSettingsRepository::open(&db).unwrap());
    let history = Arc::new(SqliteHistoryRepository::open(&db).unwrap());
    let mut values = settings.load().unwrap();
    values.providers.push(
        ProviderConfig::new(
            ProviderId::new("local-test"),
            "Local test".into(),
            Some(endpoint.parse().unwrap()),
            Some("test-model".into()),
            None,
            true,
            true,
        )
        .unwrap(),
    );
    values.active_provider = ProviderId::new("local-test");
    settings.save(&values).unwrap();
    let services = AppServices::new(
        settings,
        history.clone(),
        Arc::new(FakeSecrets),
        Arc::new(FakeClipboard::default()),
    );
    (dir, services, history)
}

#[tokio::test]
async fn app_integration_tests_manual_translation_and_history_policy() {
    let (endpoint, requests, worker) = mock_server(2);
    let (_dir, services, history) = fixture(&endpoint);
    let result = services
        .translate_text(
            "hello".into(),
            "en".into(),
            "zh-CN".into(),
            "local-test".into(),
        )
        .await
        .unwrap();
    assert_eq!(result.translated_text, "translated: hello");
    assert_eq!(history.list(10).unwrap().len(), 1);
    assert_eq!(result.provider_id, "local-test");
    services
        .update_settings(|settings| {
            settings.history_enabled = false;
            Ok(())
        })
        .unwrap();
    services
        .translate_text(
            "again".into(),
            "en".into(),
            "zh-CN".into(),
            "local-test".into(),
        )
        .await
        .unwrap();
    assert_eq!(history.list(10).unwrap().len(), 1);
    assert_eq!(
        requests.recv_timeout(Duration::from_secs(2)).unwrap()["model"],
        "test-model"
    );
    assert_eq!(
        requests.recv_timeout(Duration::from_secs(2)).unwrap()["messages"][1]["content"],
        "again"
    );
    worker.join().unwrap();
}

#[tokio::test]
async fn app_integration_tests_screenshot_ocr_confirmation_translates_edited_text() {
    let (endpoint, requests, worker) = mock_server(1);
    let (_dir, services, history) = fixture(&endpoint);
    let ocr = OcrService::new(Arc::new(FakeCapture), Arc::new(FakeOcr));
    let display = ocr.begin().unwrap().remove(0);
    let recognized = ocr
        .recognize(
            &display.id,
            ScreenshotRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            None,
        )
        .unwrap();
    assert_eq!(recognized.text, "截图文字");
    assert!(history.list(10).unwrap().is_empty());
    assert!(requests.try_recv().is_err()); // OCR alone never dispatches translation.
    services
        .update_settings(|settings| {
            settings.privacy_mode = PrivacyMode::LocalOnly;
            Ok(())
        })
        .unwrap();
    // A stale online selection is routed to the only local provider in sensitive mode.
    let result = services
        .translate_text(
            "edited text".into(),
            "zh-CN".into(),
            "en".into(),
            "mymemory".into(),
        )
        .await
        .unwrap();
    assert_eq!(result.translated_text, "translated: edited text");
    assert_eq!(history.list(10).unwrap()[0].source_text, "edited text");
    assert_eq!(result.provider_id, "local-test");
    assert_eq!(
        requests.recv_timeout(Duration::from_secs(2)).unwrap()["messages"][1]["content"],
        "edited text"
    );
    worker.join().unwrap();
}
