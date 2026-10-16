use std::{sync::Arc, time::Instant};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use super::{HttpTimeouts, HttpTransport};
use crate::{
    domain::{
        ProviderConfig, ProviderError, TranslationProvider, TranslationRequest, TranslationResult,
    },
    platform::secure_store::SecureStore,
};

pub struct DeepL {
    transport: HttpTransport,
    secure_store: Arc<dyn SecureStore>,
}

impl DeepL {
    pub fn new(
        config: ProviderConfig,
        secure_store: Arc<dyn SecureStore>,
        timeouts: HttpTimeouts,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            transport: HttpTransport::new(
                config,
                Some("https://api-free.deepl.com/v2/translate"),
                timeouts,
            )?,
            secure_store,
        })
    }
}

#[derive(Deserialize)]
struct Response {
    translations: Vec<Translation>,
}
#[derive(Deserialize)]
struct Translation {
    text: String,
}

fn language(code: &str, target: bool) -> String {
    let upper = code.to_ascii_uppercase();
    if target {
        match upper.as_str() {
            "ZH-CN" => return "ZH-HANS".into(),
            "ZH-TW" => return "ZH-HANT".into(),
            _ => {}
        }
    }
    if target
        && matches!(
            upper.as_str(),
            "EN-GB" | "EN-US" | "PT-BR" | "PT-PT" | "ZH-HANS" | "ZH-HANT"
        )
    {
        upper
    } else {
        upper.split('-').next().unwrap_or_default().into()
    }
}

#[async_trait]
impl TranslationProvider for DeepL {
    fn id(&self) -> &'static str {
        "deepl"
    }

    async fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, ProviderError> {
        let start = Instant::now();
        let key = self
            .transport
            .secret(self.secure_store.as_ref(), true)?
            .ok_or(ProviderError::Authentication)?;
        let mut auth = reqwest::header::HeaderValue::from_str(&format!("DeepL-Auth-Key {key}"))
            .map_err(|_| ProviderError::Authentication)?;
        auth.set_sensitive(true);
        let response: Response = self.transport.send(self.transport.client.post(self.transport.endpoint.clone()).header(reqwest::header::AUTHORIZATION, auth).json(&json!({
            "text": [request.text()], "source_lang": language(request.source().as_str(), false), "target_lang": language(request.target().as_str(), true)
        }))).await?;
        let text = response
            .translations
            .into_iter()
            .next()
            .ok_or(ProviderError::InvalidResponse)?
            .text;
        self.transport.result(text, request, start)
    }
}
