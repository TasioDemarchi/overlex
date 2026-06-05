// TranslationChain — adaptive multi-engine fallback wrapper
// Implements TranslationEngine to provide:
//   primary → other enabled paid engines (in enabled_engines order) → google_gtx
// Excludes mymemory from the fallback chain (only used when set as primary).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use super::{TranslationContext, TranslationEngine, TranslationError, TranslationResult};
use super::{PAID_ENGINES};
use crate::app_log;

/// Wraps multiple engines in an adaptive fallback chain.
pub struct TranslationChain {
    /// The primary engine key (e.g., "gemini")
    pub primary: String,
    /// All enabled engines, keyed by engine key
    pub engines: HashMap<String, Arc<dyn TranslationEngine>>,
    /// Ordered list for fallback: primary → other paid enabled → google_gtx
    pub fallback_order: Vec<String>,
}

impl TranslationChain {
    /// Build a TranslationChain.
    ///
    /// `primary` — the primary engine key (e.g., "gemini").
    /// `engines` — all enabled engines, keyed by engine key.
    /// `enabled_engines` — ordered list of enabled engine keys from settings
    ///                      (preserves the user's configured order).
    pub fn new(
        primary: &str,
        engines: HashMap<String, Arc<dyn TranslationEngine>>,
        enabled_engines: &[String],
    ) -> Self {
        // Build fallback order:
        // 1. Primary first
        // 2. Other enabled paid engines (in enabled_engines order, excluding primary and mymemory)
        // 3. google_gtx if not already present
        let mut fallback_order: Vec<String> = vec![primary.to_string()];

        for engine_key in enabled_engines {
            if engine_key == primary || engine_key == "mymemory" {
                // Skip the primary (already first) and mymemory (not in fallback chain)
                continue;
            }
            // Only include paid engines in the fallback chain
            if PAID_ENGINES.contains(&engine_key.as_str()) || engine_key == "google_gtx" {
                if !fallback_order.contains(engine_key) && engines.contains_key(engine_key) {
                    fallback_order.push(engine_key.clone());
                }
            }
        }

        // Ensure google_gtx is always the last resort
        if !fallback_order.contains(&"google_gtx".to_string())
            && engines.contains_key("google_gtx")
        {
            fallback_order.push("google_gtx".to_string());
        }

        app_log!(
            "[CHAIN] Fallback order: primary={}, chain={:?}",
            primary, fallback_order
        );

        Self {
            primary: primary.to_string(),
            engines,
            fallback_order,
        }
    }
}

#[async_trait]
impl TranslationEngine for TranslationChain {
    async fn translate(
        &self,
        text: &str,
        source: &str,
        target: &str,
        context: Option<&TranslationContext>,
    ) -> Result<TranslationResult, TranslationError> {
        let mut last_error: Option<TranslationError> = None;

        for engine_key in &self.fallback_order {
            let engine = match self.engines.get(engine_key) {
                Some(e) => e,
                None => {
                    app_log!(
                        "[CHAIN] Engine '{}' not found in engines map, skipping",
                        engine_key
                    );
                    continue;
                }
            };

            let engine_name = engine.name().to_string();

            match engine.translate(text, source, target, context).await {
                Ok(mut result) => {
                    // Annotate with chain-level metadata
                    let is_fallback = engine_key != &self.primary;
                    result.engine_used = engine_name.clone();
                    result.fallback = is_fallback;

                    if is_fallback {
                        app_log!(
                            "[CHAIN] Fallback: primary '{}' failed, used '{}' instead",
                            self.primary, engine_name
                        );
                    } else {
                        app_log!(
                            "[CHAIN] Primary engine '{}' succeeded",
                            engine_name
                        );
                    }

                    return Ok(result);
                }
                Err(e) => {
                    app_log!(
                        "[CHAIN] Engine '{}' failed: {}",
                        engine_name, e
                    );
                    last_error = Some(e);
                    // Continue to next engine in fallback order
                }
            }
        }

        // All engines failed — return the last error
        app_log!(
            "[CHAIN] All engines in fallback chain failed. Returning last error."
        );
        Err(last_error.unwrap_or_else(|| {
            TranslationError::Network("No engines available in fallback chain".to_string())
        }))
    }

    fn name(&self) -> &str {
        // Return the name of the primary engine (the chain itself doesn't have a single name)
        // We look up the primary from the engines map
        if let Some(engine) = self.engines.get(&self.primary) {
            // We need to return a &str. Since engine.name() returns &str but is tied to
            // the engine's lifetime, and self outlives the borrow, this is fine.
            // Actually engine.name() borrows engine, which borrows self.engines.
            // We return &str tied to self — this works because Arc keeps the engine alive.
            engine.name()
        } else {
            "TranslationChain"
        }
    }

    fn requires_api_key(&self) -> bool {
        // The chain itself doesn't require a key — individual engines do.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translation::TranslationResult;

    /// A mock engine that always succeeds on the first call and fails on subsequent calls.
    struct MockEngine {
        name_str: &'static str,
        succeed: bool,
        requires_key: bool,
    }

    #[async_trait]
    impl TranslationEngine for MockEngine {
        async fn translate(
            &self,
            text: &str,
            _source: &str,
            _target: &str,
            _context: Option<&TranslationContext>,
        ) -> Result<TranslationResult, TranslationError> {
            if self.succeed {
                Ok(TranslationResult {
                    original: text.to_string(),
                    translated: format!("[{}] {}", self.name_str, text),
                    detected_source: None,
                    engine_used: self.name_str.to_string(),
                    fallback: false,
                })
            } else {
                Err(TranslationError::Network("mock failure".to_string()))
            }
        }

        fn name(&self) -> &str {
            self.name_str
        }

        fn requires_api_key(&self) -> bool {
            self.requires_key
        }
    }

    fn mock_engine(name: &'static str, succeed: bool, requires_key: bool) -> Arc<dyn TranslationEngine> {
        Arc::new(MockEngine {
            name_str: name,
            succeed,
            requires_key,
        })
    }

    #[tokio::test]
    async fn test_primary_succeeds() {
        let mut engines = HashMap::new();
        let google = mock_engine("Google Translate", true, false);
        let gemini = mock_engine("Gemini", true, true);
        engines.insert("google_gtx".to_string(), google);
        engines.insert("gemini".to_string(), gemini);

        let enabled: Vec<String> = vec!["google_gtx".to_string(), "gemini".to_string()];

        let chain = TranslationChain::new("gemini", engines, &enabled);

        let result = chain.translate("hello", "en", "es", None).await.unwrap();
        assert_eq!(result.engine_used, "Gemini");
        assert!(!result.fallback);
        assert_eq!(result.translated, "[Gemini] hello");
    }

    #[tokio::test]
    async fn test_primary_fails_fallback_succeeds() {
        let mut engines = HashMap::new();
        let google = mock_engine("Google Translate", true, false);
        let gemini = mock_engine("Gemini", false, true);
        let deepl = mock_engine("DeepL", true, true);
        engines.insert("google_gtx".to_string(), google);
        engines.insert("gemini".to_string(), gemini);
        engines.insert("deepl".to_string(), deepl);

        let enabled: Vec<String> = vec![
            "google_gtx".to_string(),
            "gemini".to_string(),
            "deepl".to_string(),
        ];

        let chain = TranslationChain::new("gemini", engines, &enabled);

        let result = chain.translate("hello", "en", "es", None).await.unwrap();
        assert_eq!(result.engine_used, "DeepL");
        assert!(result.fallback);
    }

    #[tokio::test]
    async fn test_all_paid_fail_google_gtx_succeeds() {
        let mut engines = HashMap::new();
        let google = mock_engine("Google Translate", true, false);
        let gemini = mock_engine("Gemini", false, true);
        let deepl = mock_engine("DeepL", false, true);
        engines.insert("google_gtx".to_string(), google);
        engines.insert("gemini".to_string(), gemini);
        engines.insert("deepl".to_string(), deepl);

        let enabled: Vec<String> = vec![
            "google_gtx".to_string(),
            "gemini".to_string(),
            "deepl".to_string(),
        ];

        let chain = TranslationChain::new("gemini", engines, &enabled);

        let result = chain.translate("hello", "en", "es", None).await.unwrap();
        assert_eq!(result.engine_used, "Google Translate");
        assert!(result.fallback);
    }

    #[tokio::test]
    async fn test_all_engines_fail_returns_error() {
        let mut engines = HashMap::new();
        let google = mock_engine("Google Translate", false, false);
        let gemini = mock_engine("Gemini", false, true);
        engines.insert("google_gtx".to_string(), google);
        engines.insert("gemini".to_string(), gemini);

        let enabled: Vec<String> = vec!["google_gtx".to_string(), "gemini".to_string()];

        let chain = TranslationChain::new("gemini", engines, &enabled);

        let result = chain.translate("hello", "en", "es", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fallback_order_preserves_enabled_engines_order() {
        let mut engines = HashMap::new();
        engines.insert("google_gtx".to_string(), mock_engine("Google Translate", true, false));
        engines.insert("gemini".to_string(), mock_engine("Gemini", false, true));
        engines.insert("deepl".to_string(), mock_engine("DeepL", false, true));
        engines.insert("deepseek".to_string(), mock_engine("DeepSeek", true, true));

        // User configured: google_gtx first, then deepseek, then deepl, then gemini
        let enabled: Vec<String> = vec![
            "google_gtx".to_string(),
            "deepseek".to_string(),
            "deepl".to_string(),
            "gemini".to_string(),
        ];

        let chain = TranslationChain::new("deepl", engines, &enabled);

        // Expected fallback order: deepl (primary) → deepseek (next enabled paid) → gemini (next enabled paid) → google_gtx (last resort)
        assert_eq!(
            chain.fallback_order,
            vec!["deepl", "deepseek", "gemini", "google_gtx"]
        );
    }

    #[tokio::test]
    async fn test_mymemory_only_as_primary() {
        let mut engines = HashMap::new();
        engines.insert("mymemory".to_string(), mock_engine("MyMemory", true, false));
        engines.insert("google_gtx".to_string(), mock_engine("Google Translate", true, false));

        let enabled: Vec<String> = vec!["google_gtx".to_string(), "mymemory".to_string()];

        let chain = TranslationChain::new("mymemory", engines, &enabled);

        // mymemory is primary, google_gtx is still in fallback as last resort
        assert_eq!(chain.fallback_order, vec!["mymemory", "google_gtx"]);

        let result = chain.translate("hello", "en", "es", None).await.unwrap();
        assert_eq!(result.engine_used, "MyMemory");
        assert!(!result.fallback);
    }

    #[tokio::test]
    async fn test_free_engines_only() {
        let mut engines = HashMap::new();
        engines.insert("google_gtx".to_string(), mock_engine("Google Translate", true, false));

        let enabled: Vec<String> = vec!["google_gtx".to_string()];

        let chain = TranslationChain::new("google_gtx", engines, &enabled);

        assert_eq!(chain.fallback_order, vec!["google_gtx"]);

        let result = chain.translate("hello", "en", "es", None).await.unwrap();
        assert_eq!(result.engine_used, "Google Translate");
        assert!(!result.fallback);
    }

    #[test]
    fn test_chain_name_returns_primary_engine_name() {
        let mut engines = HashMap::new();
        engines.insert("gemini".to_string(), mock_engine("Gemini", true, true));

        let enabled: Vec<String> = vec!["gemini".to_string()];

        let chain = TranslationChain::new("gemini", engines, &enabled);
        assert_eq!(chain.name(), "Gemini");
    }

    #[test]
    fn test_chain_requires_api_key_false() {
        let mut engines = HashMap::new();
        engines.insert("google_gtx".to_string(), mock_engine("Google Translate", true, false));

        let enabled: Vec<String> = vec!["google_gtx".to_string()];

        let chain = TranslationChain::new("google_gtx", engines, &enabled);
        assert!(!chain.requires_api_key());
    }
}