use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;

use super::{status_error, HttpTimeouts, HttpTransport};
use crate::domain::{
    ProviderConfig, ProviderError, TranslationProvider, TranslationRequest, TranslationResult,
};

pub struct MyMemory {
    transport: HttpTransport,
}

impl MyMemory {
    pub fn new(config: ProviderConfig, timeouts: HttpTimeouts) -> Result<Self, ProviderError> {
        Ok(Self {
            transport: HttpTransport::new(
                config,
                Some("https://api.mymemory.translated.net/get"),
                timeouts,
            )?,
        })
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(rename = "responseData")]
    data: Option<serde_json::Value>,
    #[serde(rename = "responseStatus")]
    status: u16,
}
#[derive(Deserialize)]
struct Data {
    #[serde(rename = "translatedText")]
    text: String,
}

#[async_trait]
impl TranslationProvider for MyMemory {
    fn id(&self) -> &'static str {
        "mymemory"
    }

    async fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationResult, ProviderError> {
        let start = Instant::now();
        if request.text().len() > 500 {
            return Err(ProviderError::Configuration);
        }
        let response: Response = self
            .transport
            .send(
                self.transport
                    .client
                    .get(self.transport.endpoint.clone())
                    .query(&[
                        ("q", request.text()),
                        (
                            "langpair",
                            &format!(
                                "{}|{}",
                                request.source().as_str(),
                                request.target().as_str()
                            ),
                        ),
                    ]),
            )
            .await?;
        if response.status != 200 {
            return Err(status_error(response.status, None));
        }
        let data: Data =
            serde_json::from_value(response.data.ok_or(ProviderError::InvalidResponse)?)
                .map_err(|_| ProviderError::InvalidResponse)?;
        self.transport.result(data.text, request, start)
    }
}
