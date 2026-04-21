// Integration stub tests for translation adapter
// Tests live integration with the translation module from outside the crate

#[cfg(test)]
mod tests {
    use overlex_lib::translation::TranslationEngine;
    use overlex_lib::translation::LibreTranslateAdapter;

    #[test]
    fn test_adapter_creation() {
        let adapter = LibreTranslateAdapter::new(
            "https://libretranslate.com".to_string(),
            None,
        );
        assert_eq!(adapter.name(), "LibreTranslate");
        assert!(!adapter.requires_api_key());
    }

    #[test]
    fn test_adapter_with_api_key() {
        let adapter = LibreTranslateAdapter::new(
            "https://libretranslate.com".to_string(),
            Some("test-key".to_string()),
        );
        assert_eq!(adapter.name(), "LibreTranslate");
        assert!(adapter.requires_api_key());
    }
}