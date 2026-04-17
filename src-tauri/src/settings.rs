// Settings module - load/save settings.json
// TODO: Implement settings persistence with serde_json

use std::path::PathBuf;

/// Get settings file path in APPDATA
pub fn get_settings_path() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(app_data).join("overlex").join("settings.json")
}

/// Load settings from disk - stub
pub fn load_settings() -> Result<crate::commands::Settings, String> {
    // TODO: Read settings.json, parse with serde_json, backup corrupt files
    Ok(crate::commands::Settings::default())
}

/// Save settings to disk - stub
pub fn save_settings(_settings: &crate::commands::Settings) -> Result<(), String> {
    // TODO: Validate hotkeys, write JSON, create parent dir if needed
    Ok(())
}

/// Get API key from DPAPI storage - stub
pub fn get_api_key(_engine: &str) -> Result<String, String> {
    // TODO: Use wincredential to retrieve DPAPI-protected key
    Err("Not implemented".to_string())
}

/// Store API key via DPAPI - stub
pub fn set_api_key(_engine: &str, _api_key: &str) -> Result<(), String> {
    // TODO: Use wincredential to store with DPAPI
    Err("Not implemented".to_string())
}