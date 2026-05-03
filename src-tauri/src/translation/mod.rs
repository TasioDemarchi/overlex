// Translation module - translation engine trait and adapters

mod google_gtx;
mod libretranslate;
mod mymemory;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use google_gtx::GoogleGtxAdapter;
pub use libretranslate::LibreTranslateAdapter;
pub use mymemory::MyMemoryAdapter;

use crate::commands::Settings;

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

/// Create a translation engine based on settings.
/// Supports: google_gtx (default, free), mymemory (free), libretranslate (free, self-hosted or public).
/// All supported engines are free and require NO registration.
pub fn create_engine(settings: &Settings) -> Box<dyn TranslationEngine> {
    match settings.engine.as_str() {
        "mymemory" => {
            eprintln!("[ENGINE] Using MyMemory (free, no API key)");
            Box::new(MyMemoryAdapter::new())
        }
        "libretranslate" => {
            eprintln!("[ENGINE] Using LibreTranslate at {}", settings.libre_translate_url);
            Box::new(LibreTranslateAdapter::new(
                settings.libre_translate_url.clone(),
                None, // No API key needed for public instances
            ))
        }
        "google_gtx" | _ => {
            eprintln!("[ENGINE] Using Google GTX (free, no API key)");
            Box::new(GoogleGtxAdapter::new())
        }
    }
}