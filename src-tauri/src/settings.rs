// Settings module - load/save settings.json

use std::path::PathBuf;
use keyring::Entry;
use crate::commands::Settings;

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
        let defaults = Settings::default();
        let _ = save_settings_to_disk(&defaults);
        return defaults;
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str::<Settings>(&content) {
                Ok(settings) => settings,
                Err(e) => {
                    // Corrupt — backup and reset
                    eprintln!("Corrupt settings.json: {e}. Backing up and resetting.");
                    let bak = path.with_extension("json.bak");
                    let _ = std::fs::rename(&path, &bak);
                    let defaults = Settings::default();
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
}