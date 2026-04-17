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

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
};

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

            // Register global hotkeys with default settings
            let settings = commands::Settings::default();
            let mut hotkey_state = hotkeys::HotkeyState::new();
            match hotkeys::register_hotkeys(
                &mut hotkey_state,
                &settings.ocr_hotkey,
                &settings.write_hotkey,
                app.handle().clone(),
            ) {
                Ok(()) => eprintln!("Global hotkeys registered successfully"),
                Err(e) => eprintln!("Failed to register hotkeys: {e}"),
            }
            // Store state so we can unregister later
            app.manage(std::sync::Mutex::new(hotkey_state));

            // Pre-create result window (hidden)
            let result_window = app.get_webview_window("result");
            if result_window.is_none() {
                eprintln!("Result window not pre-created - will be created on demand");
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
