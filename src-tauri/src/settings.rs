// Settings module - load/save settings.json + API keys JSON file

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
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

// ---------------------------------------------------------------------------
// API keys JSON file storage (replaces Windows Credential Manager / keyring)
// ---------------------------------------------------------------------------

/// API keys storage schema (stored at %APPDATA%/overlex/api_keys.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiKeysStore {
    version: u32,
    keys: std::collections::HashMap<String, String>,
}

impl Default for ApiKeysStore {
    fn default() -> Self {
        Self {
            version: 1,
            keys: std::collections::HashMap::new(),
        }
    }
}

/// Get the API keys file path: %APPDATA%/overlex/api_keys.json
fn api_keys_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
    Ok(PathBuf::from(appdata).join("overlex").join("api_keys.json"))
}

/// Load the API keys store from disk. Creates a fresh store if the file
/// does not exist. On corrupt JSON: backup to .bak and start fresh.
fn load_api_keys_store() -> ApiKeysStore {
    let path = match api_keys_path() {
        Ok(p) => p,
        Err(_) => return ApiKeysStore::default(),
    };

    if !path.exists() {
        // First read — create with empty keys
        let store = ApiKeysStore::default();
        let _ = write_api_keys_store_atomic(&store);
        return store;
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<ApiKeysStore>(&content) {
            Ok(store) => store,
            Err(e) => {
                // Corrupt — backup and reset
                app_log!("Corrupt api_keys.json: {e}. Backing up and resetting.");
                let bak = path.with_extension("json.bak");
                let _ = std::fs::rename(&path, &bak);
                let store = ApiKeysStore::default();
                let _ = write_api_keys_store_atomic(&store);
                store
            }
        },
        Err(_) => ApiKeysStore::default(),
    }
}

/// Write the API keys store atomically: write to .tmp, then rename.
fn write_api_keys_store_atomic(store: &ApiKeysStore) -> Result<(), String> {
    let path = api_keys_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create api_keys dir: {e}"))?;
    }

    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Serialize api_keys failed: {e}"))?;

    // Write to temp file first
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("Write api_keys.tmp failed: {e}"))?;

    // Atomic rename
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Rename api_keys.tmp failed: {e}"))?;

    Ok(())
}

/// Get an API key for a given engine from the JSON file store.
pub fn get_api_key(engine: &str) -> Result<String, String> {
    let store = load_api_keys_store();
    store
        .keys
        .get(engine)
        .cloned()
        .ok_or_else(|| format!("No API key stored for {engine}"))
}

/// Store an API key for a given engine in the JSON file store.
pub fn set_api_key(engine: &str, api_key: &str) -> Result<(), String> {
    let mut store = load_api_keys_store();
    store.keys.insert(engine.to_string(), api_key.to_string());
    write_api_keys_store_atomic(&store)
}

/// Delete an API key for a given engine from the JSON file store.
pub fn delete_api_key(engine: &str) -> Result<(), String> {
    let mut store = load_api_keys_store();
    store.keys.remove(engine);
    write_api_keys_store_atomic(&store)
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

    // --- API keys JSON file storage tests ---

    /// RAII guard that cleans up the test temp directory on drop.
    struct TestDirGuard(std::path::PathBuf);
    impl Drop for TestDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Override APPDATA for test isolation. Returns the temp dir path and a guard.
    fn override_appdata_for_test() -> (TestDirGuard, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!("overlex_test_{}", std::process::id()));
        // Clean up any leftover from previous failed test run
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("APPDATA", &tmp);
        (TestDirGuard(tmp.clone()), tmp)
    }

    #[test]
    fn test_api_keys_store_create_read_write() {
        let (_guard, tmp) = override_appdata_for_test();

        // Start fresh — no keys file exists yet
        let key_path = tmp.join("overlex").join("api_keys.json");
        assert!(!key_path.exists());

        // Write a key
        set_api_key("gemini", "test-key-123").unwrap();
        assert!(key_path.exists());

        // Read it back
        let key = get_api_key("gemini").unwrap();
        assert_eq!(key, "test-key-123");

        // Write another key
        set_api_key("deepseek", "sk-deepseek").unwrap();
        let key2 = get_api_key("deepseek").unwrap();
        assert_eq!(key2, "sk-deepseek");

        // Both keys exist
        assert_eq!(get_api_key("gemini").unwrap(), "test-key-123");
        assert_eq!(get_api_key("deepseek").unwrap(), "sk-deepseek");
    }

    #[test]
    fn test_api_keys_store_corruption_recovery() {
        let (_guard, tmp) = override_appdata_for_test();

        // Write valid store first
        set_api_key("gemini", "initial-key").unwrap();
        assert_eq!(get_api_key("gemini").unwrap(), "initial-key");

        // Corrupt the file by writing garbage
        let key_path = tmp.join("overlex").join("api_keys.json");
        std::fs::write(&key_path, "NOT VALID JSON {{{").unwrap();

        // Reading should detect corruption and start fresh
        let result = get_api_key("gemini");
        assert!(result.is_err()); // gemini key is gone

        // Verify .bak was created
        let bak_path = tmp.join("overlex").join("api_keys.json.bak");
        assert!(bak_path.exists());

        // The original file should exist again (recreated fresh)
        assert!(key_path.exists());
        let content = std::fs::read_to_string(&key_path).unwrap();
        assert!(content.contains("\"version\": 1"));
        assert!(content.contains("\"keys\": {}"));
    }

    #[test]
    fn test_api_keys_store_atomic_write() {
        let (_guard, tmp) = override_appdata_for_test();

        // Write a key
        set_api_key("deepl", "fx-deepl-key").unwrap();

        // Verify no .tmp file remains after successful write
        let tmp_path = tmp.join("overlex").join("api_keys.json.tmp");
        assert!(!tmp_path.exists(), ".tmp file should not exist after successful write");

        // Verify the actual file exists and has correct content
        let key_path = tmp.join("overlex").join("api_keys.json");
        assert!(key_path.exists());
        let key = get_api_key("deepl").unwrap();
        assert_eq!(key, "fx-deepl-key");
    }

    #[test]
    fn test_get_api_key_missing_engine() {
        let (_guard, _tmp) = override_appdata_for_test();

        // No keys stored yet
        let result = get_api_key("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No API key stored"));
    }

    #[test]
    fn test_set_api_key_overwrites_existing() {
        let (_guard, tmp) = override_appdata_for_test();

        // Set initial key
        set_api_key("gemini", "first-key").unwrap();
        assert_eq!(get_api_key("gemini").unwrap(), "first-key");

        // Overwrite with new key
        set_api_key("gemini", "second-key").unwrap();
        assert_eq!(get_api_key("gemini").unwrap(), "second-key");

        // Verify the file only contains the latest key
        let key_path = tmp.join("overlex").join("api_keys.json");
        let content = std::fs::read_to_string(&key_path).unwrap();
        assert!(content.contains("second-key"));
        assert!(!content.contains("first-key"));
    }

    #[test]
    fn test_delete_api_key() {
        let (_guard, _tmp) = override_appdata_for_test();

        // Set a key
        set_api_key("gemini", "delete-me").unwrap();
        assert!(get_api_key("gemini").is_ok());

        // Delete it
        delete_api_key("gemini").unwrap();
        let result = get_api_key("gemini");
        assert!(result.is_err());

        // Delete a key that doesn't exist — should succeed (no-op)
        let result2 = delete_api_key("nonexistent");
        assert!(result2.is_ok());
    }
}