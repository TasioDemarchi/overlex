// Gemini adapter — Google Gemini 2.0 Flash API for context-aware translation
// POST https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={api_key}

use crate::app_log;
use crate::translation::{
    TranslationContext, TranslationEngine, TranslationError, TranslationResult,
};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

/// Gemini 2.0 Flash translation adapter
pub struct GeminiAdapter {
    api_key: Option<String>,
    client: Client,
}

impl GeminiAdapter {
    pub fn new(api_key: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");
        Self { api_key, client }
    }

    /// Convert language code to human-readable name
    fn language_name(code: &str) -> String {
        match code.to_lowercase().as_str() {
            "auto" => "auto-detect".to_string(),
            "en" => "English".to_string(),
            "es" => "Spanish".to_string(),
            "fr" => "French".to_string(),
            "de" => "German".to_string(),
            "it" => "Italian".to_string(),
            "pt" => "Portuguese".to_string(),
            "ru" => "Russian".to_string(),
            "ja" => "Japanese".to_string(),
            "ko" => "Korean".to_string(),
            "zh" => "Chinese".to_string(),
            "ar" => "Arabic".to_string(),
            "hi" => "Hindi".to_string(),
            "tr" => "Turkish".to_string(),
            "pl" => "Polish".to_string(),
            "nl" => "Dutch".to_string(),
            "sv" => "Swedish".to_string(),
            "da" => "Danish".to_string(),
            "fi" => "Finnish".to_string(),
            "no" => "Norwegian".to_string(),
            "cs" => "Czech".to_string(),
            "el" => "Greek".to_string(),
            "he" => "Hebrew".to_string(),
            "th" => "Thai".to_string(),
            "vi" => "Vietnamese".to_string(),
            "id" => "Indonesian".to_string(),
            "ms" => "Malay".to_string(),
            "uk" => "Ukrainian".to_string(),
            "hu" => "Hungarian".to_string(),
            "ro" => "Romanian".to_string(),
            "bg" => "Bulgarian".to_string(),
            "sk" => "Slovak".to_string(),
            _ => code.to_string(), // Return as-is if unknown
        }
    }

    /// Build the system instruction based on game context availability
    fn build_system_instruction(
        source: &str,
        target: &str,
        context: Option<&TranslationContext>,
    ) -> String {
        let source_name = Self::language_name(source);
        let target_name = Self::language_name(target);
        let lang_pair = format!("{} → {}", source_name, target_name);

        if let Some(ctx) = context {
            match (&ctx.process_name, &ctx.profile_name) {
                (Some(proc), Some(profile)) => {
                    format!(
                        "You are a professional game translator. Translate the following text from {}. Game context: {} (profile: {}). Only respond with the translated text, no explanations, no quotes, no markdown.",
                        lang_pair, proc, profile
                    )
                }
                (Some(proc), None) => {
                    format!(
                        "You are a professional game translator. Translate the following text from {}. Game context: {}. Only respond with the translated text, no explanations, no quotes, no markdown.",
                        lang_pair, proc
                    )
                }
                _ => {
                    format!(
                        "You are a professional translator. Translate the following text from {}. Only respond with the translated text, no explanations, no quotes, no markdown.",
                        lang_pair
                    )
                }
            }
        } else {
            format!(
                "You are a professional translator. Translate the following text from {}. Only respond with the translated text, no explanations, no quotes, no markdown.",
                lang_pair
            )
        }
    }
}

#[async_trait]
impl TranslationEngine for GeminiAdapter {
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

        let system_instruction = Self::build_system_instruction(source, target, context);

        let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";

        let body = json!({
            "systemInstruction": {
                "parts": [{"text": system_instruction}]
            },
            "contents": [{
                "parts": [{"text": text}]
            }],
            "generationConfig": {
                "temperature": 0.1
            }
        });

        let response = self
            .client
            .post(url)
            .header("x-goog-api-key", api_key)
            .json(&body)
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
            let status_code = status.as_u16();
            let body_text = response.text().await.unwrap_or_default();

            app_log!("[GEMINI] API error HTTP {}: {}", status_code, &body_text[..body_text.len().min(200)]);

            // 400 with auth-related errors → InvalidApiKey
            if status_code == 400
                && (body_text.contains("API key")
                    || body_text.contains("api_key")
                    || body_text.contains("API_KEY_INVALID")
                    || body_text.contains("permission"))
            {
                return Err(TranslationError::InvalidApiKey);
            }

            // 403 — could be invalid key OR billing not enabled
            if status_code == 403 {
                // Check if it's a billing issue (specific Google error messages)
                if body_text.contains("billing")
                    || body_text.contains("BILLING")
                    || body_text.contains("quota")
                    || body_text.contains("QUOTA")
                    || body_text.contains("Cloud billing")
                    || body_text.contains("project billing")
                {
                    return Err(TranslationError::Network(format!(
                        "Gemini API billing required. Enable billing in your Google Cloud project. Details: {}",
                        &body_text[..body_text.len().min(300)]
                    )));
                }
                return Err(TranslationError::InvalidApiKey);
            }

            return Err(TranslationError::Network(format!(
                "HTTP {}: {}",
                status_code,
                &body_text[..body_text.len().min(300)]
            )));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| TranslationError::Network(format!("Failed to parse response: {}", e)))?;

        let translated = json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(TranslationResult {
            original: text.to_string(),
            translated: translated.trim().to_string(),
            detected_source: None,
        })
    }

    fn name(&self) -> &str {
        "Gemini"
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
        let adapter = GeminiAdapter::new(Some("test-key".to_string()));
        assert_eq!(adapter.name(), "Gemini");
        assert!(adapter.requires_api_key());
    }

    #[test]
    fn test_adapter_creation_no_key() {
        let adapter = GeminiAdapter::new(None);
        assert_eq!(adapter.name(), "Gemini");
        assert!(adapter.requires_api_key());
    }

    #[test]
    fn test_system_instruction_with_full_context() {
        let ctx = TranslationContext {
            process_name: Some("poe2.exe".to_string()),
            profile_name: Some("Path of Exile 2".to_string()),
        };
        let instruction = GeminiAdapter::build_system_instruction("en", "es", Some(&ctx));
        assert!(instruction.contains("Game context: poe2.exe"));
        assert!(instruction.contains("Path of Exile 2"));
        assert!(instruction.contains("professional game translator"));
    }

    #[test]
    fn test_system_instruction_with_process_only() {
        let ctx = TranslationContext {
            process_name: Some("eldenring.exe".to_string()),
            profile_name: None,
        };
        let instruction = GeminiAdapter::build_system_instruction("en", "ja", Some(&ctx));
        assert!(instruction.contains("Game context: eldenring.exe"));
        assert!(!instruction.contains("profile:"));
    }

    #[test]
    fn test_system_instruction_without_context() {
        let instruction = GeminiAdapter::build_system_instruction("auto", "es", None);
        assert!(!instruction.contains("Game context"));
        assert!(instruction.contains("professional translator"));
    }

    #[test]
    fn test_system_instruction_empty_context_fields() {
        let ctx = TranslationContext {
            process_name: None,
            profile_name: None,
        };
        let instruction = GeminiAdapter::build_system_instruction("en", "es", Some(&ctx));
        assert!(!instruction.contains("Game context"));
        assert!(instruction.contains("professional translator"));
    }
}
