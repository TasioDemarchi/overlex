// Translation module - translation engine trait and adapters

mod libretranslate;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use libretranslate::LibreTranslateAdapter;

/// Translation engine trait
#[async_trait]
pub trait TranslationEngine: Send + Sync {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<TranslationResult, TranslationError>;

    fn name(&self) -> &str;
    fn requires_api_key(&self) -> bool;
}

/// Translation result
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationResult {
    pub original: String,
    pub translated: String,
    pub detected_source: Option<String>,
}

/// Translation errors
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "detail")]
pub enum TranslationError {
    Network(String),
    RateLimit,
    InvalidApiKey,
    Timeout,
    ServiceDown(String),
}

impl std::fmt::Display for TranslationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationError::Network(msg) => write!(f, "Network error: {}", msg),
            TranslationError::RateLimit => write!(f, "Rate limit exceeded"),
            TranslationError::InvalidApiKey => write!(f, "Invalid API key"),
            TranslationError::Timeout => write!(f, "Request timed out"),
            TranslationError::ServiceDown(msg) => write!(f, "Service down: {}", msg),
        }
    }
}

impl std::error::Error for TranslationError {}