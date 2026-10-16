mod domain_tests {
    use std::sync::Arc;

    use quick_translate_lib::domain::{
        LanguageCode, ProviderError, TranslationProvider, TranslationRequest, TranslationResult,
        ValidationError,
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

        assert_eq!(request.text, " Hello ");
        assert_eq!(request.source.as_str(), "en");
        assert_eq!(request.target.as_str(), "zh-CN");
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
