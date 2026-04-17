// Hotkeys module - global hotkey registration via Win32 RegisterHotKey
// Runs a dedicated OS thread with a message pump for WM_HOTKEY events.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT,
    VIRTUAL_KEY, VK_A, VK_B, VK_C, VK_D, VK_E, VK_F, VK_G, VK_H, VK_I, VK_J, VK_K, VK_L,
    VK_M, VK_N, VK_O, VK_P, VK_Q, VK_R, VK_S, VK_T, VK_U, VK_V, VK_W, VK_X, VK_Y, VK_Z,
    VK_F1, VK_F2, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_F10, VK_F11, VK_F12,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT,
};

const HOTKEY_ID_OCR: i32 = 1;
const HOTKEY_ID_WRITE: i32 = 2;

/// Global hotkey state — shared between the app and the hotkey thread.
pub struct HotkeyState {
    shutdown: Arc<AtomicBool>,
    thread_id: Arc<AtomicU32>,
    thread_handle: Option<JoinHandle<()>>,
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            thread_id: Arc::new(AtomicU32::new(0)),
            thread_handle: None,
        }
    }
}

/// Parse a hotkey string like "CTRL+SHIFT+T" into (modifiers, virtual_key).
pub fn parse_hotkey(hotkey_str: &str) -> Result<(HOT_KEY_MODIFIERS, VIRTUAL_KEY), String> {
    let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return Err("Empty hotkey string".to_string());
    }

    let mut modifiers = HOT_KEY_MODIFIERS(0);
    let mut key: Option<VIRTUAL_KEY> = None;

    for part in &parts {
        match part.to_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
            "SHIFT" => modifiers |= MOD_SHIFT,
            "ALT" => modifiers |= MOD_ALT,
            k => {
                if key.is_some() {
                    return Err(format!("Multiple keys specified: already had a key before '{k}'"));
                }
                key = Some(str_to_vk(k)?);
            }
        }
    }

    let vk = key.ok_or_else(|| "No key specified in hotkey string".to_string())?;
    if modifiers.0 == 0 {
        return Err("At least one modifier (CTRL, SHIFT, ALT) is required".to_string());
    }

    Ok((modifiers, vk))
}

/// Convert a single key name to a VIRTUAL_KEY.
fn str_to_vk(s: &str) -> Result<VIRTUAL_KEY, String> {
    match s {
        "A" => Ok(VK_A), "B" => Ok(VK_B), "C" => Ok(VK_C), "D" => Ok(VK_D),
        "E" => Ok(VK_E), "F" => Ok(VK_F), "G" => Ok(VK_G), "H" => Ok(VK_H),
        "I" => Ok(VK_I), "J" => Ok(VK_J), "K" => Ok(VK_K), "L" => Ok(VK_L),
        "M" => Ok(VK_M), "N" => Ok(VK_N), "O" => Ok(VK_O), "P" => Ok(VK_P),
        "Q" => Ok(VK_Q), "R" => Ok(VK_R), "S" => Ok(VK_S), "T" => Ok(VK_T),
        "U" => Ok(VK_U), "V" => Ok(VK_V), "W" => Ok(VK_W), "X" => Ok(VK_X),
        "Y" => Ok(VK_Y), "Z" => Ok(VK_Z),
        "F1" => Ok(VK_F1), "F2" => Ok(VK_F2), "F3" => Ok(VK_F3), "F4" => Ok(VK_F4),
        "F5" => Ok(VK_F5), "F6" => Ok(VK_F6), "F7" => Ok(VK_F7), "F8" => Ok(VK_F8),
        "F9" => Ok(VK_F9), "F10" => Ok(VK_F10), "F11" => Ok(VK_F11), "F12" => Ok(VK_F12),
        other => Err(format!("Unknown key: '{other}'. Use A-Z or F1-F12")),
    }
}

/// Register global hotkeys and start the message pump thread.
/// Returns Ok(()) if both hotkeys registered successfully.
pub fn register_hotkeys(
    state: &mut HotkeyState,
    ocr_hotkey: &str,
    write_hotkey: &str,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Parse both hotkeys before starting the thread
    let (ocr_mods, ocr_vk) = parse_hotkey(ocr_hotkey)?;
    let (write_mods, write_vk) = parse_hotkey(write_hotkey)?;

    // Check for duplicate hotkeys
    if ocr_mods == write_mods && ocr_vk == write_vk {
        return Err("OCR and Write hotkeys cannot be the same".to_string());
    }

    // If there's already a running thread, shut it down first
    unregister_hotkeys(state);

    let shutdown = state.shutdown.clone();
    let thread_id_store = state.thread_id.clone();
    shutdown.store(false, Ordering::SeqCst);

    let handle = thread::spawn(move || {
        // Store this thread's ID so we can post WM_QUIT to it later
        let tid = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
        thread_id_store.store(tid, Ordering::SeqCst);

        // Register hotkeys on THIS thread (they're thread-affine)
        let ocr_ok = unsafe {
            RegisterHotKey(
                HWND::default(),
                HOTKEY_ID_OCR,
                ocr_mods,
                ocr_vk.0 as u32,
            )
        };
        if ocr_ok.is_err() {
            eprintln!("Failed to register OCR hotkey — may be in use by another application");
            return;
        }

        let write_ok = unsafe {
            RegisterHotKey(
                HWND::default(),
                HOTKEY_ID_WRITE,
                write_mods,
                write_vk.0 as u32,
            )
        };
        if write_ok.is_err() {
            // Clean up the OCR hotkey we just registered
            let _ = unsafe { UnregisterHotKey(HWND::default(), HOTKEY_ID_OCR) };
            eprintln!("Failed to register Write hotkey — may be in use by another application");
            return;
        }

        eprintln!("Hotkeys registered: OCR (id={HOTKEY_ID_OCR}), Write (id={HOTKEY_ID_WRITE})");

        // Message pump — blocks until WM_QUIT or shutdown signal
        let mut msg = MSG::default();
        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            // GetMessageW blocks until a message arrives; returns false on WM_QUIT
            let ret = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
            if !ret.as_bool() {
                // WM_QUIT received
                break;
            }

            if msg.message == WM_HOTKEY {
                let hotkey_id = msg.wParam.0 as i32;
                match hotkey_id {
                    HOTKEY_ID_OCR => {
                        eprintln!("OCR hotkey pressed!");
                        let _ = app_handle.emit("start-ocr-flow", ());
                    }
                    HOTKEY_ID_WRITE => {
                        eprintln!("Write hotkey pressed!");
                        let _ = app_handle.emit("start-write-flow", ());
                    }
                    _ => {}
                }
            }
        }

        // Cleanup
        let _ = unsafe { UnregisterHotKey(HWND::default(), HOTKEY_ID_OCR) };
        let _ = unsafe { UnregisterHotKey(HWND::default(), HOTKEY_ID_WRITE) };
        eprintln!("Hotkeys unregistered, thread exiting");
    });

    state.thread_handle = Some(handle);
    Ok(())
}

/// Signal the hotkey thread to stop and wait for it to exit.
pub fn unregister_hotkeys(state: &mut HotkeyState) {
    state.shutdown.store(true, Ordering::SeqCst);

    let tid = state.thread_id.load(Ordering::SeqCst);
    if tid != 0 {
        // Post WM_QUIT to unblock GetMessageW
        let _ = unsafe { PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }

    if let Some(handle) = state.thread_handle.take() {
        let _ = handle.join();
    }

    state.thread_id.store(0, Ordering::SeqCst);
    state.shutdown.store(false, Ordering::SeqCst);
}
