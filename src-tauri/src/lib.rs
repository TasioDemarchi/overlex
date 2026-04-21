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
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime, Listener, Position, LogicalPosition,
};
use image::{ImageBuffer, Rgba, ImageEncoder};
use translation::TranslationEngine;
use window_vibrancy::apply_acrylic;

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
                    eprintln!("Opening settings window...");
                    let show_result = window.show();
                    let focus_result = window.set_focus();
                    eprintln!("show: {:?}, focus: {:?}", show_result, focus_result);
                } else {
                    eprintln!("Settings window 'main' NOT FOUND");
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

            // Initialize translation engine (default: Google GTX - no API key required)
            // LibreTranslateAdapter is available as alternative if user configures it
            let engine = translation::GoogleGtxAdapter::new();
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
                    eprintln!("[RUST] starting capture...");
                    // 1. Capture screenshot as raw RGBA bytes FIRST (before showing freeze window)
                    //    If we show the window first, BitBlt captures the black overlay instead of the screen.
                    let raw_result = match tokio::task::spawn_blocking(capture::capture_fullscreen_raw).await {
                        Ok(Ok(data)) => data,
                        Ok(Err(e)) => {
                            eprintln!("Screenshot capture failed: {}", e);
                            return;
                        }
                        Err(e) => {
                            eprintln!("Screenshot task panicked: {}", e);
                            return;
                        }
                    };

                    let (rgba_bytes, width, height) = raw_result;
                    eprintln!("[RUST] capture done {}x{}, encoding PNG...", width, height);

                    // 2. Encode PNG synchronously (CompressionType::Fast for speed)
                    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = match ImageBuffer::from_raw(width, height, rgba_bytes.clone()) {
                        Some(i) => i,
                        None => {
                            eprintln!("Failed to create image buffer for PNG encoding");
                            return;
                        }
                    };

                    let mut png_bytes: Vec<u8> = Vec::new();
                    let encoder = image::codecs::png::PngEncoder::new_with_quality(
                        std::io::Cursor::new(&mut png_bytes),
                        image::codecs::png::CompressionType::Fast,
                        image::codecs::png::FilterType::NoFilter,
                    );
                    if let Err(e) = encoder.write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8) {
                        eprintln!("PNG encoding failed: {}", e);
                        return;
                    }

                    // 3. Store PNG in ScreenshotState for OCR (ocr_capture_region uses this)
                    if let Some(state) = handle.try_state::<ScreenshotState>() {
                        *state.png_data.lock().unwrap() = Some(png_bytes.clone());
                    }

                    // DEBUG: save PNG to disk to verify it's not corrupt
                    let _ = std::fs::write("C:\\Users\\Slim-7\\Desktop\\debug_screenshot.png", &png_bytes);
                    eprintln!("[RUST] PNG saved to Desktop for inspection");

                    // 4. Show freeze window
                    if let Some(freeze_win) = handle.get_webview_window("freeze") {
                        let _ = freeze_win.show();
                        let _ = freeze_win.set_focus();
                        eprintln!("[RUST] freeze window shown, png_bytes len={}", png_bytes.len());

                        // Delay to let WebView fully render before injecting
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                        // Encode PNG as base64 and inject directly via eval — bypasses event system entirely
                        use base64::Engine as _;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                        eprintln!("[RUST] base64 len={}, injecting via eval...", b64.len());
                        let js = format!(
                            r#"
                            (function() {{
                                var canvas = document.getElementById('freeze-canvas');
                                if (!canvas) {{
                                    window.__TAURI__.core.invoke('js_log', {{msg: '[JS] ERROR: canvas not found'}});
                                    return;
                                }}
                                var ctx = canvas.getContext('2d', {{ alpha: false }});
                                canvas.width = window.innerWidth;
                                canvas.height = window.innerHeight;
                                // TEST: paint red first to confirm WebView renders
                                ctx.fillStyle = '#ff0000';
                                ctx.fillRect(0, 0, canvas.width, canvas.height);
                                window.__TAURI__.core.invoke('js_log', {{msg: '[JS] painted RED ' + canvas.width + 'x' + canvas.height}});
                                var img = new Image();
                                img.onload = function() {{
                                    ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
                                    ctx.fillStyle = 'rgba(0,0,0,0.3)';
                                    ctx.fillRect(0, 0, canvas.width, canvas.height);
                                    window._screenshotImg = img;
                                    window.__TAURI__.core.invoke('js_log', {{msg: '[JS] drawn screenshot ' + canvas.width + 'x' + canvas.height}});
                                }};
                                img.onerror = function() {{
                                    window.__TAURI__.core.invoke('js_log', {{msg: '[JS] ERROR: img decode failed'}});
                                }};
                                img.src = 'data:image/png;base64,{b64}';
                            }})();
                            "#,
                            b64 = b64
                        );
                        let eval_result = freeze_win.eval(&js);
                        eprintln!("[RUST] eval result: {:?}", eval_result);
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

                    // Position based on overlay_position setting
                    // "near-selection" doesn't make sense for write mode (no selection), treat as "bottom-right"
                    if let Some(settings_state) = handle.try_state::<SettingsState>() {
                        let settings = settings_state.settings.lock().unwrap().clone();
                        let (screen_w, screen_h) = capture::get_screen_size().unwrap_or((1920, 1080));
                        let win_w = 420i32;
                        let win_h = 300i32;
                        let margin = 20i32;

                        let (pos_x, pos_y) = match settings.overlay_position.as_str() {
                            "top-left" => (margin, margin),
                            "top-right" => (screen_w - win_w - margin, margin),
                            "bottom-left" => (margin, screen_h - win_h - margin),
                            // "near-selection" and "bottom-right" both → bottom-right
                            _ => (screen_w - win_w - margin, screen_h - win_h - margin),
                        };

                        let _ = write_win.set_position(Position::Logical(
                            LogicalPosition::new(pos_x as f64, pos_y as f64)
                        ));
                    }
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

            // Apply Windows blur effects (Mica/Acrylic/Blur) for visual enhancement
            #[cfg(target_os = "windows")]
            {
                if let Some(write_win) = app.get_webview_window("write") {
                    let _ = apply_acrylic(&write_win, Some((13, 17, 23, 160)));
                }

                if let Some(result_win) = app.get_webview_window("result") {
                    let _ = apply_acrylic(&result_win, Some((13, 17, 23, 160)));
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::translate_text,
            commands::translate_chat,
            commands::ocr_capture_region,
            commands::dismiss_result,
            commands::hide_window,
            commands::get_screenshot_base64,
            commands::get_api_key,
            commands::set_api_key,
            commands::get_stored_screenshot,
            commands::drag_result_window_noactivate,
            commands::get_dpi_scale,
            commands::js_log,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
