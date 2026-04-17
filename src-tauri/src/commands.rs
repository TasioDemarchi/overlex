// Commands module - all Tauri command handlers
// TODO: Implement OCR capture, translation, settings handlers

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub ocr_hotkey: String,
    pub write_hotkey: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine: String,
    pub overlay_timeout_ms: u32,
    pub overlay_position: String,
    pub start_with_windows: bool,
    pub libre_translate_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ocr_hotkey: "CTRL+SHIFT+T".to_string(),
            write_hotkey: "CTRL+SHIFT+W".to_string(),
            source_lang: "auto".to_string(),
            target_lang: "es".to_string(),
            engine: "libretranslate".to_string(),
            overlay_timeout_ms: 5000,
            overlay_position: "near-selection".to_string(),
            start_with_windows: false,
            libre_translate_url: "https://libretranslate.com".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranslationResult {
    pub original: String,
    pub translated: String,
    pub detected_source: Option<String>,
}

/// Get current settings
#[tauri::command]
pub async fn get_settings() -> Result<Settings, String> {
    // TODO: Load from settings.json
    Ok(Settings::default())
}

/// Save settings to disk
#[tauri::command]
pub async fn save_settings(settings: Settings) -> Result<(), String> {
    // TODO: Validate hotkeys, save to JSON, backup on corrupt
    let _ = settings;
    Ok(())
}

/// Translate text via write mode
#[tauri::command]
pub async fn translate_text(text: String) -> Result<TranslationResult, String> {
    // TODO: Call translation engine adapter
    let _ = text;
    Err("Not implemented".to_string())
}

/// Capture selected region from freeze overlay
#[tauri::command]
pub async fn ocr_capture_region(x: i32, y: i32, width: i32, height: i32) -> Result<TranslationResult, String> {
    // TODO: Crop screenshot, run OCR, translate, emit result
    let _ = (x, y, width, height);
    Err("Not implemented".to_string())
}

/// Dismiss result overlay
#[tauri::command]
pub async fn dismiss_result() -> Result<(), String> {
    // TODO: Hide result window
    Ok(())
}

/// Get fullscreen screenshot as base64 (for freeze overlay)
#[tauri::command]
pub async fn get_screenshot_base64() -> Result<String, String> {
    // TODO: BitBlt capture, encode to PNG, base64 encode
    Err("Not implemented".to_string())
}