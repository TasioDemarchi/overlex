// MyMemory adapter — free translation API (no registration, no API key required)
// https://api.mymemory.translated.net
// Free tier: 5000 chars/day without email, 50000 chars/day with email
// No API key needed for basic usage

use crate::translation::{TranslationEngine, TranslationError, TranslationResult};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

/// MyMemory translation adapter — free, no API key required
pub struct MyMemoryAdapter {
    client: Client,
    email: Option<String>,
}

impl MyMemoryAdapter {
    /// Create a new MyMemory adapter without email (5000 chars/day limit)
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, email: None }
    }

    /// Create a new MyMemory adapter with email (50000 chars/day limit, still free)
    pub fn with_email(email: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, email: Some(email) }
    }
}

impl Default for MyMemoryAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from MyMemory API
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct MyMemoryResponse {
    responseData: ResponseData,
    responseStatus: i32,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct ResponseData {
    translatedText: String,
}

// MyMemory sometimes returns a plain error message in responseData
// when the response status is not 200
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct MyMemoryErrorResponse {
    responseData: ErrorData,
    responseStatus: i32,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct ErrorData {
    translatedText: String,
}

#[async_trait]
impl TranslationEngine for MyMemoryAdapter {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<TranslationResult, TranslationError> {
        let lang_pair = format!("{}|{}", source, target);

        let mut request = self.client
            .get("https://api.mymemory.translated.net/get")
            .query(&[
                ("q", text),
                ("langpair", &lang_pair),
            ]);

        // Add email if configured for higher rate limit
        if let Some(ref email) = self.email {
            request = request.query(&[("de", email.as_str())]);
        }

        let response = request
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

        let body = response.text().await
            .map_err(|e| TranslationError::Network(format!("Failed to read response: {}", e)))?;

        // Try to parse as success response first
        let parsed: Result<MyMemoryResponse, _> = serde_json::from_str(&body);

        match parsed {
            Ok(result) => {
                if result.responseStatus == 200 {
                    let translated = result.responseData.translatedText;

                    // MyMemory sometimes returns all-uppercase when it can't translate
                    // Check if the "translation" is just the original text in uppercase
                    let translated_clean = if translated.trim().to_uppercase() == text.trim().to_uppercase() {
                        // Likely a failed translation — return as-is, let the user decide
                        translated
                    } else {
                        translated
                    };

                    Ok(TranslationResult {
                        original: text.to_string(),
                        translated: translated_clean,
                        detected_source: None,
                    })
                } else {
                    Err(TranslationError::Network(format!(
                        "MyMemory returned status {}: {}",
                        result.responseStatus,
                        result.responseData.translatedText
                    )))
                }
            }
            Err(_) => {
                // Try parsing as error response
                let err_parsed: Result<MyMemoryErrorResponse, _> = serde_json::from_str(&body);
                match err_parsed {
                    Ok(err_result) => {
                        Err(TranslationError::Network(format!(
                            "MyMemory error (status {}): {}",
                            err_result.responseStatus,
                            err_result.responseData.translatedText
                        )))
                    }
                    Err(_) => {
                        Err(TranslationError::Network(format!(
                            "Failed to parse MyMemory response: {}",
                            &body[..body.len().min(200)]
                        )))
                    }
                }
            }
        }
    }

    fn name(&self) -> &str {
        "MyMemory"
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
        let adapter = MyMemoryAdapter::new();
        assert_eq!(adapter.name(), "MyMemory");
        assert!(!adapter.requires_api_key());
    }

    #[test]
    fn test_adapter_with_email() {
        let adapter = MyMemoryAdapter::with_email("test@example.com".to_string());
        assert_eq!(adapter.name(), "MyMemory");
        assert!(!adapter.requires_api_key());
    }
}