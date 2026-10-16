use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use async_trait::async_trait;
use quick_translate_lib::{
    domain::{
        PrivacyMode, ProviderConfig, ProviderError, ProviderId, TranslationProvider,
        TranslationRequest, TranslationResult,
    },
    platform::secure_store::{SecureStore, SecureStoreError},
    providers::{HttpTimeouts, ProviderRegistry, CREDENTIAL_SERVICE},
    services::translation_router::{TranslationError, TranslationRouter},
};
use serde_json::Value;

fn request() -> TranslationRequest {
    TranslationRequest::new(
        "Hello & 你好".into(),
        "en".parse().unwrap(),
        "zh-CN".parse().unwrap(),
    )
    .unwrap()
}

fn config(id: &str, endpoint: &str, local: bool, enabled: bool) -> ProviderConfig {
    ProviderConfig::new(
        ProviderId::new(id),
        id.into(),
        Some(endpoint.parse().unwrap()),
        if id == "mymemory" || id == "deepl" {
            None
        } else {
            Some("test-model".into())
        },
        if id == "mymemory" {
            None
        } else {
            Some("test-account".into())
        },
        local,
        enabled,
    )
    .unwrap()
}

#[derive(Default)]
struct Secrets {
    reads: Mutex<Vec<(String, String)>>,
}
impl SecureStore for Secrets {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecureStoreError> {
        self.reads
            .lock()
            .unwrap()
            .push((service.into(), account.into()));
        Ok(Some("super-secret-key".into()))
    }
    fn set(&self, _: &str, _: &str, _: &str) -> Result<(), SecureStoreError> {
        unreachable!()
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), SecureStoreError> {
        unreachable!()
    }
}

// Real HTTP boundary; records the entire request, including its body.
fn server(
    status: u16,
    headers: &str,
    body: &str,
    delay: Duration,
) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/api", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    let response = format!("HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0; 4096];
            let count = stream.read(&mut chunk).unwrap();
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        sender.send(String::from_utf8(bytes).unwrap()).unwrap();
        thread::sleep(delay);
        let _ = stream.write_all(response.as_bytes());
    });
    (endpoint, receiver, handle)
}

fn registry(configs: &[ProviderConfig]) -> ProviderRegistry {
    ProviderRegistry::from_configs_with_timeouts(
        configs,
        Arc::new(Secrets::default()),
        HttpTimeouts {
            connect: Duration::from_secs(1),
            total: Duration::from_secs(2),
        },
    )
    .unwrap()
}

#[tokio::test]
async fn provider_tests_mymemory_parses_response_and_encodes_query() {
    let (url, received, handle) = server(
        200,
        "",
        r#"{"responseData":{"translatedText":"你好"},"responseStatus":200,"responseDetails":"","quotaFinished":false,"matches":[]}"#,
        Duration::ZERO,
    );
    let registry = registry(&[config("mymemory", &url, false, true)]);
    let result = registry
        .translate(&ProviderId::new("mymemory"), &request())
        .await
        .unwrap();
    assert_eq!(result.translated_text, "你好");
    assert_eq!(result.provider_id, "mymemory");
    assert_eq!(result.character_count, 10);
    let received = received.recv().unwrap();
    let target = received
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();
    let query: std::collections::HashMap<_, _> =
        url::Url::parse(&format!("http://localhost{target}"))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
    assert_eq!(query.get("q").unwrap(), "Hello & 你好");
    assert_eq!(query.get("langpair").unwrap(), "en|zh-CN");
    let metadata = registry.metadata(&ProviderId::new("mymemory")).unwrap();
    assert!(!metadata.is_local);
    assert!(metadata.attribution.as_ref().unwrap().contains("MyMemory"));
    assert!(!metadata.limits_notice.as_ref().unwrap().is_empty());
    handle.join().unwrap();
}

#[tokio::test]
async fn provider_tests_deepl_reads_keyring_and_sends_authentication_header() {
    let (url, received, handle) = server(
        200,
        "",
        r#"{"translations":[{"detected_source_language":"EN","text":"你好"}]}"#,
        Duration::ZERO,
    );
    let secrets = Arc::new(Secrets::default());
    let registry =
        ProviderRegistry::from_configs(&[config("deepl", &url, false, true)], secrets.clone())
            .unwrap();
    let result = registry
        .translate(&ProviderId::new("deepl"), &request())
        .await
        .unwrap();
    assert_eq!(result.translated_text, "你好");
    let received = received.recv().unwrap();
    assert!(received
        .to_lowercase()
        .contains("authorization: deepl-auth-key super-secret-key\r\n"));
    let body: Value = serde_json::from_str(received.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["text"][0], "Hello & 你好");
    assert_eq!(body["target_lang"], "ZH-HANS");
    assert_eq!(
        *secrets.reads.lock().unwrap(),
        vec![(CREDENTIAL_SERVICE.into(), "test-account".into())]
    );
    handle.join().unwrap();
}

#[tokio::test]
async fn provider_tests_deepl_preserves_chinese_target_script_in_http_body() {
    for (target, expected) in [("zh-CN", "ZH-HANS"), ("zh-TW", "ZH-HANT")] {
        let (url, received, handle) = server(
            200,
            "",
            r#"{"translations":[{"text":"translated"}]}"#,
            Duration::ZERO,
        );
        let request = TranslationRequest::new(
            "hello".into(),
            "en".parse().unwrap(),
            target.parse().unwrap(),
        )
        .unwrap();
        registry(&[config("deepl", &url, false, true)])
            .translate(&ProviderId::new("deepl"), &request)
            .await
            .unwrap();
        let received = received.recv().unwrap();
        let body: Value = serde_json::from_str(received.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["target_lang"], expected, "target {target}");
        assert_eq!(body["source_lang"], "EN");
        handle.join().unwrap();
    }
}

#[tokio::test]
async fn provider_tests_openai_parses_chat_and_preserves_configured_id() {
    let (url, received, handle) = server(
        200,
        "",
        r#"{"id":"chatcmpl-test","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"你好"},"finish_reason":"stop"}]}"#,
        Duration::ZERO,
    );
    let registry = registry(&[config("office-local", &url, true, true)]);
    let result = registry
        .translate(&ProviderId::new("office-local"), &request())
        .await
        .unwrap();
    assert_eq!(result.provider_id, "office-local");
    assert_eq!(result.translated_text, "你好");
    let received = received.recv().unwrap();
    assert!(received.starts_with("POST /api/chat/completions "));
    assert!(received
        .to_lowercase()
        .contains("authorization: bearer super-secret-key\r\n"));
    let body: Value = serde_json::from_str(received.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["model"], "test-model");
    assert_eq!(body["messages"][1]["content"], "Hello & 你好");
    assert!(body["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains("zh-CN"));
    handle.join().unwrap();
}

#[tokio::test]
async fn provider_tests_http_status_classification_and_error_redaction() {
    for (status, expected) in [
        (401, ProviderError::Authentication),
        (403, ProviderError::Authentication),
        (
            429,
            ProviderError::RateLimited {
                retry_after_seconds: Some(12),
            },
        ),
        (503, ProviderError::Service { status: Some(503) }),
    ] {
        for id in ["mymemory", "deepl", "openai-compatible"] {
            let (url, received, handle) = server(
                status,
                "Retry-After: 12\r\n",
                "super-secret-key private request content",
                Duration::ZERO,
            );
            let error = registry(&[config(id, &url, false, true)])
                .translate(&ProviderId::new(id), &request())
                .await
                .unwrap_err();
            assert_eq!(error, expected);
            assert!(!format!("{error:?} {error}").contains("super-secret-key"));
            received.recv().unwrap();
            handle.join().unwrap();
        }
    }
}

#[tokio::test]
async fn provider_tests_malformed_json_and_schema_mismatches() {
    for id in ["mymemory", "deepl", "openai-compatible"] {
        for body in [
            "invalid secret response",
            "{}",
            r#"{"translations":[],"choices":[],"responseData":{"translatedText":""},"responseStatus":200}"#,
        ] {
            let (url, received, handle) = server(200, "", body, Duration::ZERO);
            assert_eq!(
                registry(&[config(id, &url, false, true)])
                    .translate(&ProviderId::new(id), &request())
                    .await
                    .unwrap_err(),
                ProviderError::InvalidResponse
            );
            received.recv().unwrap();
            handle.join().unwrap();
        }
    }
}

#[tokio::test]
async fn provider_tests_mymemory_classifies_service_status_without_translation_data() {
    let (url, received, handle) = server(
        200,
        "",
        r#"{"responseData":"","responseStatus":429,"responseDetails":"private source text","quotaFinished":true,"matches":[]}"#,
        Duration::ZERO,
    );
    assert_eq!(
        registry(&[config("mymemory", &url, false, true)])
            .translate(&ProviderId::new("mymemory"), &request())
            .await
            .unwrap_err(),
        ProviderError::RateLimited {
            retry_after_seconds: None
        }
    );
    received.recv().unwrap();
    handle.join().unwrap();
}

#[tokio::test]
async fn provider_tests_total_timeout_and_network_errors() {
    let (url, received, handle) = server(200, "", "{}", Duration::from_millis(200));
    let registry = ProviderRegistry::from_configs_with_timeouts(
        &[config("mymemory", &url, false, true)],
        Arc::new(Secrets::default()),
        HttpTimeouts {
            connect: Duration::from_millis(30),
            total: Duration::from_millis(60),
        },
    )
    .unwrap();
    assert_eq!(
        registry
            .translate(&ProviderId::new("mymemory"), &request())
            .await
            .unwrap_err(),
        ProviderError::Timeout
    );
    received.recv().unwrap();
    handle.join().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let reset = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.read(&mut [0; 4096]);
        // Close a connected peer without an HTTP response: a transport failure,
        // independent of platform-specific closed-port connection retry timing.
    });
    assert_eq!(
        self::registry(&[config("mymemory", &url, false, true)])
            .translate(&ProviderId::new("mymemory"), &request())
            .await
            .unwrap_err(),
        ProviderError::Network
    );
    reset.join().unwrap();
}

#[tokio::test]
async fn provider_tests_redirects_never_forward_text_or_credentials() {
    let sink = TcpListener::bind("127.0.0.1:0").unwrap();
    sink.set_nonblocking(true).unwrap();
    for id in ["mymemory", "deepl", "openai-compatible"] {
        let (url, received, handle) = server(
            307,
            &format!("Location: http://{}/stolen\r\n", sink.local_addr().unwrap()),
            "",
            Duration::ZERO,
        );
        assert_eq!(
            registry(&[config(id, &url, true, true)])
                .translate(&ProviderId::new(id), &request())
                .await
                .unwrap_err(),
            ProviderError::Service { status: Some(307) }
        );
        received.recv().unwrap();
        handle.join().unwrap();
        assert_eq!(
            sink.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn provider_tests_remote_endpoints_cannot_be_marked_local() {
    for id in ["mymemory", "deepl", "openai-compatible"] {
        assert!(ProviderRegistry::from_configs(
            &[config(id, "https://example.com/api", true, true)],
            Arc::new(Secrets::default())
        )
        .is_err());
    }
}

#[tokio::test]
async fn provider_tests_mymemory_rejects_oversized_utf8_before_network() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/get", listener.local_addr().unwrap());
    let request = TranslationRequest::new(
        "你".repeat(167),
        "en".parse().unwrap(),
        "zh-CN".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(
        registry(&[config("mymemory", &url, false, true)])
            .translate(&ProviderId::new("mymemory"), &request)
            .await
            .unwrap_err(),
        ProviderError::Configuration
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

struct MissingSecret;
impl SecureStore for MissingSecret {
    fn get(&self, _: &str, _: &str) -> Result<Option<String>, SecureStoreError> {
        Ok(None)
    }
    fn set(&self, _: &str, _: &str, _: &str) -> Result<(), SecureStoreError> {
        unreachable!()
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), SecureStoreError> {
        unreachable!()
    }
}

#[tokio::test]
async fn provider_tests_missing_secret_blocks_authenticated_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    for id in ["deepl", "openai-compatible"] {
        let registry = ProviderRegistry::from_configs(
            &[config(id, &url, false, true)],
            Arc::new(MissingSecret),
        )
        .unwrap();
        assert_eq!(
            registry
                .translate(&ProviderId::new(id), &request())
                .await
                .unwrap_err(),
            ProviderError::Authentication
        );
    }
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn provider_tests_local_endpoint_can_run_without_a_secret() {
    let (url, received, handle) = server(
        200,
        "",
        r#"{"choices":[{"message":{"content":"本地"}}]}"#,
        Duration::ZERO,
    );
    let config = ProviderConfig::new(
        ProviderId::new("local-keyless"),
        "Local".into(),
        Some(url.parse().unwrap()),
        Some("local-model".into()),
        None,
        true,
        true,
    )
    .unwrap();
    let registry = ProviderRegistry::from_configs(&[config], Arc::new(MissingSecret)).unwrap();
    assert_eq!(
        registry
            .translate(&ProviderId::new("local-keyless"), &request())
            .await
            .unwrap()
            .translated_text,
        "本地"
    );
    assert!(!received
        .recv()
        .unwrap()
        .to_lowercase()
        .contains("authorization:"));
    handle.join().unwrap();
}

struct Fake {
    calls: Arc<AtomicUsize>,
    output: Result<&'static str, ProviderError>,
}
#[async_trait]
impl TranslationProvider for Fake {
    fn id(&self) -> &'static str {
        "fake"
    }
    async fn translate(&self, _: &TranslationRequest) -> Result<TranslationResult, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.output.clone().map(|text| TranslationResult {
            translated_text: text.into(),
            provider_id: "fake".into(),
            elapsed_ms: 0,
            character_count: 10,
        })
    }
}
fn register(
    registry: &mut ProviderRegistry,
    id: &str,
    local: bool,
    enabled: bool,
    output: Result<&'static str, ProviderError>,
) -> Arc<AtomicUsize> {
    let calls = Arc::new(AtomicUsize::new(0));
    registry
        .register(
            config(id, "http://127.0.0.1:12345/v1", local, enabled),
            Arc::new(Fake {
                calls: calls.clone(),
                output,
            }),
        )
        .unwrap();
    calls
}

#[tokio::test]
async fn provider_tests_local_only_filters_before_dispatch_even_on_failure() {
    for fallback in [true, false] {
        let mut registry = ProviderRegistry::new();
        let remote = register(&mut registry, "remote", false, true, Ok("remote text"));
        let local = register(
            &mut registry,
            "local",
            true,
            true,
            Err(ProviderError::Network),
        );
        let router = TranslationRouter::new(registry);
        assert_eq!(
            router
                .translate(
                    PrivacyMode::LocalOnly,
                    &ProviderId::new("remote"),
                    &request(),
                    fallback
                )
                .await
                .unwrap_err(),
            TranslationError::Provider(ProviderError::Network)
        );
        assert_eq!(remote.load(Ordering::SeqCst), 0);
        assert_eq!(local.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn provider_tests_local_only_empty_or_disabled_has_typed_error() {
    let mut registry = ProviderRegistry::new();
    let remote = register(&mut registry, "remote", false, true, Ok("remote"));
    let disabled = register(&mut registry, "disabled-local", true, false, Ok("disabled"));
    assert_eq!(
        TranslationRouter::new(registry)
            .translate(
                PrivacyMode::LocalOnly,
                &ProviderId::new("remote"),
                &request(),
                true
            )
            .await
            .unwrap_err(),
        TranslationError::NoLocalProvider
    );
    assert_eq!(remote.load(Ordering::SeqCst), 0);
    assert_eq!(disabled.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn provider_tests_online_selection_and_local_fallback_are_deterministic() {
    for fallback in [true, false] {
        let mut registry = ProviderRegistry::new();
        let remote = register(
            &mut registry,
            "remote",
            false,
            true,
            Err(ProviderError::Timeout),
        );
        let z = register(&mut registry, "z-local", true, true, Ok("z text"));
        let a = register(&mut registry, "a-local", true, true, Ok("a text"));
        let other = register(
            &mut registry,
            "other-remote",
            false,
            true,
            Ok("other remote"),
        );
        let result = TranslationRouter::new(registry)
            .translate(
                PrivacyMode::OnlinePreferred,
                &ProviderId::new("remote"),
                &request(),
                fallback,
            )
            .await;
        if fallback {
            let result = result.unwrap();
            assert_eq!(result.translated_text, "a text");
            assert_eq!(result.provider_id, "a-local");
        } else {
            assert_eq!(
                result.unwrap_err(),
                TranslationError::Provider(ProviderError::Timeout)
            );
        }
        assert_eq!(remote.load(Ordering::SeqCst), 1);
        assert_eq!(a.load(Ordering::SeqCst), usize::from(fallback));
        assert_eq!(z.load(Ordering::SeqCst), 0);
        assert_eq!(other.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn provider_tests_successful_selection_skips_fallback_and_local_selection_is_respected() {
    for mode in [PrivacyMode::OnlinePreferred, PrivacyMode::LocalOnly] {
        let mut registry = ProviderRegistry::new();
        let selected = register(&mut registry, "z-selected", true, true, Ok("selected"));
        let other = register(&mut registry, "a-other", true, true, Ok("other"));
        let result = TranslationRouter::new(registry)
            .translate(mode, &ProviderId::new("z-selected"), &request(), true)
            .await
            .unwrap();
        assert_eq!(result.provider_id, "z-selected");
        assert_eq!(selected.load(Ordering::SeqCst), 1);
        assert_eq!(other.load(Ordering::SeqCst), 0);
    }
}
