// Settings module - load/save settings.json

use std::path::PathBuf;
use keyring::Entry;
use crate::commands::Settings;
use crate::app_log;

/// Get the settings file path: %APPDATA%/overlex/settings.json
pub fn settings_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
    Ok(PathBuf::from(appdata).join("overlex").join("settings.json"))
}

/// Load settings from disk. On corrupt JSON: backup to .bak and return defaults.
pub fn load_settings() -> Settings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(_) => return Settings::default(),
    };

    if !path.exists() {
        // First run — save defaults
        let mut defaults = Settings::default();
        normalize_settings(&mut defaults);
        let _ = save_settings_to_disk(&defaults);
        return defaults;
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str::<Settings>(&content) {
                Ok(mut settings) => {
                    normalize_settings(&mut settings);
                    settings
                }
                Err(e) => {
                    // Corrupt — backup and reset
                    app_log!("Corrupt settings.json: {e}. Backing up and resetting.");
                    let bak = path.with_extension("json.bak");
                    let _ = std::fs::rename(&path, &bak);
                    let mut defaults = Settings::default();
                    normalize_settings(&mut defaults);
                    let _ = save_settings_to_disk(&defaults);
                    defaults
                }
            }
        }
        Err(_) => Settings::default(),
    }
}

/// Save settings to disk. Creates parent directory if needed.
pub fn save_settings_to_disk(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create settings dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| format!("Serialize failed: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Write failed: {e}"))?;
    Ok(())
}

/// Validate that OCR and Write hotkeys don't conflict.
pub fn validate_hotkeys(settings: &Settings) -> Result<(), String> {
    if settings.ocr_hotkey.trim().is_empty() || settings.write_hotkey.trim().is_empty() {
        return Err("Hotkeys cannot be empty".to_string());
    }
    if settings.ocr_hotkey.to_uppercase() == settings.write_hotkey.to_uppercase() {
        return Err("OCR and Write hotkeys cannot be the same".to_string());
    }
    // Validate both parse correctly
    crate::hotkeys::parse_hotkey(&settings.ocr_hotkey)?;
    crate::hotkeys::parse_hotkey(&settings.write_hotkey)?;
    Ok(())
}

/// Get API key from Windows Credential Manager (DPAPI-protected)
pub fn get_api_key(engine: &str) -> Result<String, String> {
    let service = format!("overlex-{engine}");
    let entry = Entry::new(&service, "overlex").map_err(|e| format!("Credential error: {e}"))?;
    entry.get_password().map_err(|e| format!("No API key stored for {engine}: {e}"))
}

/// Store API key via Windows Credential Manager (DPAPI-protected)
pub fn set_api_key(engine: &str, api_key: &str) -> Result<(), String> {
    let service = format!("overlex-{engine}");
    let entry = Entry::new(&service, "overlex").map_err(|e| format!("Credential error: {e}"))?;
    entry.set_password(api_key).map_err(|e| format!("Failed to store API key: {e}"))
}

/// Delete API key from Windows Credential Manager
pub fn delete_api_key(engine: &str) -> Result<(), String> {
    let service = format!("overlex-{engine}");
    let entry = Entry::new(&service, "overlex").map_err(|e| format!("Credential error: {e}"))?;
    entry.delete_credential().map_err(|e| format!("Failed to delete API key: {e}"))
}

/// Ensure settings are valid:
/// - Free engines are always present in enabled_engines
/// - Primary engine is always in enabled_engines
/// - No duplicates in enabled_engines
pub fn normalize_settings(settings: &mut Settings) {
    const FREE: &[&str] = &["google_gtx", "mymemory"];

    // Ensure free engines are always enabled
    for &engine in FREE {
        if !settings.enabled_engines.contains(&engine.to_string()) {
            settings.enabled_engines.push(engine.to_string());
        }
    }

    // Ensure primary_engine is in enabled_engines
    if !settings.enabled_engines.contains(&settings.primary_engine) {
        settings.enabled_engines.push(settings.primary_engine.clone());
    }

    // Deduplicate enabled_engines while preserving order
    let mut seen = std::collections::HashSet::new();
    settings.enabled_engines.retain(|e| seen.insert(e.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings_valid() {
        let settings = Settings::default();
        assert_eq!(settings.ocr_hotkey, "CTRL+SHIFT+T");
        assert_eq!(settings.write_hotkey, "CTRL+SHIFT+W");
    }

    #[test]
    fn test_validate_hotkeys_conflict() {
        let mut settings = Settings::default();
        settings.ocr_hotkey = "CTRL+SHIFT+T".to_string();
        settings.write_hotkey = "CTRL+SHIFT+T".to_string();
        
        let result = validate_hotkeys(&settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be the same"));
    }

    // --- normalize_settings tests (Task 5.2) ---

    #[test]
    fn test_normalize_adds_free_engines() {
        let mut settings = Settings::default();
        settings.enabled_engines = vec!["gemini".to_string()];
        normalize_settings(&mut settings);
        assert!(settings.enabled_engines.contains(&"google_gtx".to_string()));
        assert!(settings.enabled_engines.contains(&"mymemory".to_string()));
        assert!(settings.enabled_engines.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_normalize_does_not_duplicate_free() {
        let mut settings = Settings::default();
        settings.enabled_engines = vec![
            "google_gtx".to_string(),
            "mymemory".to_string(),
            "gemini".to_string(),
        ];
        normalize_settings(&mut settings);
        // Count occurrences
        let gtx_count = settings.enabled_engines.iter().filter(|e| *e == "google_gtx").count();
        let mm_count = settings.enabled_engines.iter().filter(|e| *e == "mymemory").count();
        assert_eq!(gtx_count, 1);
        assert_eq!(mm_count, 1);
    }

    #[test]
    fn test_normalize_adds_primary_to_enabled() {
        let mut settings = Settings::default();
        settings.primary_engine = "gemini".to_string();
        settings.enabled_engines = vec!["google_gtx".to_string(), "mymemory".to_string()];
        normalize_settings(&mut settings);
        assert!(settings.enabled_engines.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_normalize_idempotent() {
        let mut settings = Settings::default();
        settings.primary_engine = "gemini".to_string();
        settings.enabled_engines = vec![
            "google_gtx".to_string(),
            "mymemory".to_string(),
            "gemini".to_string(),
        ];
        normalize_settings(&mut settings);
        let first = settings.enabled_engines.clone();
        normalize_settings(&mut settings);
        assert_eq!(settings.enabled_engines, first);
    }

    #[test]
    fn test_normalize_deduplicates() {
        let mut settings = Settings::default();
        settings.enabled_engines = vec![
            "google_gtx".to_string(),
            "google_gtx".to_string(),
            "mymemory".to_string(),
            "gemini".to_string(),
            "gemini".to_string(),
        ];
        normalize_settings(&mut settings);
        let gtx_count = settings.enabled_engines.iter().filter(|e| *e == "google_gtx").count();
        let gem_count = settings.enabled_engines.iter().filter(|e| *e == "gemini").count();
        assert_eq!(gtx_count, 1);
        assert_eq!(gem_count, 1);
    }

    // --- Settings migration tests (Task 5.1) ---

    #[test]
    fn test_deserialize_old_format_engine_field() {
        let json = r#"{"ocr_hotkey":"CTRL+SHIFT+T","write_hotkey":"CTRL+SHIFT+W","source_lang":"auto","target_lang":"es","engine":"gemini","overlay_timeout_ms":5000,"overlay_position":"near-selection","start_with_windows":false,"ocr_preprocessing":true,"ocr_binarize":false,"history_enabled":true,"profiles":[],"show_debug":false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.primary_engine, "gemini");
        assert!(settings.enabled_engines.contains(&"google_gtx".to_string()));
        assert!(settings.enabled_engines.contains(&"mymemory".to_string()));
        assert!(settings.enabled_engines.contains(&"gemini".to_string()));
    }

    #[test]
    fn test_deserialize_old_format_engine_deepl() {
        let json = r#"{"ocr_hotkey":"CTRL+SHIFT+T","write_hotkey":"CTRL+SHIFT+W","source_lang":"auto","target_lang":"es","engine":"deepl","overlay_timeout_ms":5000,"overlay_position":"near-selection","start_with_windows":false,"ocr_preprocessing":true,"ocr_binarize":false,"history_enabled":true,"profiles":[],"show_debug":false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.primary_engine, "deepl");
        assert!(settings.enabled_engines.contains(&"deepl".to_string()));
    }

    #[test]
    fn test_deserialize_new_format_roundtrip() {
        let mut settings = Settings::default();
        settings.primary_engine = "deepseek".to_string();
        settings.enabled_engines = vec![
            "google_gtx".to_string(),
            "mymemory".to_string(),
            "deepseek".to_string(),
        ];
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.primary_engine, "deepseek");
        assert_eq!(loaded.enabled_engines, settings.enabled_engines);
    }

    #[test]
    fn test_deserialize_empty_json_uses_defaults() {
        let json = r#"{}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.primary_engine, "google_gtx");
        assert!(settings.enabled_engines.contains(&"google_gtx".to_string()));
        assert!(settings.enabled_engines.contains(&"mymemory".to_string()));
    }

    #[test]
    fn test_deserialize_explicit_primary_and_enabled() {
        let json = r#"{"ocr_hotkey":"CTRL+SHIFT+T","write_hotkey":"CTRL+SHIFT+W","source_lang":"auto","target_lang":"es","primary_engine":"gemini","enabled_engines":["google_gtx","mymemory","gemini"],"overlay_timeout_ms":5000,"overlay_position":"near-selection","start_with_windows":false,"ocr_preprocessing":true,"ocr_binarize":false,"history_enabled":true,"profiles":[],"show_debug":false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.primary_engine, "gemini");
        assert_eq!(settings.enabled_engines.len(), 3);
    }

    #[test]
    fn test_game_profile_alias_engine_to_primary_engine() {
        let json = r#"{"display_name":"Test","process_names":["test.exe"],"engine":"deepl"}"#;
        let profile: crate::commands::GameProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.primary_engine, Some("deepl".to_string()));
    }

    #[test]
    fn test_game_profile_primary_engine_new_format() {
        let json = r#"{"display_name":"Test","process_names":["test.exe"],"primary_engine":"gemini"}"#;
        let profile: crate::commands::GameProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.primary_engine, Some("gemini".to_string()));
    }
}