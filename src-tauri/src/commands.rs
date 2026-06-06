// Commands module - all Tauri command handlers

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, Position};

use crate::{
    capture, history,
    history::HistoryEntry,
    ocr, settings,
    translation::{TranslationContext, TranslationEngine, TranslationError},
    ActiveGameState, FocusRestoreState, HistoryState, ResultPayload, ScreenshotState,
    SettingsState, TranslationState,
};

// ============================================================
// In-memory log buffer for debugging
// ============================================================

const MAX_LOG_ENTRIES: usize = 200;

use std::time::SystemTime;

/// A single log entry
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Global log buffer (thread-safe)
static LOG_BUFFER: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());

/// Add a log entry (called via js_log command from frontend, or manually from Rust)
pub fn add_log(level: &str, message: &str) {
    if let Ok(mut buffer) = LOG_BUFFER.lock() {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| {
                let secs = d.as_secs();
                let hours = (secs / 3600) % 24;
                let mins = (secs / 60) % 60;
                let s = secs % 60;
                format!("{:02}:{:02}:{:02}", hours, mins, s)
            })
            .unwrap_or_else(|_| "??:??:??".to_string());

        buffer.push(LogEntry {
            timestamp,
            level: level.to_string(),
            message: message.to_string(),
        });

        // Keep only recent entries
        if buffer.len() > MAX_LOG_ENTRIES {
            let excess = buffer.len() - MAX_LOG_ENTRIES;
            buffer.drain(0..excess);
        }
    }
}

/// Log at INFO level
#[macro_export]
macro_rules! app_log {
    ($($arg:tt)*) => {{
        $crate::commands::add_log("INFO", &format!($($arg)*));
        eprintln!($($arg)*);
    }};
}

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
            app_log!("Failed to get screen size for positioning: {}", e);
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
        "bottom-right" => (
            screen_width - window_width - 20,
            screen_height - window_height - 20,
        ),
        "near-selection" | _ => {
            // Position near selection area - offset slightly to the right and down
            let x = sel_x + sel_width.min(50);
            let y = sel_y;
            (x, y)
        }
    };

    let _ = window.set_position(Position::Logical(tauri::LogicalPosition::new(
        pos_x as f64,
        pos_y as f64,
    )));
}

/// Error payload emitted on overlex-error events
#[derive(serde::Serialize, Clone)]
pub struct ErrorPayload {
    pub code: String, // "NETWORK_ERROR", "OCR_ERROR", "OCR_EMPTY", "OCR_LANGUAGE_MISSING", "RATE_LIMIT"
    pub message: String,
}

/// Emit an error to the result window with guaranteed delivery.
/// Tries to emit via Tauri event, then injects via eval() for guaranteed reception.
fn emit_error(app_handle: &tauri::AppHandle, error: ErrorPayload, show_window: bool) {
    if let Some(result_window) = app_handle.get_webview_window("result") {
        if show_window {
            let _ = result_window.show();
        }
        let _ = result_window.emit("overlex-error", error.clone());
        if let Ok(json) = serde_json::to_string(&error) {
            let _ = result_window.eval(&format!(
                "if (window.onOverlexError) window.onOverlexError({});",
                json
            ));
        }
    } else {
        let _ = app_handle.emit("overlex-error", error);
    }
}

/// Emit a translation result to the result window with guaranteed delivery.
/// Tries to emit via Tauri event, then injects via eval() for guaranteed reception.
fn emit_result(app_handle: &tauri::AppHandle, payload: &ResultPayload, show_window: bool) {
    if let Some(result_window) = app_handle.get_webview_window("result") {
        if show_window {
            let _ = result_window.show();
        }
        let _ = result_window.emit("translation-result", payload);
        if let Ok(json) = serde_json::to_string(payload) {
            let _ = result_window.eval(&format!(
                "if (window.onTranslationResult) window.onTranslationResult({});",
                json
            ));
        }
    } else {
        let _ = app_handle.emit("translation-result", payload);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameProfile {
    pub display_name: String,
    pub process_names: Vec<String>,
    #[serde(default)]
    pub source_lang: Option<String>,
    #[serde(default)]
    pub target_lang: Option<String>,
    /// Primary translation engine override. Accepts "engine" as alias for backward compat.
    #[serde(default, alias = "engine")]
    pub primary_engine: Option<String>,
    #[serde(default)]
    pub ocr_preprocessing: Option<bool>,
    #[serde(default)]
    pub ocr_binarize: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Settings {
    pub ocr_hotkey: String,
    pub write_hotkey: String,
    pub source_lang: String,
    pub target_lang: String,
    /// Primary translation engine (must be in enabled_engines)
    #[serde(default = "default_primary_engine")]
    pub primary_engine: String,
    /// All enabled engines (free engines always included)
    #[serde(default = "default_enabled_engines")]
    pub enabled_engines: Vec<String>,
    pub overlay_timeout_ms: u32,
    pub overlay_position: String,
    pub start_with_windows: bool,
    #[serde(default = "default_true")]
    pub ocr_preprocessing: bool,
    #[serde(default)]
    pub ocr_binarize: bool,
    #[serde(default = "default_true")]
    pub history_enabled: bool,
    #[serde(default)]
    pub profiles: Vec<GameProfile>,
    #[serde(default)]
    pub show_debug: bool,
}

fn default_primary_engine() -> String {
    "google_gtx".to_string()
}

fn default_enabled_engines() -> Vec<String> {
    vec!["google_gtx".to_string(), "mymemory".to_string()]
}

fn default_true() -> bool {
    true
}

/// Helper struct for deserialization — breaks the infinite recursion cycle
/// that would occur if we called serde_json::from_value<Settings> inside
/// the custom Deserialize impl.
#[derive(Debug, Deserialize)]
#[serde(default)]
struct SettingsRaw {
    pub ocr_hotkey: String,
    pub write_hotkey: String,
    pub source_lang: String,
    pub target_lang: String,
    pub primary_engine: String,
    pub enabled_engines: Vec<String>,
    pub overlay_timeout_ms: u32,
    pub overlay_position: String,
    pub start_with_windows: bool,
    pub ocr_preprocessing: bool,
    pub ocr_binarize: bool,
    pub history_enabled: bool,
    pub profiles: Vec<GameProfile>,
    pub show_debug: bool,
}

impl Default for SettingsRaw {
    fn default() -> Self {
        let s = Settings::default();
        Self {
            ocr_hotkey: s.ocr_hotkey,
            write_hotkey: s.write_hotkey,
            source_lang: s.source_lang,
            target_lang: s.target_lang,
            primary_engine: s.primary_engine,
            enabled_engines: s.enabled_engines,
            overlay_timeout_ms: s.overlay_timeout_ms,
            overlay_position: s.overlay_position,
            start_with_windows: s.start_with_windows,
            ocr_preprocessing: s.ocr_preprocessing,
            ocr_binarize: s.ocr_binarize,
            history_enabled: s.history_enabled,
            profiles: s.profiles,
            show_debug: s.show_debug,
        }
    }
}

/// Custom Deserialize to handle backward compatibility with the old `engine` field.
/// If `engine` is present but `primary_engine` is missing, migrate:
///   primary_engine = engine
///   enabled_engines = ["google_gtx", "mymemory", engine]
impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let value = serde_json::Value::deserialize(deserializer)?;

        // Detect old format
        let old_engine = value.get("engine").and_then(|v| v.as_str()).map(|s| s.to_string());
        let has_primary = value.get("primary_engine").is_some();

        // Deserialize into SettingsRaw (has #[derive(Deserialize)], no recursion)
        let raw: SettingsRaw = serde_json::from_value(value).map_err(D::Error::custom)?;

        let mut settings = Settings {
            ocr_hotkey: raw.ocr_hotkey,
            write_hotkey: raw.write_hotkey,
            source_lang: raw.source_lang,
            target_lang: raw.target_lang,
            primary_engine: raw.primary_engine,
            enabled_engines: raw.enabled_engines,
            overlay_timeout_ms: raw.overlay_timeout_ms,
            overlay_position: raw.overlay_position,
            start_with_windows: raw.start_with_windows,
            ocr_preprocessing: raw.ocr_preprocessing,
            ocr_binarize: raw.ocr_binarize,
            history_enabled: raw.history_enabled,
            profiles: raw.profiles,
            show_debug: raw.show_debug,
        };

        // Migrate old format
        if let Some(engine) = old_engine {
            if !has_primary {
                settings.primary_engine = engine.clone();
            }
            if !settings.enabled_engines.contains(&engine) {
                settings.enabled_engines.push(engine);
            }
        }

        Ok(settings)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ocr_hotkey: "CTRL+SHIFT+T".to_string(),
            write_hotkey: "CTRL+SHIFT+W".to_string(),
            source_lang: "auto".to_string(),
            target_lang: "es".to_string(),
            primary_engine: default_primary_engine(),
            enabled_engines: default_enabled_engines(),
            overlay_timeout_ms: 5000,
            overlay_position: "near-selection".to_string(),
            start_with_windows: false,
            ocr_preprocessing: true,
            ocr_binarize: false,
            history_enabled: true,
            profiles: Vec::new(),
            show_debug: false,
        }
    }
}

#[derive(Serialize, Clone, Default)]
pub struct ActiveGameInfo {
    pub process_name: Option<String>,
    pub fullscreen_exclusive: bool,
    pub matched_profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranslationResult {
    pub original: String,
    pub translated: String,
    pub detected_source: Option<String>,
    pub engine_used: String,
    pub fallback: bool,
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

    app_log!("Languages swapped: {} -> {}", new_source, new_target);
    Ok(result)
}

/// Get base settings (saved defaults without profile overrides).
/// This is what the settings UI uses to populate the form.
#[tauri::command]
pub async fn get_settings(
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<Settings, String> {
    let settings = settings_state.saved_defaults.lock().unwrap().clone();
    Ok(settings)
}

/// Get active/effective settings (with profile overrides applied).
/// This is what overlays (write, result) use for engine/language info.
#[tauri::command]
pub async fn get_active_settings(
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<Settings, String> {
    let settings = settings_state.settings.lock().unwrap().clone();
    Ok(settings)
}

/// Return type for save_settings with per-engine keyring error map.
/// Empty map = total success (including keyring).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SaveSettingsResponse {
    /// Per-engine errors from Credential Manager operations.
    /// Key = engine name, Value = error message.
    pub key_errors: std::collections::HashMap<String, String>,
}

/// Save settings to disk
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    api_keys: std::collections::HashMap<String, String>,
    settings_state: tauri::State<'_, SettingsState>,
    active_game_state: tauri::State<'_, ActiveGameState>,
    hotkey_state: tauri::State<'_, std::sync::Mutex<crate::hotkeys::HotkeyState>>,
    translation_state: tauri::State<'_, TranslationState>,
    app_handle: tauri::AppHandle,
) -> Result<SaveSettingsResponse, String> {
    app_log!("[SETTINGS] Saving settings, api_keys provided: {}", api_keys.len());

    // Validate hotkeys
    settings::validate_hotkeys(&settings)?;

    // Normalize settings before saving
    let mut normalized_settings = settings.clone();
    settings::normalize_settings(&mut normalized_settings);

    // Persist API keys to Credential Manager (best-effort).
    // Keys must be stored BEFORE create_all_engines so engines can load them on restart.
    let mut key_errors: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (engine, key) in &api_keys {
        if let Err(e) = settings::set_api_key(engine, key) {
            app_log!("[SETTINGS] Failed to store API key for {}: {}", engine, e);
            key_errors.insert(engine.clone(), e);
        }
    }
    if !key_errors.is_empty() {
        app_log!("[SETTINGS] {} API key(s) failed to persist: {:?}", key_errors.len(), key_errors.keys());
    }

    // Save to disk (persist the raw settings from the frontend)
    settings::save_settings_to_disk(&normalized_settings)?;

    // Determine effective settings: apply profile overrides if a profile is active.
    // Lock order: settings(1) → saved_defaults(2) → active_game.info(3) → engines/chain(4).
    //
    // Step 1: Read current primary from active settings (lock 1).
    let old_primary = settings_state.settings.lock().unwrap().primary_engine.clone();

    // Step 2: Update saved_defaults (lock 2), check active profile (lock 3),
    //          and compute the effective settings.
    let effective_settings: Settings = {
        let mut saved = settings_state.saved_defaults.lock().unwrap();
        *saved = normalized_settings.clone();

        let active_profile_name = {
            let info = active_game_state.info.lock().unwrap();
            info.matched_profile.clone()
        };

        if let Some(ref profile_name) = active_profile_name {
            if let Some(profile) = saved
                .profiles
                .iter()
                .find(|p| &p.display_name == profile_name)
            {
                let overridden = apply_profile_overrides(&saved, profile);
                app_log!(
                    "[SETTINGS] Re-applied profile '{}' overrides after save",
                    profile_name
                );
                overridden
            } else {
                saved.clone()
            }
        } else {
            saved.clone()
        }
    };

    // Step 3: Update active settings (lock 1).
    *settings_state.settings.lock().unwrap() = effective_settings.clone();

    // Step 4: Rebuild engines HashMap and TranslationChain from saved config (lock 4).
    {
        let enabled = normalized_settings.enabled_engines.clone();
        let new_engines = crate::translation::create_all_engines(&enabled, &api_keys);
        let new_chain = crate::translation::TranslationChain::new(
            &normalized_settings.primary_engine,
            new_engines.clone(),
            &enabled,
        );

        {
            let mut engines_guard = translation_state.engines.write().unwrap();
            *engines_guard = new_engines;
        }
        {
            let mut chain_guard = translation_state.chain.write().unwrap();
            *chain_guard = std::sync::Arc::new(new_chain);
        }

        app_log!(
            "[SETTINGS] Engine chain rebuilt: primary={}, enabled={:?}",
            normalized_settings.primary_engine, enabled
        );
    }

    // Re-register hotkeys with the effective settings
    let mut hk = hotkey_state.lock().map_err(|e| e.to_string())?;
    crate::hotkeys::register_hotkeys(
        &mut hk,
        &effective_settings.ocr_hotkey,
        &effective_settings.write_hotkey,
        app_handle.clone(),
    )?;

    // Emit settings-changed so overlays re-check show_debug, engine, and languages
    let _ = app_handle.emit(
        "settings-changed",
        serde_json::json!({
            "show_debug": effective_settings.show_debug,
            "primary_engine": effective_settings.primary_engine,
            "enabled_engines": effective_settings.enabled_engines,
            "source_lang": effective_settings.source_lang,
            "target_lang": effective_settings.target_lang,
        }),
    );

    app_log!(
        "[SETTINGS] Saved. effective primary_engine={}, show_debug={}",
        effective_settings.primary_engine, effective_settings.show_debug
    );

    Ok(SaveSettingsResponse { key_errors })
}

/// Translate text via write mode
#[tauri::command]
pub async fn translate_text(
    text: String,
    translation_state: tauri::State<'_, TranslationState>,
    settings_state: tauri::State<'_, SettingsState>,
    active_game_state: tauri::State<'_, ActiveGameState>,
    focus_state: tauri::State<'_, FocusRestoreState>,
    _history_state: tauri::State<'_, HistoryState>,
    app_handle: tauri::AppHandle,
) -> Result<TranslationResult, String> {
    // Get settings
    let settings = settings_state.settings.lock().unwrap().clone();

    // Build TranslationContext from active game info
    let context = {
        let info = active_game_state.info.lock().unwrap();
        match (&info.process_name, &info.matched_profile) {
            (None, None) => None,
            _ => Some(TranslationContext {
                process_name: info.process_name.clone(),
                profile_name: info.matched_profile.clone(),
            }),
        }
    };

    // Call translation chain (acquire read lock, clone Arc, release lock before async call)
    let chain = translation_state.chain.read().unwrap().clone();
    let result = match chain
        .translate(
            &text,
            &settings.source_lang,
            &settings.target_lang,
            context.as_ref(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            match &e {
                TranslationError::InvalidApiKey => {
                    emit_error(
                        &app_handle,
                        ErrorPayload {
                            code: "INVALID_API_KEY".to_string(),
                            message: format!(
                                "API key required for the translation engine. Set it in Settings.",
                            ),
                        },
                        true,
                    );
                }
                _ => {
                    emit_error(
                        &app_handle,
                        ErrorPayload {
                            code: "NETWORK_ERROR".to_string(),
                            message: e.to_string(),
                        },
                        true,
                    );
                }
            }
            if let Some(write_win) = app_handle.get_webview_window("write") {
                let _ = write_win.hide();
            }
            return Err(e.to_string());
        }
    };

    let translated_result = TranslationResult {
        original: text.clone(),
        translated: result.translated.clone(),
        detected_source: result.detected_source.clone(),
        engine_used: result.engine_used.clone(),
        fallback: result.fallback,
    };

    // Create payload BEFORE getting the window
    let payload = ResultPayload {
        original: text,
        translated: translated_result.translated.clone(),
        error: None,
        timeout_ms: settings.overlay_timeout_ms,
        source_lang: settings.source_lang.clone(),
        target_lang: settings.target_lang.clone(),
        engine_used: result.engine_used.clone(),
        fallback: result.fallback,
    };

    // Show result window and emit directly to it
    if let Some(result_window) = app_handle.get_webview_window("result") {
        position_result_window(&result_window, &settings, &None, 0, 0, 0, 0);
    }
    emit_result(&app_handle, &payload, true);

    // Close write window and restore focus to previous app
    if let Some(write_win) = app_handle.get_webview_window("write") {
        let _ = write_win.hide();
    }

    // Restore focus to the previously foreground window
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

        let stored = focus_state.hwnd.lock().unwrap().take();
        if let Some(raw_hwnd) = stored {
            unsafe {
                let _ = SetForegroundWindow(HWND(raw_hwnd as *mut _));
            }
        }
    }

    // Save to history (fire-and-forget if enabled)
    if settings.history_enabled {
        let engine_used = translated_result.engine_used.clone();
        let entry = history::HistoryEntry {
            id: 0,
            original_text: translated_result.original.clone(),
            translated_text: translated_result.translated.clone(),
            source_lang: settings.source_lang.clone(),
            target_lang: settings.target_lang.clone(),
            engine: engine_used,
            created_at: String::new(),
        };
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = history::HistoryDb::insert(&entry) {
                app_log!("[HISTORY] Failed to save entry: {}", e);
            }
        });
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
    active_game_state: tauri::State<'_, ActiveGameState>,
    app_handle: tauri::AppHandle,
) -> Result<TranslationResult, String> {
    // 1. Get screenshot from state
    let screenshot = match screenshot_state.png_data.lock().unwrap().clone() {
        Some(s) => s,
        None => {
            emit_error(
                &app_handle,
                ErrorPayload {
                    code: "OCR_ERROR".to_string(),
                    message: "No screenshot available. Start OCR flow first.".to_string(),
                },
                true,
            );
            if let Some(freeze_win) = app_handle.get_webview_window("freeze") {
                let _ = freeze_win.hide();
            }
            return Err("No screenshot available. Start OCR flow first.".to_string());
        }
    };

    // 2. Crop the region
    let cropped_png = match capture::capture_region(&screenshot, x, y, width as u32, height as u32)
    {
        Ok(c) => c,
        Err(e) => {
            emit_error(
                &app_handle,
                ErrorPayload {
                    code: "OCR_ERROR".to_string(),
                    message: format!("Failed to capture region: {}", e),
                },
                true,
            );
            if let Some(freeze_win) = app_handle.get_webview_window("freeze") {
                let _ = freeze_win.hide();
            }
            return Err(format!("Failed to capture region: {}", e));
        }
    };

    // 3. Pre-process image if enabled (runs in spawn_blocking to avoid blocking async runtime)
    let processed_png = {
        let settings = settings_state.settings.lock().unwrap().clone();
        if settings.ocr_preprocessing {
            let binarize = settings.ocr_binarize;
            let cropped_clone = cropped_png.clone();
            match tokio::task::spawn_blocking(move || {
                ocr::preprocess_for_ocr(&cropped_clone, binarize)
            })
            .await
            {
                Ok(Ok(processed)) => {
                    app_log!("[OCR] Pre-processing applied (binarize={})", binarize);
                    processed
                }
                Ok(Err(e)) => {
                    app_log!("[OCR] Pre-processing failed, using original: {}", e);
                    cropped_png
                }
                Err(e) => {
                    app_log!("[OCR] Pre-processing task panicked: {}", e);
                    cropped_png
                }
            }
        } else {
            cropped_png
        }
    };

    // 4. Run OCR - ocr_region is async but internally uses .get() to block
    let ocr_result = match ocr::ocr_region(&processed_png).await {
        Ok(r) => r,
        Err(e) => {
            emit_error(
                &app_handle,
                ErrorPayload {
                    code: "OCR_ERROR".to_string(),
                    message: format!("OCR failed: {}", e),
                },
                true,
            );
            if let Some(freeze_win) = app_handle.get_webview_window("freeze") {
                let _ = freeze_win.hide();
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
            engine_used: String::new(),
            fallback: false,
        };

        emit_result(&app_handle, &error_payload, true);
        emit_error(
            &app_handle,
            ErrorPayload {
                code: "OCR_EMPTY".to_string(),
                message: "No text detected in selection".to_string(),
            },
            true,
        );
        if let Some(freeze_win) = app_handle.get_webview_window("freeze") {
            let _ = freeze_win.hide();
        }
        return Err("No text detected in selection".to_string());
    }

    let original_text = ocr_result.text.trim().to_string();

    // 5. Get settings
    let settings = settings_state.settings.lock().unwrap().clone();

    // 6. Build TranslationContext from active game info
    let context = {
        let info = active_game_state.info.lock().unwrap();
        match (&info.process_name, &info.matched_profile) {
            (None, None) => None,
            _ => Some(TranslationContext {
                process_name: info.process_name.clone(),
                profile_name: info.matched_profile.clone(),
            }),
        }
    };

    // 7. Translate via chain (acquire read lock, clone Arc, release lock before async call)
    let chain = translation_state.chain.read().unwrap().clone();
    let translation_result = match chain
        .translate(
            &original_text,
            &settings.source_lang,
            &settings.target_lang,
            context.as_ref(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            match &e {
                TranslationError::InvalidApiKey => {
                    emit_error(
                        &app_handle,
                        ErrorPayload {
                            code: "INVALID_API_KEY".to_string(),
                            message: format!(
                                "API key required for the translation engine. Set it in Settings.",
                            ),
                        },
                        true,
                    );
                }
                _ => {
                    emit_error(
                        &app_handle,
                        ErrorPayload {
                            code: "NETWORK_ERROR".to_string(),
                            message: format!("Translation failed: {}", e),
                        },
                        true,
                    );
                }
            }
            if let Some(freeze_win) = app_handle.get_webview_window("freeze") {
                let _ = freeze_win.hide();
            }
            return Err(format!("Translation failed: {}", e));
        }
    };

    // 7. Save to history (fire-and-forget if enabled)
    if settings.history_enabled {
        let engine_used = translation_result.engine_used.clone();
        let entry = history::HistoryEntry {
            id: 0,
            original_text: original_text.clone(),
            translated_text: translation_result.translated.clone(),
            source_lang: settings.source_lang.clone(),
            target_lang: settings.target_lang.clone(),
            engine: engine_used,
            created_at: String::new(), // DB sets this via DEFAULT
        };
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = history::HistoryDb::insert(&entry) {
                app_log!("[HISTORY] Failed to save entry: {}", e);
            }
        });
    }

    // Create payload BEFORE getting the window
    let payload = ResultPayload {
        original: original_text.clone(),
        translated: translation_result.translated.clone(),
        error: None,
        timeout_ms: settings.overlay_timeout_ms,
        source_lang: settings.source_lang.clone(),
        target_lang: settings.target_lang.clone(),
        engine_used: translation_result.engine_used.clone(),
        fallback: translation_result.fallback,
    };

    // Show result window and emit directly to it
    if let Some(result_window) = app_handle.get_webview_window("result") {
        position_result_window(&result_window, &settings, &None, x, y, width, height);
    }
    emit_result(&app_handle, &payload, true);

    // Hide freeze window
    if let Some(freeze_win) = app_handle.get_webview_window("freeze") {
        let _ = freeze_win.hide();
    }

    Ok(TranslationResult {
        original: original_text,
        translated: translation_result.translated,
        detected_source: translation_result.detected_source,
        engine_used: translation_result.engine_used,
        fallback: translation_result.fallback,
    })
}

/// Translate text via chat mode (write mode) - only translates, no window management
#[tauri::command]
pub async fn translate_chat(
    text: String,
    translation_state: tauri::State<'_, TranslationState>,
    settings_state: tauri::State<'_, SettingsState>,
    active_game_state: tauri::State<'_, ActiveGameState>,
    _history_state: tauri::State<'_, HistoryState>,
    app_handle: tauri::AppHandle,
) -> Result<TranslationResult, String> {
    // Get settings for source/target languages
    let settings = settings_state.settings.lock().unwrap().clone();

    // Build TranslationContext from active game info
    let context = {
        let info = active_game_state.info.lock().unwrap();
        match (&info.process_name, &info.matched_profile) {
            (None, None) => None,
            _ => Some(TranslationContext {
                process_name: info.process_name.clone(),
                profile_name: info.matched_profile.clone(),
            }),
        }
    };

    // Call translation chain (acquire read lock, clone Arc, release lock before async call)
    let chain = translation_state.chain.read().unwrap().clone();
    let result = chain
        .translate(
            &text,
            &settings.source_lang,
            &settings.target_lang,
            context.as_ref(),
        )
        .await
        .map_err(|e| {
            if let TranslationError::InvalidApiKey = &e {
                emit_error(
                    &app_handle,
                    ErrorPayload {
                        code: "INVALID_API_KEY".to_string(),
                        message: "API key required for the translation engine. Set it in Settings.".to_string(),
                    },
                    true,
                );
            }
            e.to_string()
        })?;

    let engine_used = result.engine_used.clone();
    let translated_result = TranslationResult {
        original: text.clone(),
        translated: result.translated,
        detected_source: result.detected_source,
        engine_used,
        fallback: result.fallback,
    };

    // Save to history (fire-and-forget if enabled)
    if settings.history_enabled {
        let history_engine = translated_result.engine_used.clone();
        let entry = history::HistoryEntry {
            id: 0,
            original_text: translated_result.original.clone(),
            translated_text: translated_result.translated.clone(),
            source_lang: settings.source_lang.clone(),
            target_lang: settings.target_lang.clone(),
            engine: history_engine,
            created_at: String::new(),
        };
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = history::HistoryDb::insert(&entry) {
                app_log!("[HISTORY] Failed to save entry: {}", e);
            }
        });
    }

    Ok(translated_result)
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

/// Get recent log entries for debugging
#[tauri::command]
pub fn get_recent_logs() -> Vec<LogEntry> {
    LOG_BUFFER.lock().map(|b| b.clone()).unwrap_or_default()
}

/// Add a log entry from the frontend (for debugging)
#[tauri::command]
pub fn log_from_frontend(level: String, message: String) {
    add_log(&level, &message);
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

/// Check if API key exists for a given engine (debug helper)
#[tauri::command]
pub async fn check_api_key(engine: String) -> Result<bool, String> {
    match settings::get_api_key(&engine) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Set API key for translation engine
#[tauri::command]
pub async fn set_api_key(engine: String, key: String) -> Result<(), String> {
    settings::set_api_key(&engine, &key)
}

/// Test API key by making a minimal request to the engine's API
/// Receives the key directly (not from storage) to avoid race conditions
#[tauri::command]
pub async fn test_api_key(engine: String, key: String) -> Result<TestApiKeyResult, String> {
    if key.is_empty() {
        return Ok(TestApiKeyResult {
            success: false,
            message: "API key is empty".to_string(),
        });
    }

    match engine.as_str() {
        "gemini" => {
            // Test Gemini API with a minimal request
            // Use header authentication (official Google method)
            let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

            let body = serde_json::json!({
                "contents": [{
                    "parts": [{"text": "Hi"}]
                }]
            });

            let response = client
                .post(url)
                .header("x-goog-api-key", &key)
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Network error: {}", e))?;

            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();

            if status == 200 {
                Ok(TestApiKeyResult {
                    success: true,
                    message: "Gemini API key is valid and working".to_string(),
                })
            } else if status == 403 {
                // Check if it's a billing issue
                if body_text.contains("billing")
                    || body_text.contains("BILLING")
                    || body_text.contains("quota")
                    || body_text.contains("QUOTA")
                    || body_text.contains("Cloud billing")
                    || body_text.contains("project billing")
                {
                    Ok(TestApiKeyResult {
                        success: false,
                        message: "Billing not enabled. Enable billing in your Google Cloud project to use the free tier.".to_string(),
                    })
                } else {
                    Ok(TestApiKeyResult {
                        success: false,
                        message: format!("API key rejected (HTTP 403). Check if the key is valid and has Gemini API access enabled.").to_string(),
                    })
                }
            } else if status == 400 {
                Ok(TestApiKeyResult {
                    success: false,
                    message: format!("Invalid request (HTTP 400): {}", &body_text[..body_text.len().min(200)]).to_string(),
                })
            } else {
                Ok(TestApiKeyResult {
                    success: false,
                    message: format!("API error (HTTP {}): {}", status, &body_text[..body_text.len().min(200)]).to_string(),
                })
            }
        }
        "deepl" => {
            // Test DeepL API with a minimal request
            let url = "https://api-free.deepl.com/v2/translate";

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

            let response = client
                .post(url)
                .form(&[
                    ("auth_key", key.as_str()),
                    ("text", "Hi"),
                    ("target_lang", "ES"),
                ])
                .send()
                .await
                .map_err(|e| format!("Network error: {}", e))?;

            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();

            if status == 200 {
                Ok(TestApiKeyResult {
                    success: true,
                    message: "DeepL API key is valid and working".to_string(),
                })
            } else if status == 403 {
                Ok(TestApiKeyResult {
                    success: false,
                    message: "DeepL API key rejected (HTTP 403). Check if the key is valid and has access to the free tier.".to_string(),
                })
            } else {
                Ok(TestApiKeyResult {
                    success: false,
                    message: format!("DeepL API error (HTTP {}): {}", status, &body_text[..body_text.len().min(200)]).to_string(),
                })
            }
        }
        "deepseek" => {
            let url = "https://api.deepseek.com/chat/completions";

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

            let body = serde_json::json!({
                "model": "deepseek-v4-flash",
                "messages": [{"role": "user", "content": "Hi"}],
                "max_tokens": 10
            });

            let response = client
                .post(url)
                .header("Authorization", format!("Bearer {}", key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Network error: {}", e))?;

            let status = response.status();
            if status.is_success() {
                Ok(TestApiKeyResult {
                    success: true,
                    message: "DeepSeek API key is valid and working".to_string(),
                })
            } else {
                let body = response.text().await.unwrap_or_default();
                match status.as_u16() {
                    401 | 403 => Ok(TestApiKeyResult {
                        success: false,
                        message: "Invalid API key. Check your key at platform.deepseek.com".to_string(),
                    }),
                    _ => Ok(TestApiKeyResult {
                        success: false,
                        message: format!("Error: {} - {}", status, body),
                    }),
                }
            }
        }
        _ => {
            Err(format!("Engine '{}' does not support API key testing", engine))
        }
    }
}

/// Result of API key test
#[derive(serde::Serialize)]
pub struct TestApiKeyResult {
    pub success: bool,
    pub message: String,
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
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
            };

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

// ============================================================================
// History Commands
// ============================================================================

/// Get translation history with pagination (newest first)
#[tauri::command]
pub async fn get_history(
    limit: u32,
    offset: u32,
    _history: tauri::State<'_, HistoryState>,
) -> Result<Vec<HistoryEntry>, String> {
    let (limit, offset) = (limit, offset);
    tokio::task::spawn_blocking(move || history::HistoryDb::get_all(limit, offset))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

/// Search translation history using FTS5
#[tauri::command]
pub async fn search_history(
    query: String,
    _history: tauri::State<'_, HistoryState>,
) -> Result<Vec<HistoryEntry>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return tokio::task::spawn_blocking(move || history::HistoryDb::get_all(50, 0))
            .await
            .map_err(|e| format!("Task join error: {}", e))?;
    }
    let query_for_search = query.clone();
    tokio::task::spawn_blocking(move || history::HistoryDb::search(&query_for_search))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

/// Export translation history as JSON or CSV
#[tauri::command]
pub async fn export_history(
    format: String,
    _history: tauri::State<'_, HistoryState>,
) -> Result<String, String> {
    let format = format.clone();
    tokio::task::spawn_blocking(move || history::HistoryDb::export(&format))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

/// Clear all translation history
#[tauri::command]
pub async fn clear_history(_history: tauri::State<'_, HistoryState>) -> Result<(), String> {
    tokio::task::spawn_blocking(history::HistoryDb::clear)
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

/// Delete a specific history entry by ID
#[tauri::command]
pub async fn delete_history_entry(
    id: i64,
    _history: tauri::State<'_, HistoryState>,
) -> Result<(), String> {
    let id = id;
    tokio::task::spawn_blocking(move || history::HistoryDb::delete(id))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

// ============================================================================
// Profile Commands
// ============================================================================

/// Helper: apply a GameProfile's override fields on top of base Settings.
/// Only `Some` fields override; `None` fields leave the base value intact.
pub(crate) fn apply_profile_overrides(base: &Settings, profile: &GameProfile) -> Settings {
    let mut s = base.clone();
    if let Some(v) = &profile.source_lang {
        s.source_lang = v.clone();
    }
    if let Some(v) = &profile.target_lang {
        s.target_lang = v.clone();
    }
    if let Some(v) = &profile.primary_engine {
        s.primary_engine = v.clone();
    }
    if let Some(v) = profile.ocr_preprocessing {
        s.ocr_preprocessing = v;
    }
    if let Some(v) = profile.ocr_binarize {
        s.ocr_binarize = v;
    }
    s
}

/// Add a game profile, persist to disk, and apply overrides if it matches
/// the currently-active foreground process.
#[tauri::command]
pub async fn add_profile(
    profile: GameProfile,
    settings_state: tauri::State<'_, SettingsState>,
    active_game: tauri::State<'_, ActiveGameState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Check whether the new profile matches the current foreground process
    let matches_active = {
        let info = active_game.info.lock().unwrap();
        info.process_name.as_ref().map_or(false, |pn| {
            profile
                .process_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(pn))
        })
    };

    // 1. Push to saved_defaults and persist
    {
        let mut saved = settings_state.saved_defaults.lock().unwrap().clone();
        saved.profiles.push(profile.clone());
        settings::save_settings_to_disk(&saved)?;
        *settings_state.saved_defaults.lock().unwrap() = saved;
    }

    // 2. Push to active settings tier; apply overrides if matching
    let overridden_settings = {
        let mut settings = settings_state.settings.lock().unwrap().clone();
        settings.profiles.push(profile.clone());

        if matches_active {
            let saved = settings_state.saved_defaults.lock().unwrap().clone();
            let overridden = apply_profile_overrides(&saved, &profile);
            settings.source_lang = overridden.source_lang;
            settings.target_lang = overridden.target_lang;
            settings.primary_engine = overridden.primary_engine;
            settings.ocr_preprocessing = overridden.ocr_preprocessing;
            settings.ocr_binarize = overridden.ocr_binarize;
        }

        let clone_for_event = settings.clone();
        *settings_state.settings.lock().unwrap() = settings;
        clone_for_event
    };

    // 3. Update active-game matched_profile if applicable
    if matches_active {
        {
            let mut info = active_game.info.lock().unwrap();
            info.matched_profile = Some(profile.display_name.clone());
        }
        let info = active_game.info.lock().unwrap().clone();
        let _ = app_handle.emit("active-game-changed", &info);
    }

    let _ = app_handle.emit("settings-changed", &overridden_settings);
    app_log!(
        "[PROFILE] Added profile '{}' (matches_active={})",
        profile.display_name, matches_active
    );
    Ok(())
}

/// Remove a game profile by display_name, persist, and revert to defaults
/// if the removed profile was the active one.
#[tauri::command]
pub async fn remove_profile(
    display_name: String,
    settings_state: tauri::State<'_, SettingsState>,
    active_game: tauri::State<'_, ActiveGameState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Check if the profile being removed is the currently matched one
    let was_active = {
        let info = active_game.info.lock().unwrap();
        info.matched_profile.as_deref() == Some(&display_name)
    };

    // 1. Remove from saved_defaults and persist
    {
        let mut saved = settings_state.saved_defaults.lock().unwrap().clone();
        let len_before = saved.profiles.len();
        saved.profiles.retain(|p| p.display_name != display_name);
        if saved.profiles.len() == len_before {
            return Err(format!("Profile '{}' not found", display_name));
        }
        settings::save_settings_to_disk(&saved)?;
        *settings_state.saved_defaults.lock().unwrap() = saved;
    }

    // 2. Remove from active settings; revert to saved_defaults if was active
    let updated_settings = {
        let mut settings = settings_state.settings.lock().unwrap().clone();
        settings.profiles.retain(|p| p.display_name != display_name);

        if was_active {
            let saved = settings_state.saved_defaults.lock().unwrap().clone();
            settings.source_lang = saved.source_lang;
            settings.target_lang = saved.target_lang;
            settings.primary_engine = saved.primary_engine;
            settings.ocr_preprocessing = saved.ocr_preprocessing;
            settings.ocr_binarize = saved.ocr_binarize;
        }

        let clone_for_event = settings.clone();
        *settings_state.settings.lock().unwrap() = settings;
        clone_for_event
    };

    // 3. Clear matched_profile if this was the active profile
    if was_active {
        {
            let mut info = active_game.info.lock().unwrap();
            info.matched_profile = None;
        }
        let info = active_game.info.lock().unwrap().clone();
        let _ = app_handle.emit("active-game-changed", &info);
    }

    let _ = app_handle.emit("settings-changed", &updated_settings);
    app_log!(
        "[PROFILE] Removed profile '{}' (was_active={})",
        display_name, was_active
    );
    Ok(())
}

/// Update an existing game profile by display_name, persist, and re-apply
/// overrides if it is the currently active profile.
#[tauri::command]
pub async fn update_profile(
    profile: GameProfile,
    settings_state: tauri::State<'_, SettingsState>,
    active_game: tauri::State<'_, ActiveGameState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let display_name = profile.display_name.clone();

    // Check if the profile being updated is the currently matched one
    let is_active = {
        let info = active_game.info.lock().unwrap();
        info.matched_profile.as_deref() == Some(&display_name)
    };

    // 1. Replace in saved_defaults and persist
    {
        let mut saved = settings_state.saved_defaults.lock().unwrap().clone();
        let pos = saved
            .profiles
            .iter()
            .position(|p| p.display_name == display_name)
            .ok_or_else(|| format!("Profile '{}' not found", display_name))?;
        saved.profiles[pos] = profile.clone();
        settings::save_settings_to_disk(&saved)?;
        *settings_state.saved_defaults.lock().unwrap() = saved;
    }

    // 2. Replace in active settings; re-apply overrides if active
    let updated_settings = {
        let mut settings = settings_state.settings.lock().unwrap().clone();
        if let Some(pos) = settings
            .profiles
            .iter()
            .position(|p| p.display_name == display_name)
        {
            settings.profiles[pos] = profile.clone();
        }

        if is_active {
            let saved = settings_state.saved_defaults.lock().unwrap().clone();
            let overridden = apply_profile_overrides(&saved, &profile);
            settings.source_lang = overridden.source_lang;
            settings.target_lang = overridden.target_lang;
            settings.primary_engine = overridden.primary_engine;
            settings.ocr_preprocessing = overridden.ocr_preprocessing;
            settings.ocr_binarize = overridden.ocr_binarize;
        }

        let clone_for_event = settings.clone();
        *settings_state.settings.lock().unwrap() = settings;
        clone_for_event
    };

    // 3. Emit events
    if is_active {
        let info = active_game.info.lock().unwrap().clone();
        let _ = app_handle.emit("active-game-changed", &info);
    }
    let _ = app_handle.emit("settings-changed", &updated_settings);
    app_log!(
        "[PROFILE] Updated profile '{}' (is_active={})",
        display_name, is_active
    );
    Ok(())
}

/// Return all configured game profiles.
#[tauri::command]
pub async fn list_profiles(
    settings_state: tauri::State<'_, SettingsState>,
) -> Result<Vec<GameProfile>, String> {
    let profiles = settings_state.settings.lock().unwrap().profiles.clone();
    Ok(profiles)
}

/// Return current active game information (foreground process + matched profile).
#[tauri::command]
pub async fn get_active_game(
    active_game: tauri::State<'_, ActiveGameState>,
) -> Result<ActiveGameInfo, String> {
    let info = active_game.info.lock().unwrap().clone();
    Ok(info)
}

/// Toggle the debug indicator on overlays.
#[tauri::command]
pub async fn toggle_debug(
    show: bool,
    settings_state: tauri::State<'_, SettingsState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // 1. Update saved_defaults and persist
    {
        let mut saved = settings_state.saved_defaults.lock().unwrap().clone();
        saved.show_debug = show;
        settings::save_settings_to_disk(&saved)?;
        *settings_state.saved_defaults.lock().unwrap() = saved;
    }

    // 2. Update active settings
    let updated_settings = {
        let mut settings = settings_state.settings.lock().unwrap().clone();
        settings.show_debug = show;
        let clone_for_event = settings.clone();
        *settings_state.settings.lock().unwrap() = settings;
        clone_for_event
    };

    // 3. Emit event so overlays update
    let _ = app_handle.emit("settings-changed", &updated_settings);
    app_log!("[DEBUG] show_debug set to {}", show);
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_match_case_insensitive() {
        let profile = GameProfile {
            display_name: "Path of Exile".to_string(),
            process_names: vec!["poe2.exe".to_string(), "PathOfExile_x64.exe".to_string()],
            source_lang: None,
            target_lang: Some("es".to_string()),
            primary_engine: None,
            ocr_preprocessing: None,
            ocr_binarize: None,
        };

        // Case-insensitive matching should work for both uppercase and mixed-case queries
        assert!(profile
            .process_names
            .iter()
            .any(|p| p.to_lowercase() == "POE2.EXE".to_lowercase()));
        assert!(profile
            .process_names
            .iter()
            .any(|p| p.to_lowercase() == "pathofexile_x64.exe".to_lowercase()));
        assert!(!profile
            .process_names
            .iter()
            .any(|p| p.to_lowercase() == "notepad.exe".to_lowercase()));
    }

    #[test]
    fn test_profile_match_multiple_processes() {
        let profile = GameProfile {
            display_name: "POE".to_string(),
            process_names: vec![
                "PathOfExileSteam.exe".to_string(),
                "PathOfExile_x64.exe".to_string(),
                "PathOfExile.exe".to_string(),
            ],
            source_lang: None,
            target_lang: None,
            primary_engine: None,
            ocr_preprocessing: None,
            ocr_binarize: None,
        };

        // All variants should match case-insensitively
        assert!(profile
            .process_names
            .iter()
            .any(|p| p.eq_ignore_ascii_case("PathOfExileSteam.exe")));
        assert!(profile
            .process_names
            .iter()
            .any(|p| p.eq_ignore_ascii_case("PathOfExile.exe")));
    }

    #[test]
    fn test_override_application() {
        let base_settings = Settings {
            source_lang: "en".to_string(),
            target_lang: "es".to_string(),
            primary_engine: "google_gtx".to_string(),
            enabled_engines: vec!["google_gtx".to_string(), "mymemory".to_string()],
            ocr_preprocessing: true,
            ocr_binarize: false,
            ..Default::default()
        };

        let profile = GameProfile {
            display_name: "POE".to_string(),
            process_names: vec!["poe2.exe".to_string()],
            source_lang: None,                   // Don't override
            target_lang: Some("ja".to_string()), // Override
            primary_engine: Some("gemini".to_string()),  // Override
            ocr_preprocessing: None,             // Don't override
            ocr_binarize: Some(true),            // Override
        };

        let result = apply_profile_overrides(&base_settings, &profile);

        assert_eq!(result.source_lang, "en"); // Not overridden
        assert_eq!(result.target_lang, "ja"); // Overridden
        assert_eq!(result.primary_engine, "gemini"); // Overridden
        assert_eq!(result.ocr_preprocessing, true); // Not overridden
        assert_eq!(result.ocr_binarize, true); // Overridden
    }

    #[test]
    fn test_profiles_serde_default() {
        let json = r#"{"source_lang":"en","target_lang":"es","primary_engine":"google_gtx","enabled_engines":["google_gtx","mymemory"],"ocr_hotkey":"ctrl+shift+t","write_hotkey":"ctrl+shift+w","ocr_preprocessing":true,"ocr_binarize":false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();

        // New fields should use serde defaults when absent from JSON
        assert!(settings.profiles.is_empty());
        assert!(!settings.show_debug);
    }

    #[test]
    fn test_settings_backward_compat() {
        // Old settings.json without profiles/show_debug should deserialize with defaults
        let json = r#"{"source_lang":"en","target_lang":"es","engine":"google_gtx","ocr_hotkey":"ctrl+shift+t","write_hotkey":"ctrl+shift+w","ocr_preprocessing":true,"ocr_binarize":false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();

        assert!(settings.profiles.is_empty());
        assert!(!settings.show_debug);
        assert_eq!(settings.source_lang, "en");
        assert_eq!(settings.target_lang, "es");
        assert_eq!(settings.primary_engine, "google_gtx"); // migrated from old `engine` field
    }
}
