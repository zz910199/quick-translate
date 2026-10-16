mod domain_tests {
    use std::sync::Arc;

    use quick_translate_lib::domain::{
        LanguageCode, ProviderConfig, ProviderError, ProviderId, TranslationProvider,
        TranslationRequest, TranslationResult, ValidationError,
    };

    #[test]
    fn accepts_language_codes_and_serializes_as_strings() {
        for code in ["en", "zh-CN", "ja"] {
            let language: LanguageCode = code.parse().unwrap();
            assert_eq!(language.as_str(), code);
            assert_eq!(
                serde_json::to_string(&language).unwrap(),
                format!("\"{code}\"")
            );
        }
    }

    #[test]
    fn rejects_empty_auto_and_malformed_language_codes() {
        for code in ["", "auto", "e", "en_Us", "en-", "en-123456789", "en--US"] {
            assert_eq!(
                code.parse::<LanguageCode>(),
                Err(ValidationError::InvalidLanguageCode)
            );
        }

        assert!(serde_json::from_str::<LanguageCode>("\"auto\"").is_err());
    }

    #[test]
    fn creates_request_for_nonempty_text_and_distinct_languages() {
        let source = "en".parse().unwrap();
        let target = "zh-CN".parse().unwrap();
        let request = TranslationRequest::new(" Hello ".into(), source, target).unwrap();

        assert_eq!(request.text(), " Hello ");
        assert_eq!(request.source().as_str(), "en");
        assert_eq!(request.target().as_str(), "zh-CN");
    }

    #[test]
    fn rejects_whitespace_only_request_text() {
        let result = TranslationRequest::new(
            " \t\n ".into(),
            "en".parse().unwrap(),
            "ja".parse().unwrap(),
        );
        assert_eq!(result, Err(ValidationError::EmptyText));
    }

    #[test]
    fn rejects_equal_source_and_target_languages() {
        let result =
            TranslationRequest::new("hello".into(), "en".parse().unwrap(), "en".parse().unwrap());
        assert_eq!(result, Err(ValidationError::IdenticalLanguages));
    }

    #[test]
    fn rejects_invalid_deserialized_request() {
        let json = r#"{"text":"   ","source":"en","target":"ja"}"#;
        assert!(serde_json::from_str::<TranslationRequest>(json).is_err());
    }

    fn provider_config_json(endpoint: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "example",
            "display_name": "Example",
            "endpoint": endpoint,
            "model": "basic",
            "secret_account": "example-keyring-account",
            "is_local": false,
            "enabled": true
        })
    }

    #[test]
    fn rejects_secret_bearing_endpoints_during_deserialization() {
        for endpoint in [
            "https://user@example.com/translate",
            "https://user:password@example.com/translate",
            "https://example.com/translate?api_key=secret",
            "https://example.com/translate#secret",
        ] {
            let value = provider_config_json(endpoint);
            let error = serde_json::from_value::<ProviderConfig>(value).unwrap_err();
            assert!(!format!("{error:?}").contains(endpoint));
        }
    }

    #[test]
    fn accepts_https_and_loopback_http_endpoints_without_changing_settings_shape() {
        for endpoint in [
            "https://example.com/translate",
            "http://localhost:8080/translate",
            "http://127.0.0.1:8080/translate",
            "http://[::1]:8080/translate",
        ] {
            let value = provider_config_json(endpoint);
            let config: ProviderConfig = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(config.endpoint().unwrap().as_str(), endpoint);
            assert_eq!(config.secret_account(), Some("example-keyring-account"));
            assert_eq!(serde_json::to_value(config).unwrap(), value);
        }
    }

    #[test]
    fn rejects_secret_bearing_endpoints_during_direct_construction() {
        for endpoint in [
            "https://user:password@example.com/translate",
            "https://example.com/translate?api_key=secret",
            "https://example.com/translate#secret",
        ] {
            let result = ProviderConfig::new(
                ProviderId::new("example"),
                "Example".into(),
                Some(endpoint.parse().unwrap()),
                Some("basic".into()),
                Some("example-keyring-account".into()),
                false,
                true,
            );
            assert_eq!(
                result.unwrap_err(),
                ValidationError::InvalidProviderEndpoint
            );
        }
    }

    #[test]
    fn config_debug_does_not_expose_endpoint_content() {
        let config: ProviderConfig = serde_json::from_value(provider_config_json(
            "https://example.com/secret-path-marker",
        ))
        .unwrap();
        let debug = format!("{config:?}");
        assert!(!debug.contains("secret-path-marker"));
    }

    struct FakeProvider {
        outcome: Result<TranslationResult, ProviderError>,
    }

    #[async_trait::async_trait]
    impl TranslationProvider for FakeProvider {
        fn id(&self) -> &'static str {
            "fake"
        }

        async fn translate(
            &self,
            _request: &TranslationRequest,
        ) -> Result<TranslationResult, ProviderError> {
            self.outcome.clone()
        }
    }

    fn request() -> TranslationRequest {
        TranslationRequest::new("hello".into(), "en".parse().unwrap(), "ja".parse().unwrap())
            .unwrap()
    }

    #[test]
    fn provider_contract_supports_trait_objects_and_fixed_result() {
        let provider: Arc<dyn TranslationProvider> = Arc::new(FakeProvider {
            outcome: Ok(TranslationResult {
                translated_text: "こんにちは".into(),
                provider_id: "fake".into(),
                elapsed_ms: 12,
                character_count: 5,
            }),
        });

        let result = tauri::async_runtime::block_on(provider.translate(&request())).unwrap();
        assert_eq!(provider.id(), result.provider_id);
        assert_eq!(result.translated_text, "こんにちは");
        assert_eq!(result.character_count, 5);
    }

    #[test]
    fn provider_errors_propagate_unchanged() {
        let errors = [
            ProviderError::Authentication,
            ProviderError::RateLimited {
                retry_after_seconds: None,
            },
            ProviderError::RateLimited {
                retry_after_seconds: Some(30),
            },
            ProviderError::Network,
            ProviderError::Timeout,
            ProviderError::Service { status: None },
            ProviderError::Service { status: Some(503) },
            ProviderError::InvalidResponse,
            ProviderError::UnsupportedLanguage,
            ProviderError::Configuration,
        ];

        for error in errors {
            let provider: Arc<dyn TranslationProvider> = Arc::new(FakeProvider {
                outcome: Err(error.clone()),
            });
            let result = tauri::async_runtime::block_on(provider.translate(&request()));
            assert_eq!(result, Err(error));
        }
    }
}
