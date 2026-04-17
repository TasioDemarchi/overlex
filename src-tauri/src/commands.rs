// Commands module - all Tauri command handlers

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, Position};

use crate::{capture, ocr, ResultPayload, SettingsState, ScreenshotState, TranslationState, FocusRestoreState, settings};

/// Position the result window based on settings.
/// For "near-selection": uses the selection coordinates passed (x, y, width, height).
/// For corner positions: calculates position based on screen size.
fn position_result_window(
    window: &tauri::WebviewWindow,
    settings: &Settings,
    _selection: &Option<(i32, i32, i32, i32)>,
    sel_x: i32,
    sel_y: i32,
    sel_width: i32,
    _sel_height: i32,
) {
    // Get screen size
    let (screen_width, screen_height) = match capture::get_screen_size() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to get screen size for positioning: {}", e);
            return;
        }
    };

    // Get window size (approximate - could query but hardcoded for simplicity)
    let window_width = 350;
    let window_height = 200;

    let (pos_x, pos_y) = match settings.overlay_position.as_str() {
        "top-left" => (20, 20),
        "top-right" => (screen_width - window_width - 20, 20),
        "bottom-left" => (20, screen_height - window_height - 20),
        "bottom-right" => (screen_width - window_width - 20, screen_height - window_height - 20),
        "near-selection" | _ => {
            // Position near selection area - offset slightly to the right and down
            let x = sel_x + sel_width.min(50);
            let y = sel_y;
            (x, y)
        }
    };

    let _ = window.set_position(Position::Logical(tauri::LogicalPosition::new(pos_x as f64, pos_y as f64)));
}

/// Error payload emitted on overlex-error events
#[derive(serde::Serialize, Clone)]
pub struct ErrorPayload {
    pub code: String,    // "NETWORK_ERROR", "OCR_ERROR", "OCR_EMPTY", "OCR_LANGUAGE_MISSING", "RATE_LIMIT"
    pub message: String,
}

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
pub async fn get_settings(settings_state: tauri::State<'_, SettingsState>) -> Result<Settings, String> {
    let settings = settings_state.settings.lock().unwrap().clone();
    Ok(settings)
}

/// Save settings to disk
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    settings_state: tauri::State<'_, SettingsState>,
    hotkey_state: tauri::State<'_, std::sync::Mutex<crate::hotkeys::HotkeyState>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Validate hotkeys
    settings::validate_hotkeys(&settings)?;

    // Save to disk
    settings::save_settings_to_disk(&settings)?;

    // Update in-memory state
    *settings_state.settings.lock().unwrap() = settings.clone();

    // Re-register hotkeys
    let mut hk = hotkey_state.lock().map_err(|e| e.to_string())?;
    crate::hotkeys::register_hotkeys(&mut hk, &settings.ocr_hotkey, &settings.write_hotkey, app_handle)?;

    Ok(())
}

/// Translate text via write mode
#[tauri::command]
pub async fn translate_text(
    text: String,
    translation_state: tauri::State<'_, TranslationState>,
    settings_state: tauri::State<'_, SettingsState>,
    focus_state: tauri::State<'_, FocusRestoreState>,
    app_handle: tauri::AppHandle,
) -> Result<TranslationResult, String> {
    // Get settings
    let settings = settings_state.settings.lock().unwrap().clone();

    // Call translation engine
    let result = match translation_state
        .engine
        .translate(&text, &settings.source_lang, &settings.target_lang)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let err_payload = ErrorPayload {
                code: "NETWORK_ERROR".to_string(),
                message: e.to_string(),
            };
            let _ = app_handle.emit("overlex-error", err_payload);
            return Err(e.to_string());
        }
    };

    let translated_result = TranslationResult {
        original: text.clone(),
        translated: result.translated,
        detected_source: result.detected_source,
    };

    // Emit result event
    let payload = ResultPayload {
        original: text,
        translated: translated_result.translated.clone(),
        error: None,
        timeout_ms: settings.overlay_timeout_ms,
    };
    let _ = app_handle.emit("translation-result", payload);

    // Show result window
    if let Some(result_window) = app_handle.get_webview_window("result") {
        let _ = result_window.show();

        // Position the result window based on settings
        position_result_window(&result_window, &settings, &None, 0, 0, 0, 0);
    }

    // Close write window and restore focus to previous app
    if let Some(write_win) = app_handle.get_webview_window("write") {
        let _ = write_win.hide();
    }

    // Restore focus to the previously foreground window
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        use windows::Win32::Foundation::HWND;

        let stored = focus_state.hwnd.lock().unwrap().take();
        if let Some(raw_hwnd) = stored {
            unsafe { let _ = SetForegroundWindow(HWND(raw_hwnd as *mut _)); }
        }
    }

    Ok(translated_result)
}

/// Capture selected region from freeze overlay
#[tauri::command]
pub async fn ocr_capture_region(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    screenshot_state: tauri::State<'_, ScreenshotState>,
    translation_state: tauri::State<'_, TranslationState>,
    settings_state: tauri::State<'_, SettingsState>,
    app_handle: tauri::AppHandle,
) -> Result<TranslationResult, String> {
    // 1. Get screenshot from state
    let screenshot = match screenshot_state.png_data.lock().unwrap().clone() {
        Some(s) => s,
        None => {
            let err_payload = ErrorPayload {
                code: "OCR_ERROR".to_string(),
                message: "No screenshot available. Start OCR flow first.".to_string(),
            };
            let _ = app_handle.emit("overlex-error", err_payload);
            return Err("No screenshot available. Start OCR flow first.".to_string());
        }
    };

    // 2. Crop the region
    let cropped_png = match capture::capture_region(&screenshot, x, y, width as u32, height as u32) {
        Ok(c) => c,
        Err(e) => {
            let err_payload = ErrorPayload {
                code: "OCR_ERROR".to_string(),
                message: format!("Failed to capture region: {}", e),
            };
            let _ = app_handle.emit("overlex-error", err_payload);
            return Err(format!("Failed to capture region: {}", e));
        }
    };

    // 3. Run OCR - ocr_region is async but internally uses .get() to block
    let ocr_result = match ocr::ocr_region(&cropped_png).await {
        Ok(r) => r,
        Err(e) => {
            let err_payload = ErrorPayload {
                code: "OCR_ERROR".to_string(),
                message: format!("OCR failed: {}", e),
            };
            let _ = app_handle.emit("overlex-error", err_payload);
            return Err(format!("OCR failed: {}", e));
        }
    };

    // 4. Check if text was detected
    if ocr_result.text.trim().is_empty() {
        // Get settings for timeout value
        let settings = settings_state.settings.lock().unwrap().clone();

        let error_payload = ResultPayload {
            original: String::new(),
            translated: String::new(),
            error: Some("No text detected in selection".to_string()),
            timeout_ms: settings.overlay_timeout_ms,
        };
        let _ = app_handle.emit("translation-result", error_payload);

        let err_payload = ErrorPayload {
            code: "OCR_EMPTY".to_string(),
            message: "No text detected in selection".to_string(),
        };
        let _ = app_handle.emit("overlex-error", err_payload);
        return Err("No text detected in selection".to_string());
    }

    let original_text = ocr_result.text.trim().to_string();

    // 5. Get settings
    let settings = settings_state.settings.lock().unwrap().clone();

    // 6. Translate
    let translation_result = match translation_state
        .engine
        .translate(&original_text, &settings.source_lang, &settings.target_lang)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let err_payload = ErrorPayload {
                code: "NETWORK_ERROR".to_string(),
                message: format!("Translation failed: {}", e),
            };
            let _ = app_handle.emit("overlex-error", err_payload);
            return Err(format!("Translation failed: {}", e));
        }
    };

    // 7. Emit result event
    let payload = ResultPayload {
        original: original_text.clone(),
        translated: translation_result.translated.clone(),
        error: None,
        timeout_ms: settings.overlay_timeout_ms,
    };
    let _ = app_handle.emit("translation-result", payload);

    // 8. Show result window
    if let Some(result_window) = app_handle.get_webview_window("result") {
        let _ = result_window.show();

        // Position the result window based on settings with selection coordinates
        position_result_window(&result_window, &settings, &None, x, y, width, height);
    }

    Ok(TranslationResult {
        original: original_text,
        translated: translation_result.translated,
        detected_source: translation_result.detected_source,
    })
}

/// Dismiss result overlay
#[tauri::command]
pub async fn dismiss_result(app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window("result") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Get fullscreen screenshot as base64 (for freeze overlay)
#[tauri::command]
pub async fn get_screenshot_base64() -> Result<String, String> {
    let png_bytes = tokio::task::spawn_blocking(capture::capture_fullscreen)
        .await
        .map_err(|e| format!("Screenshot task panicked: {}", e))??;

    Ok(STANDARD.encode(&png_bytes))
}

/// Get API key for translation engine
#[tauri::command]
pub async fn get_api_key(engine: String) -> Result<String, String> {
    settings::get_api_key(&engine)
}

/// Set API key for translation engine
#[tauri::command]
pub async fn set_api_key(engine: String, key: String) -> Result<(), String> {
    settings::set_api_key(&engine, &key)
}