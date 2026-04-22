// Commands module - all Tauri command handlers

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, Position};

use crate::{capture, ocr, ResultPayload, SettingsState, ScreenshotState, TranslationState, FocusRestoreState, settings};

/// Language swap result payload
#[derive(serde::Serialize, Clone)]
pub struct LanguageSwapResult {
    pub source_lang: String,
    pub target_lang: String,
}

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
            engine: "google_gtx".to_string(),
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

/// Swap source and target languages
/// If source is "auto", it becomes the target language (or the old target if it wasn't "auto")
#[tauri::command]
pub async fn swap_languages(
    settings_state: tauri::State<'_, SettingsState>,
    app_handle: tauri::AppHandle,
) -> Result<LanguageSwapResult, String> {
    let mut settings = settings_state.settings.lock().unwrap().clone();

    // Swap languages
    let new_source = settings.target_lang.clone();
    let new_target = if settings.source_lang == "auto" {
        // If source was "auto", keep "auto" as target or switch to the old target?
        // Convention: if source was "auto", swap so that the detected language becomes source
        // For simplicity: keep auto as target, swap what we can
        "auto".to_string()
    } else {
        settings.source_lang.clone()
    };

    settings.source_lang = new_source.clone();
    settings.target_lang = new_target.clone();

    // Save to disk
    settings::save_settings_to_disk(&settings)?;

    // Update in-memory state (already locked, just release)
    *settings_state.settings.lock().unwrap() = settings;

    let result = LanguageSwapResult {
        source_lang: new_source.clone(),
        target_lang: new_target.clone(),
    };

    // Emit event to all windows so they can update their UI
    let _ = app_handle.emit("languages-swapped", &result);

    eprintln!("Languages swapped: {} -> {}", new_source, new_target);
    Ok(result)
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
            // Try to emit to result window, fallback to global
            if let Some(result_window) = app_handle.get_webview_window("result") {
                let _ = result_window.emit("overlex-error", err_payload.clone());
                // Show and deliver error via eval for guaranteed delivery
                let _ = result_window.show();
                if let Ok(json_err) = serde_json::to_string(&err_payload) {
                    let _ = result_window.eval(&format!(
                        "if (window.onOverlexError) window.onOverlexError({});",
                        json_err
                    ));
                }
            } else {
                let _ = app_handle.emit("overlex-error", err_payload);
            }
            return Err(e.to_string());
        }
    };

    let translated_result = TranslationResult {
        original: text.clone(),
        translated: result.translated,
        detected_source: result.detected_source,
    };

    // Create payload BEFORE getting the window
    let payload = ResultPayload {
        original: text,
        translated: translated_result.translated.clone(),
        error: None,
        timeout_ms: settings.overlay_timeout_ms,
        source_lang: settings.source_lang.clone(),
        target_lang: settings.target_lang.clone(),
    };

    // Show result window and emit directly to it
    if let Some(result_window) = app_handle.get_webview_window("result") {
        let _ = result_window.show();

        // Position the result window based on settings
        position_result_window(&result_window, &settings, &None, 0, 0, 0, 0);

        // Emit directly to result window (not globally)
        let _ = result_window.emit("translation-result", &payload);

        // Guaranteed delivery via eval
        if let Ok(json_payload) = serde_json::to_string(&payload) {
            let _ = result_window.eval(&format!(
                "if (window.onTranslationResult) window.onTranslationResult({});",
                json_payload
            ));
        }
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
            // Try to emit to result window, fallback to global
            if let Some(result_window) = app_handle.get_webview_window("result") {
                let _ = result_window.emit("overlex-error", err_payload.clone());
                // Show and deliver error via eval for guaranteed delivery
                let _ = result_window.show();
                if let Ok(json_err) = serde_json::to_string(&err_payload) {
                    let _ = result_window.eval(&format!(
                        "if (window.onOverlexError) window.onOverlexError({});",
                        json_err
                    ));
                }
            } else {
                let _ = app_handle.emit("overlex-error", err_payload);
            }
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
            // Try to emit to result window, fallback to global
            if let Some(result_window) = app_handle.get_webview_window("result") {
                let _ = result_window.emit("overlex-error", err_payload.clone());
                // Show and deliver error via eval for guaranteed delivery
                let _ = result_window.show();
                if let Ok(json_err) = serde_json::to_string(&err_payload) {
                    let _ = result_window.eval(&format!(
                        "if (window.onOverlexError) window.onOverlexError({});",
                        json_err
                    ));
                }
            } else {
                let _ = app_handle.emit("overlex-error", err_payload);
            }
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
            // Try to emit to result window, fallback to global
            if let Some(result_window) = app_handle.get_webview_window("result") {
                let _ = result_window.emit("overlex-error", err_payload.clone());
                // Show and deliver error via eval for guaranteed delivery
                let _ = result_window.show();
                if let Ok(json_err) = serde_json::to_string(&err_payload) {
                    let _ = result_window.eval(&format!(
                        "if (window.onOverlexError) window.onOverlexError({});",
                        json_err
                    ));
                }
            } else {
                let _ = app_handle.emit("overlex-error", err_payload);
            }
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
            source_lang: settings.source_lang.clone(),
            target_lang: settings.target_lang.clone(),
        };

        // Try to emit directly to result window, fallback to global
        if let Some(result_window) = app_handle.get_webview_window("result") {
            let _ = result_window.emit("translation-result", &error_payload);
            // Also deliver via eval for guaranteed delivery
            if let Ok(json_err_payload) = serde_json::to_string(&error_payload) {
                let _ = result_window.eval(&format!(
                    "if (window.onTranslationResult) window.onTranslationResult({});",
                    json_err_payload
                ));
            }
        } else {
            let _ = app_handle.emit("translation-result", error_payload);
        }

        let err_payload = ErrorPayload {
            code: "OCR_EMPTY".to_string(),
            message: "No text detected in selection".to_string(),
        };
        if let Some(result_window) = app_handle.get_webview_window("result") {
            let _ = result_window.emit("overlex-error", err_payload.clone());
            // Also deliver error via eval
            if let Ok(json_err) = serde_json::to_string(&err_payload) {
                let _ = result_window.eval(&format!(
                    "if (window.onOverlexError) window.onOverlexError({});",
                    json_err
                ));
            }
        } else {
            let _ = app_handle.emit("overlex-error", err_payload);
        }
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
            // Try to emit to result window, fallback to global
            if let Some(result_window) = app_handle.get_webview_window("result") {
                let _ = result_window.emit("overlex-error", err_payload.clone());
                // Show and deliver error via eval for guaranteed delivery
                let _ = result_window.show();
                if let Ok(json_err) = serde_json::to_string(&err_payload) {
                    let _ = result_window.eval(&format!(
                        "if (window.onOverlexError) window.onOverlexError({});",
                        json_err
                    ));
                }
            } else {
                let _ = app_handle.emit("overlex-error", err_payload);
            }
            return Err(format!("Translation failed: {}", e));
        }
    };

    // Create payload BEFORE getting the window
    let payload = ResultPayload {
        original: original_text.clone(),
        translated: translation_result.translated.clone(),
        error: None,
        timeout_ms: settings.overlay_timeout_ms,
        source_lang: settings.source_lang.clone(),
        target_lang: settings.target_lang.clone(),
    };

    // Show result window and emit directly to it
    if let Some(result_window) = app_handle.get_webview_window("result") {
        let _ = result_window.show();

        // Position the result window based on settings with selection coordinates
        position_result_window(&result_window, &settings, &None, x, y, width, height);

        // Emit directly to result window (not globally)
        let _ = result_window.emit("translation-result", &payload);

        // Guaranteed delivery via eval
        if let Ok(json_payload) = serde_json::to_string(&payload) {
            let _ = result_window.eval(&format!(
                "if (window.onTranslationResult) window.onTranslationResult({});",
                json_payload
            ));
        }
    }

    // Hide freeze window
    if let Some(freeze_win) = app_handle.get_webview_window("freeze") {
        let _ = freeze_win.hide();
    }

    Ok(TranslationResult {
        original: original_text,
        translated: translation_result.translated,
        detected_source: translation_result.detected_source,
    })
}

/// Translate text via chat mode (write mode) - only translates, no window management
#[tauri::command]
pub async fn translate_chat(
    text: String,
    translation_state: tauri::State<'_, TranslationState>,
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<TranslationResult, String> {
    // Get settings for source/target languages
    let settings = settings_state.settings.lock().unwrap().clone();

    // Call translation engine
    let result = translation_state
        .engine
        .translate(&text, &settings.source_lang, &settings.target_lang)
        .await
        .map_err(|e| e.to_string())?;

    Ok(TranslationResult {
        original: text,
        translated: result.translated,
        detected_source: result.detected_source,
    })
}

/// Hide a specific window by label
#[tauri::command]
pub async fn hide_window(label: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app_handle.get_webview_window(&label) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
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

/// Get the DPI scale factor for the primary monitor
#[tauri::command]
pub async fn get_dpi_scale() -> Result<f64, String> {
    capture::get_dpi_scale()
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

/// Get the stored screenshot from ScreenshotState (already captured, not a new capture)
/// This eliminates the race condition of the start-freeze event
/// Returns raw PNG bytes instead of base64 to avoid 33% encoding overhead
#[tauri::command]
pub async fn get_stored_screenshot(
    screenshot_state: tauri::State<'_, ScreenshotState>,
) -> Result<Vec<u8>, String> {
    screenshot_state
        .png_data
        .lock()
        .unwrap()
        .clone()
        .ok_or("No screenshot stored".to_string())
}

/// Debug: log a message from JS to the Rust terminal
#[tauri::command]
pub fn js_log(msg: String) {
    eprintln!("{}", msg);
}
#[tauri::command]
pub fn drag_result_window_noactivate(x: i32, y: i32, app_handle: tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window("result") {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER};
            use windows::Win32::Foundation::HWND;

            if let Ok(hwnd) = window.hwnd() {
                unsafe {
                    let _ = SetWindowPos(
                        HWND(hwnd.0 as *mut _),
                        HWND(std::ptr::null_mut()),
                        x,
                        y,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                    );
                }
            }
        }
    }
}

