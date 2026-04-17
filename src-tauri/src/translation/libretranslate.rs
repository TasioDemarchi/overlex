// LibreTranslate adapter - default translation engine
// TODO: Implement HTTP calls to LibreTranslate API

use crate::translation::{TranslationEngine, TranslationError, TranslationResult};
use async_trait::async_trait;

/// LibreTranslate adapter
pub struct LibreTranslateAdapter {
    base_url: String,
    api_key: Option<String>,
}

impl LibreTranslateAdapter {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self { base_url, api_key }
    }
}

#[async_trait]
impl TranslationEngine for LibreTranslateAdapter {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<TranslationResult, TranslationError> {
        // TODO: Use reqwest to POST to {base_url}/translate
        // Request: { q: text, source, target, format: "text" }
        // Response: { translatedText, detectedLanguage }
        let _ = (text, source, target);
        Err(TranslationError::Network("Not implemented".to_string()))
    }

    fn name(&self) -> &str {
        "LibreTranslate"
    }

    fn requires_api_key(&self) -> bool {
        false
    }
}