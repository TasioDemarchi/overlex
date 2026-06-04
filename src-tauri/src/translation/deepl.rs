// DeepL adapter — DeepL Free API for context-aware translation
// POST https://api-free.deepl.com/v2/translate
// Auth: Authorization: DeepL-Auth-Key {api_key}

use crate::translation::{
    TranslationContext, TranslationEngine, TranslationError, TranslationResult,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// DeepL Free translation adapter
pub struct DeepLAdapter {
    api_key: Option<String>,
    client: Client,
}

impl DeepLAdapter {
    pub fn new(api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");
        Self { api_key, client }
    }
}

/// Request body for DeepL API
#[derive(Serialize)]
struct DeepLRequest<'a> {
    text: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lang: Option<&'a str>,
    target_lang: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
}

/// Response from DeepL API
#[derive(Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Deserialize)]
struct DeepLTranslation {
    text: String,
    #[serde(rename = "detected_source_language")]
    detected_source_language: Option<String>,
}

#[async_trait]
impl TranslationEngine for DeepLAdapter {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
        context: Option<&TranslationContext>,
    ) -> Result<TranslationResult, TranslationError> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or(TranslationError::InvalidApiKey)?;

        // Build source_lang: DeepL expects uppercase language codes, skip for "auto"
        let source_lang = if source == "auto" || source.is_empty() {
            None
        } else {
            Some(source.to_uppercase())
        };

        let target_lang = target.to_uppercase();

        // Build context string if game context is available
        let context_str = context.and_then(|ctx| {
            ctx.process_name
                .as_ref()
                .map(|proc| format!("Game: {}", proc))
        });

        let request = DeepLRequest {
            text: vec![text],
            source_lang: source_lang.as_deref(),
            target_lang: &target_lang,
            context: context_str,
        };

        let response = self
            .client
            .post("https://api-free.deepl.com/v2/translate")
            .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
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

        // 429 and 456 both indicate rate limiting / quota exceeded
        if status.as_u16() == 429 || status.as_u16() == 456 {
            return Err(TranslationError::RateLimit);
        }

        // 403 → invalid API key
        if status.as_u16() == 403 {
            return Err(TranslationError::InvalidApiKey);
        }

        if status.is_server_error() {
            let body = response.text().await.unwrap_or_default();
            return Err(TranslationError::ServiceDown(body));
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(TranslationError::Network(format!(
                "HTTP {}: {}",
                status,
                &body[..body.len().min(300)]
            )));
        }

        let result: DeepLResponse = response
            .json()
            .await
            .map_err(|e| TranslationError::Network(format!("Failed to parse response: {}", e)))?;

        let translation = result
            .translations
            .first()
            .ok_or_else(|| TranslationError::Network("Empty translation response".to_string()))?;

        let detected_source = translation.detected_source_language.clone();

        Ok(TranslationResult {
            original: text.to_string(),
            translated: translation.text.clone(),
            detected_source,
        })
    }

    fn name(&self) -> &str {
        "DeepL"
    }

    fn requires_api_key(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = DeepLAdapter::new(Some("test-key".to_string()));
        assert_eq!(adapter.name(), "DeepL");
        assert!(adapter.requires_api_key());
    }

    #[test]
    fn test_adapter_creation_no_key() {
        let adapter = DeepLAdapter::new(None);
        assert_eq!(adapter.name(), "DeepL");
        assert!(adapter.requires_api_key());
    }
}
