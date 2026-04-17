// LibreTranslate adapter - default translation engine
// TODO: Implement HTTP calls to LibreTranslate API

use crate::translation::{TranslationEngine, TranslationError, TranslationResult};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// LibreTranslate adapter
pub struct LibreTranslateAdapter {
    base_url: String,
    api_key: Option<String>,
    client: Client,
}

impl LibreTranslateAdapter {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { base_url, api_key, client }
    }
}

/// Request body for LibreTranslate API
#[derive(Serialize)]
struct TranslateRequest<'a> {
    q: &'a str,
    source: &'a str,
    target: &'a str,
    format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<&'a str>,
}

/// Response from LibreTranslate API
#[derive(Deserialize)]
struct TranslateResponse {
    translated_text: String,
    detected_language: Option<DetectedLanguage>,
}

#[derive(Deserialize)]
struct DetectedLanguage {
    language: String,
}

#[async_trait]
impl TranslationEngine for LibreTranslateAdapter {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<TranslationResult, TranslationError> {
        let url = format!("{}/translate", self.base_url);

        let request = TranslateRequest {
            q: text,
            source,
            target,
            format: "text",
            api_key: self.api_key.as_deref(),
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    TranslationError::Timeout
                } else {
                    TranslationError::Network(e.to_string())
                }
            })?;

        let status = response.status();

        // Handle HTTP error status codes
        if status.as_u16() == 429 {
            return Err(TranslationError::RateLimit);
        }
        if status.as_u16() == 403 {
            return Err(TranslationError::InvalidApiKey);
        }
        if status.is_server_error() {
            let body = response.text().await.unwrap_or_default();
            return Err(TranslationError::ServiceDown(body));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(TranslationError::Network(format!("HTTP {}: {}", status, body)));
        }

        // Parse response JSON
        let result: TranslateResponse = response
            .json()
            .await
            .map_err(|e| TranslationError::Network(format!("Failed to parse response: {}", e)))?;

        let detected_source = result.detected_language.map(|d| d.language);

        Ok(TranslationResult {
            original: text.to_string(),
            translated: result.translated_text,
            detected_source,
        })
    }

    fn name(&self) -> &str {
        "LibreTranslate"
    }

    fn requires_api_key(&self) -> bool {
        self.api_key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = LibreTranslateAdapter::new(
            "https://libretranslate.com".to_string(),
            None,
        );
        assert_eq!(adapter.name(), "LibreTranslate");
    }
}