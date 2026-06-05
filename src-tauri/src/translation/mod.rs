// Translation module - translation engine trait and adapters

mod deepl;
mod gemini;
mod google_gtx;
mod libretranslate;
mod mymemory;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use deepl::DeepLAdapter;
pub use gemini::GeminiAdapter;
pub use google_gtx::GoogleGtxAdapter;
pub use libretranslate::LibreTranslateAdapter;
pub use mymemory::MyMemoryAdapter;

use crate::commands::Settings;
use crate::app_log;

/// Game context passed to translation engines for domain-aware translations
#[derive(Debug, Clone)]
pub struct TranslationContext {
    pub process_name: Option<String>,
    pub profile_name: Option<String>,
}

/// Translation engine trait
#[async_trait]
pub trait TranslationEngine: Send + Sync {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
        context: Option<&TranslationContext>,
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
/// Supports: google_gtx (default, free), mymemory (free), libretranslate,
/// gemini (requires API key), deepl (requires API key).
pub fn create_engine(settings: &Settings) -> Box<dyn TranslationEngine> {
    match settings.engine.as_str() {
        "gemini" => {
            let api_key = crate::settings::get_api_key("gemini").ok();
            app_log!(
                "[ENGINE] Using Gemini 2.0 Flash (API key: {})",
                match &api_key {
                    Some(k) => {
                        // Log key prefix and length for debugging (never log full key)
                        format!("present ({} chars, starts with {}...)", k.len(), &k[..k.len().min(8)])
                    }
                    None => "NOT FOUND — save the API key in Settings first".to_string(),
                }
            );
            Box::new(GeminiAdapter::new(api_key))
        }
        "deepl" => {
            let api_key = crate::settings::get_api_key("deepl").ok();
            app_log!(
                "[ENGINE] Using DeepL Free (API key: {})",
                match &api_key {
                    Some(k) => format!("present ({} chars)", k.len()),
                    None => "NOT FOUND".to_string(),
                }
            );
            Box::new(DeepLAdapter::new(api_key))
        }
        "mymemory" => {
            app_log!("[ENGINE] Using MyMemory (free, no API key)");
            Box::new(MyMemoryAdapter::new())
        }
        "libretranslate" => {
            let api_key = crate::settings::get_api_key("libretranslate").ok();
            app_log!(
                "[ENGINE] Using LibreTranslate at {}",
                settings.libre_translate_url
            );
            Box::new(LibreTranslateAdapter::new(
                settings.libre_translate_url.clone(),
                api_key,
            ))
        }
        "google_gtx" | _ => {
            app_log!("[ENGINE] Using Google GTX (free, no API key)");
            Box::new(GoogleGtxAdapter::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Settings;

    #[test]
    fn test_translation_context_none() {
        let ctx = TranslationContext {
            process_name: None,
            profile_name: None,
        };
        assert!(ctx.process_name.is_none());
        assert!(ctx.profile_name.is_none());
    }

    #[test]
    fn test_translation_context_with_game() {
        let ctx = TranslationContext {
            process_name: Some("poe2.exe".to_string()),
            profile_name: Some("Path of Exile 2".to_string()),
        };
        assert_eq!(ctx.process_name.as_deref(), Some("poe2.exe"));
        assert_eq!(ctx.profile_name.as_deref(), Some("Path of Exile 2"));
    }

    #[test]
    fn test_create_engine_default_fallback() {
        let settings = Settings {
            engine: "unknown_engine".to_string(),
            ..Settings::default()
        };
        let engine = create_engine(&settings);
        assert_eq!(engine.name(), "Google Translate");
        assert!(!engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_google_gtx() {
        let settings = Settings {
            engine: "google_gtx".to_string(),
            ..Settings::default()
        };
        let engine = create_engine(&settings);
        assert_eq!(engine.name(), "Google Translate");
        assert!(!engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_mymemory() {
        let settings = Settings {
            engine: "mymemory".to_string(),
            ..Settings::default()
        };
        let engine = create_engine(&settings);
        assert_eq!(engine.name(), "MyMemory");
        assert!(!engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_gemini() {
        let settings = Settings {
            engine: "gemini".to_string(),
            ..Settings::default()
        };
        let engine = create_engine(&settings);
        assert_eq!(engine.name(), "Gemini");
        assert!(engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_deepl() {
        let settings = Settings {
            engine: "deepl".to_string(),
            ..Settings::default()
        };
        let engine = create_engine(&settings);
        assert_eq!(engine.name(), "DeepL");
        assert!(engine.requires_api_key());
    }
}
