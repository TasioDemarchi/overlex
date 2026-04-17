// OverLex - Main library entry point
// This is the library crate that Tauri uses

pub mod capture;
pub mod commands;
pub mod hotkeys;
pub mod ocr;
pub mod settings;
pub mod translation;
pub mod tray;

pub use commands::Settings;

use std::sync::{Arc, Mutex};
use base64::{engine::general_purpose::STANDARD, Engine};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime, Emitter, Listener,
};
use translation::TranslationEngine;

#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

/// State to hold the translation engine (Arc for thread-safe access in async commands)
pub struct TranslationState {
    pub engine: Arc<dyn TranslationEngine>,
}

/// State to hold the latest screenshot (for OCR region capture)
pub struct ScreenshotState {
    pub png_data: Arc<Mutex<Option<Vec<u8>>>>,
}

/// State to hold current settings
pub struct SettingsState {
    pub settings: Arc<Mutex<Settings>>,
}

/// State to hold the foreground window handle for focus restore after write mode
pub struct FocusRestoreState {
    pub hwnd: Arc<Mutex<Option<isize>>>,
}

/// Result payload sent to result window
#[derive(serde::Serialize, Clone)]
pub struct ResultPayload {
    pub original: String,
    pub translated: String,
    pub error: Option<String>,
    pub timeout_ms: u32,
}

fn setup_tray<R: Runtime>(app: &tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Setup system tray
            setup_tray(app)?;

            // Load settings from disk (or create defaults on first run)
            let settings = settings::load_settings();
            let settings_for_hotkey = settings.clone();

            // Initialize translation engine
            let engine = translation::LibreTranslateAdapter::new(
                settings.libre_translate_url.clone(),
                None, // No API key by default
            );
            let translation_state = TranslationState {
                engine: Arc::new(engine),
            };
            app.manage(translation_state);

            // Initialize screenshot state (empty initially)
            let screenshot_state = ScreenshotState {
                png_data: Arc::new(Mutex::new(None)),
            };
            app.manage(screenshot_state);

            // Initialize settings state
            let settings_state = SettingsState {
                settings: Arc::new(Mutex::new(settings)),
            };
            app.manage(settings_state);

            // Initialize focus restore state (for write mode)
            let focus_state = FocusRestoreState {
                hwnd: Arc::new(Mutex::new(None)),
            };
            app.manage(focus_state);

            // Register global hotkeys with loaded settings
            let mut hotkey_state = hotkeys::HotkeyState::new();
            match hotkeys::register_hotkeys(
                &mut hotkey_state,
                &settings_for_hotkey.ocr_hotkey,
                &settings_for_hotkey.write_hotkey,
                app.handle().clone(),
            ) {
                Ok(()) => eprintln!("Global hotkeys registered successfully"),
                Err(e) => eprintln!("Failed to register hotkeys: {e}"),
            }
            // Store state so we can unregister later
            app.manage(std::sync::Mutex::new(hotkey_state));

            // Listen for start-ocr-flow event to capture screenshot and show freeze overlay
            let app_handle_ocr = app.handle().clone();
            app.listen("start-ocr-flow", move |_event| {
                let handle = app_handle_ocr.clone();
                tauri::async_runtime::spawn(async move {
                    // 1. Capture screenshot (blocking)
                    let png = match tokio::task::spawn_blocking(capture::capture_fullscreen).await {
                        Ok(Ok(data)) => data,
                        Ok(Err(e)) => {
                            eprintln!("Screenshot failed: {}", e);
                            return;
                        }
                        Err(e) => {
                            eprintln!("Screenshot task panicked: {}", e);
                            return;
                        }
                    };

                    // 2. Store in screenshot state
                    if let Some(state) = handle.try_state::<ScreenshotState>() {
                        *state.png_data.lock().unwrap() = Some(png.clone());
                    }

                    // 3. Base64 encode and emit to freeze window
                    let b64 = STANDARD.encode(&png);

                    // 4. Show freeze window and emit screenshot
                    if let Some(freeze_win) = handle.get_webview_window("freeze") {
                        let _ = freeze_win.show();
                        let _ = handle.emit("start-freeze", serde_json::json!({ "screenshot_b64": b64 }));
                    }
                });
            });

            // Listen for start-write-flow event to show write overlay
            let app_handle_write = app.handle().clone();
            app.listen("start-write-flow", move |_event| {
                let handle = app_handle_write.clone();

                // Store the current foreground window handle before showing write window
                #[cfg(target_os = "windows")]
                {
                    let raw_hwnd = unsafe { GetForegroundWindow() };
                    if let Some(state) = handle.try_state::<FocusRestoreState>() {
                        *state.hwnd.lock().unwrap() = Some(raw_hwnd.0 as isize);
                    }
                }

                if let Some(write_win) = handle.get_webview_window("write") {
                    let _ = write_win.show();
                    let _ = write_win.set_focus();
                }
            });

            // Get the result window and apply WS_EX_NOACTIVATE
            if let Some(result_window) = app.get_webview_window("result") {
                #[cfg(target_os = "windows")]
                {
                    use windows::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GetWindowLongPtrW, GWL_EXSTYLE};
                    use windows::Win32::Foundation::HWND;
                    
                    if let Ok(hwnd) = result_window.hwnd() {
                        let hwnd = HWND(hwnd.0);
                        let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
                        unsafe {
                            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | 0x08000000_isize); // WS_EX_NOACTIVATE = 0x08000000
                        }
                        eprintln!("Result window WS_EX_NOACTIVATE set successfully");
                    } else {
                        eprintln!("Warning: Could not get HWND for result window");
                    }
                }
            } else {
                eprintln!("Result window not found in setup - will be created on demand");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::translate_text,
            commands::ocr_capture_region,
            commands::dismiss_result,
            commands::get_screenshot_base64,
            commands::get_api_key,
            commands::set_api_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
