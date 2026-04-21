// Google GTX adapter - unofficial free Google Translate API
// Uses the client=gtx endpoint which is free and doesn't require an API key

use crate::translation::{TranslationEngine, TranslationError, TranslationResult};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

/// Google GTX adapter - uses the free gtx client endpoint
pub struct GoogleGtxAdapter {
    client: Client,
}

impl GoogleGtxAdapter {
    /// Create a new Google GTX adapter
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { client }
    }
}

impl Default for GoogleGtxAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TranslationEngine for GoogleGtxAdapter {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<TranslationResult, TranslationError> {
        // Build the request with properly URL-encoded query parameters
        let response = self.client
            .get("https://translate.googleapis.com/translate_a/single")
            .query(&[
                ("client", "gtx"),
                ("sl", source),
                ("tl", target),
                ("dt", "t"),
                ("q", text),
            ])
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
        if status.is_server_error() {
            let body = response.text().await.unwrap_or_default();
            return Err(TranslationError::ServiceDown(body));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(TranslationError::Network(format!("HTTP {}: {}", status, body)));
        }

        // Parse response JSON - it's a nested array structure
        // Response format: [[["translated","original",null,null,score]],null,"detected_lang"]
        let json: Value = response
            .json()
            .await
            .map_err(|e| TranslationError::Network(format!("Failed to parse response: {}", e)))?;

        // Extract translated text — response[0] is an array of segments,
        // each segment is [translated_chunk, original_chunk, ...].
        // Must concatenate ALL segments to get the full translation.
        let segments = json
            .get(0)
            .and_then(|v| v.as_array())
            .ok_or_else(|| TranslationError::Network("Failed to parse translation segments".to_string()))?;

        let translated: String = segments
            .iter()
            .filter_map(|seg| seg.get(0).and_then(|v| v.as_str()))
            .collect::<Vec<&str>>()
            .join("");

        // Extract detected language from response[2]
        let detected_source = json
            .get(2)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(TranslationResult {
            original: text.to_string(),
            translated,
            detected_source,
        })
    }

    fn name(&self) -> &str {
        "Google Translate"
    }

    fn requires_api_key(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = GoogleGtxAdapter::new();
        assert_eq!(adapter.name(), "Google Translate");
        assert!(!adapter.requires_api_key());
    }
}