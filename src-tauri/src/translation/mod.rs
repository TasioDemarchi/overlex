// Translation module - translation engine trait and adapters

pub mod chain;
mod deepl;
mod deepseek;
mod gemini;
mod groq;
mod google_gtx;
mod mymemory;

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use chain::TranslationChain;
pub use deepl::DeepLAdapter;
pub use deepseek::DeepSeekAdapter;
pub use gemini::GeminiAdapter;
pub use groq::GroqAdapter;
pub use google_gtx::GoogleGtxAdapter;
pub use mymemory::MyMemoryAdapter;

use crate::commands::Settings;
use crate::app_log;

/// Engine classification constants
pub const PAID_ENGINES: &[&str] = &["gemini", "deepl", "deepseek", "groq"];
pub const FREE_ENGINES: &[&str] = &["google_gtx", "mymemory"];
pub const ALL_ENGINES: &[&str] = &["google_gtx", "mymemory", "gemini", "deepl", "deepseek", "groq"];

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
        context_prompt: Option<&str>,
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
    /// The name() of the engine that actually performed the translation
    pub engine_used: String,
    /// True if a fallback engine was used (primary engine failed)
    pub fallback: bool,
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

/// Internal helper: create a single translation engine.
/// When `api_key_override` is provided, it is used instead of reading from
/// the credential store. This allows `save_settings` to pass the key the
/// user just typed, avoiding a race condition with Credential Manager.
fn create_engine_internal(engine_key: &str, api_key_override: Option<&str>) -> Box<dyn TranslationEngine> {
    match engine_key {
        "gemini" => {
            let api_key = api_key_override
                .map(|s| s.to_string())
                .or_else(|| crate::settings::get_api_key("gemini").ok());
            app_log!(
                "[ENGINE] Creating Gemini 2.5 Flash engine (API key: {})",
                match &api_key {
                    Some(k) => {
                        format!("present ({} chars, starts with {}...)", k.len(), &k[..k.len().min(8)])
                    }
                    None => "NOT FOUND — save the API key in Settings first".to_string(),
                }
            );
            Box::new(GeminiAdapter::new(api_key))
        }
        "deepl" => {
            let api_key = api_key_override
                .map(|s| s.to_string())
                .or_else(|| crate::settings::get_api_key("deepl").ok());
            app_log!(
                "[ENGINE] Using DeepL Free (API key: {})",
                match &api_key {
                    Some(k) => format!("present ({} chars)", k.len()),
                    None => "NOT FOUND".to_string(),
                }
            );
            Box::new(DeepLAdapter::new(api_key))
        }
        "deepseek" => {
            let api_key = api_key_override
                .map(|s| s.to_string())
                .or_else(|| crate::settings::get_api_key("deepseek").ok());
            app_log!(
                "[ENGINE] Creating DeepSeek v4 Flash engine (API key: {})",
                match &api_key {
                    Some(k) => format!("present ({} chars, starts with {}...)", k.len(), &k[..k.len().min(8)]),
                    None => "NOT FOUND — save the API key in Settings first".to_string(),
                }
            );
            Box::new(DeepSeekAdapter::new(api_key))
        }
        "groq" => {
            let api_key = api_key_override
                .map(|s| s.to_string())
                .or_else(|| crate::settings::get_api_key("groq").ok());
            app_log!(
                "[ENGINE] Creating Groq Llama 3.1 8B Instant engine (API key: {})",
                match &api_key {
                    Some(k) => format!("present ({} chars, starts with {}...)", k.len(), &k[..k.len().min(8)]),
                    None => "NOT FOUND — save the API key in Settings first".to_string(),
                }
            );
            Box::new(GroqAdapter::new(api_key))
        }
        "mymemory" => {
            app_log!("[ENGINE] Using MyMemory (free, no API key)");
            Box::new(MyMemoryAdapter::new())
        }
        "google_gtx" | _ => {
            app_log!("[ENGINE] Using Google GTX (free, no API key)");
            Box::new(GoogleGtxAdapter::new())
        }
    }
}

/// Create a translation engine based on settings (backward-compatible wrapper).
/// Supports: google_gtx (default, free), mymemory (free),
/// gemini (requires API key), deepl (requires API key), deepseek (requires API key), groq (requires API key).
pub fn create_engine(settings: &Settings, api_key_override: Option<String>) -> Box<dyn TranslationEngine> {
    create_engine_internal(&settings.primary_engine, api_key_override.as_deref())
}

/// Create all enabled engines at once, returning a HashMap keyed by engine key.
/// Iterates `enabled_engines` in order and creates the corresponding adapter.
/// When an API key is present in the map, it's passed directly; otherwise falls
/// back to the system credential manager.
pub fn create_all_engines(
    enabled_engines: &[String],
    api_keys: &HashMap<String, String>,
) -> HashMap<String, Arc<dyn TranslationEngine>> {
    let mut engines: HashMap<String, Arc<dyn TranslationEngine>> = HashMap::new();

    for engine_key in enabled_engines {
        let key_override = api_keys.get(engine_key);
        let engine = create_engine_internal(engine_key, key_override.map(|s| s.as_str()));
        let name = engine.name().to_string();
        let requires_key = engine.requires_api_key();

        app_log!(
            "[ENGINE] Registered engine '{}' (requires_key={})",
            name, requires_key
        );

        engines.insert(engine_key.clone(), Arc::from(engine));
    }

    app_log!("[ENGINE] Created {} engines total", engines.len());
    engines
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
        let mut settings = Settings::default();
        settings.primary_engine = "unknown_engine".to_string();
        let engine = create_engine(&settings, None);
        assert_eq!(engine.name(), "Google Translate");
        assert!(!engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_google_gtx() {
        let mut settings = Settings::default();
        settings.primary_engine = "google_gtx".to_string();
        let engine = create_engine(&settings, None);
        assert_eq!(engine.name(), "Google Translate");
        assert!(!engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_mymemory() {
        let mut settings = Settings::default();
        settings.primary_engine = "mymemory".to_string();
        let engine = create_engine(&settings, None);
        assert_eq!(engine.name(), "MyMemory");
        assert!(!engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_gemini() {
        let mut settings = Settings::default();
        settings.primary_engine = "gemini".to_string();
        let engine = create_engine(&settings, None);
        assert_eq!(engine.name(), "Gemini");
        assert!(engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_deepl() {
        let mut settings = Settings::default();
        settings.primary_engine = "deepl".to_string();
        let engine = create_engine(&settings, None);
        assert_eq!(engine.name(), "DeepL");
        assert!(engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_deepseek() {
        let mut settings = Settings::default();
        settings.primary_engine = "deepseek".to_string();
        let engine = create_engine(&settings, None);
        assert_eq!(engine.name(), "DeepSeek");
        assert!(engine.requires_api_key());
    }

    #[test]
    fn test_create_engine_groq() {
        let mut settings = Settings::default();
        settings.primary_engine = "groq".to_string();
        let engine = create_engine(&settings, None);
        assert_eq!(engine.name(), "Groq");
        assert!(engine.requires_api_key());
    }
}
